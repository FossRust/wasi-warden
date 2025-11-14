use std::fs;
use std::io::{Read, Take};
use std::path::{Component, Path};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD as Base64};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::bindings::exports::osagent::agent::planner::PlannedAction;
use crate::config::HostConfig;

#[derive(Debug, Serialize)]
pub struct ActionReport {
    pub capability: String,
    pub success: bool,
    pub output: Value,
    pub error: Option<String>,
}

impl ActionReport {
    fn succeeded(capability: String, output: Value) -> Self {
        Self {
            capability,
            success: true,
            output,
            error: None,
        }
    }

    fn failed(capability: String, err: anyhow::Error) -> Self {
        Self {
            capability,
            success: false,
            output: Value::Null,
            error: Some(err.to_string()),
        }
    }
}

pub fn execute_planned_actions(
    config: &HostConfig,
    actions: &[PlannedAction],
) -> Vec<ActionReport> {
    actions
        .iter()
        .map(|action| execute_action(config, action))
        .collect()
}

fn execute_action(config: &HostConfig, action: &PlannedAction) -> ActionReport {
    let capability = action.capability.clone();
    match execute_action_inner(config, action) {
        Ok(value) => ActionReport::succeeded(capability, value),
        Err(err) => ActionReport::failed(capability, err),
    }
}

fn execute_action_inner(config: &HostConfig, action: &PlannedAction) -> Result<Value> {
    let kind = CapabilityKind::parse(&action.capability)
        .ok_or_else(|| anyhow!("unsupported capability `{}`", action.capability))?;
    let input: Value = serde_json::from_str(&action.input)
        .with_context(|| format!("capability `{}` input is not valid JSON", action.capability))?;
    match kind {
        CapabilityKind::FsListDir => {
            let params: FsListDirInput =
                serde_json::from_value(input).context("fs.list_dir input must be an object")?;
            fs_list_dir(config, params)
        }
        CapabilityKind::FsReadFile => {
            let params: FsReadFileInput =
                serde_json::from_value(input).context("fs.read_file input must be an object")?;
            fs_read_file(config, params)
        }
        CapabilityKind::ProcSpawn => {
            let params: ProcSpawnInput =
                serde_json::from_value(input).context("proc.spawn input must be an object")?;
            proc_spawn(config, params)
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum CapabilityKind {
    FsListDir,
    FsReadFile,
    ProcSpawn,
}

impl CapabilityKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "fs.list_dir" => Some(Self::FsListDir),
            "fs.read_file" => Some(Self::FsReadFile),
            "proc.spawn" => Some(Self::ProcSpawn),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct FsListDirInput {
    path: Option<String>,
}

fn fs_list_dir(config: &HostConfig, params: FsListDirInput) -> Result<Value> {
    let target = if let Some(path) = params.path {
        if path.trim().is_empty() {
            config.workspace_root.clone()
        } else {
            resolve_workspace_child(&config.workspace_root, &path)?
        }
    } else {
        config.workspace_root.clone()
    };
    let mut entries = Vec::new();
    let dir_iter = fs::read_dir(target.as_std_path())
        .with_context(|| format!("failed to list directory {}", target))?;
    for entry in dir_iter {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("entry name is not valid UTF-8"))?;
        entries.push(json!({
            "name": name,
            "kind": entry_kind(&metadata),
            "size_bytes": metadata.len(),
            "modified_ms": file_time_ms(&metadata),
        }));
    }
    Ok(json!({
        "path": target.as_str(),
        "entries": entries,
    }))
}

#[derive(Deserialize)]
struct FsReadFileInput {
    path: String,
    max_bytes: Option<u64>,
}

fn fs_read_file(config: &HostConfig, params: FsReadFileInput) -> Result<Value> {
    if params.path.trim().is_empty() {
        return Err(anyhow!("fs.read_file requires a non-empty `path`"));
    }
    let target = resolve_workspace_child(&config.workspace_root, &params.path)?;
    let limit = params.max_bytes.unwrap_or(4096);
    let mut file = std::fs::File::open(target.as_std_path())
        .with_context(|| format!("failed to open file {}", target))?;
    let mut reader: Take<&mut std::fs::File> = (&mut file).take(limit + 1);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    let truncated = buffer.len() as u64 > limit;
    if truncated {
        buffer.truncate(limit as usize);
    }
    let (encoding, contents) = match String::from_utf8(buffer.clone()) {
        Ok(text) => ("utf-8", text),
        Err(_) => ("base64", Base64.encode(&buffer)),
    };
    Ok(json!({
        "path": target.as_str(),
        "truncated": truncated,
        "encoding": encoding,
        "contents": contents,
    }))
}

#[derive(Deserialize)]
struct ProcSpawnInput {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
    env: Option<Vec<ProcEnvVar>>,
}

#[derive(Deserialize)]
struct ProcEnvVar {
    key: String,
    value: String,
}

fn proc_spawn(config: &HostConfig, params: ProcSpawnInput) -> Result<Value> {
    if params.command.trim().is_empty() {
        return Err(anyhow!("proc.spawn requires `command`"));
    }
    if !config.is_proc_allowed(&params.command) {
        return Err(anyhow!(
            "command `{}` is not allowed by policy",
            params.command
        ));
    }

    let working_dir = if let Some(cwd) = params.cwd {
        if cwd.trim().is_empty() {
            config.workspace_root.clone()
        } else {
            resolve_workspace_child(&config.workspace_root, &cwd)?
        }
    } else {
        config.workspace_root.clone()
    };

    let mut cmd = Command::new(&params.command);
    cmd.args(&params.args);
    cmd.current_dir(working_dir.as_std_path());
    cmd.env_clear();
    if let Some(env) = params.env {
        for var in env {
            cmd.env(var.key, var.value);
        }
    }

    let output = cmd
        .output()
        .with_context(|| format!("failed to execute {}", params.command))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(json!({
        "command": params.command,
        "args": params.args,
        "cwd": working_dir.as_str(),
        "status": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
    }))
}

fn resolve_workspace_child(root: &Utf8Path, relative: &str) -> Result<Utf8PathBuf> {
    if relative.is_empty() {
        return Ok(root.to_path_buf());
    }
    let rel_path = Path::new(relative);
    if rel_path.is_absolute() {
        return Err(anyhow!("absolute paths are not allowed"));
    }
    let mut candidate = root.as_std_path().to_path_buf();
    for component in rel_path.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(seg) => candidate.push(seg),
            _ => return Err(anyhow!("path traversal segments are not allowed")),
        }
    }
    let candidate =
        Utf8PathBuf::from_path_buf(candidate).map_err(|_| anyhow!("path is not valid UTF-8"))?;
    ensure_within_workspace(root, &candidate)?;
    Ok(candidate)
}

fn ensure_within_workspace(root: &Utf8Path, candidate: &Utf8Path) -> Result<()> {
    if candidate.as_std_path().starts_with(root.as_std_path()) {
        Ok(())
    } else {
        Err(anyhow!("path `{}` escapes workspace root", candidate))
    }
}

fn entry_kind(meta: &fs::Metadata) -> &'static str {
    if meta.is_file() {
        "file"
    } else if meta.is_dir() {
        "directory"
    } else if meta.file_type().is_symlink() {
        "symlink"
    } else {
        "other"
    }
}

fn file_time_ms(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|ts| ts.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|dur| dur.as_millis() as u64)
}

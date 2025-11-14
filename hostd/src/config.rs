use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;

use crate::cli::StepArgs;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub workspace_root: Utf8PathBuf,
    pub allowed_proc_commands: Vec<String>,
}

impl HostConfig {
    pub fn from_step_args(args: &StepArgs) -> Result<Self> {
        let workspace_root = normalize_path(&args.workspace)
            .with_context(|| format!("invalid workspace path {}", args.workspace.display()))?;
        Ok(Self {
            workspace_root,
            allowed_proc_commands: args.allow_proc.clone(),
        })
    }

    pub fn is_proc_allowed(&self, program: &str) -> bool {
        if self.allowed_proc_commands.is_empty() {
            return false;
        }
        let base = Path::new(program)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(program);
        self.allowed_proc_commands
            .iter()
            .any(|entry| entry == program || entry == base)
    }
}

fn normalize_path(path: &Path) -> Result<Utf8PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)
    };
    let canonical = fs::canonicalize(&absolute).unwrap_or(absolute);
    Utf8PathBuf::from_path_buf(canonical).map_err(|_| anyhow::anyhow!("path is not valid UTF-8"))
}

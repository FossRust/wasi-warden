use serde::Serialize;

mod bindings {
    wit_bindgen::generate!({
        path: "../wit",
        world: "control",
        generate_all,
        default_bindings_module: "bindings",
    });
}

use bindings::exports::osagent::agent::planner::{
    self, AgentError, CompletePlan, StepResponse,
};
use bindings::osagent::common::types::{CapabilityError, CapabilityErrorCode};
use bindings::osagent::fs::fs;
use bindings::osagent::proc::proc;

struct Agent;

impl planner::Guest for Agent {
    fn step(
        task: String,
        _observation: planner::Observation,
    ) -> Result<StepResponse, AgentError> {
        if let Some(path) = task.strip_prefix("list ") {
            let summary = format!("listed path `{}`", path.trim());
            let output = list_path(path.trim()).map_err(agent_error)?;
            Ok(StepResponse::Complete(CompletePlan {
                reason: summary,
                outcome: output,
            }))
        } else if let Some(rest) = task.strip_prefix("run ") {
            let summary = format!("ran command `{}`", rest.trim());
            let output = run_command(rest.trim()).map_err(agent_error)?;
            Ok(StepResponse::Complete(CompletePlan {
                reason: summary,
                outcome: output,
            }))
        } else {
            Ok(StepResponse::Complete(CompletePlan {
                reason: "task not understood".to_string(),
                outcome: "{}".to_string(),
            }))
        }
    }
}

bindings::export!(Agent);

fn list_path(path: &str) -> Result<String, AgentErr> {
    let root = fs::open_workspace().map_err(cap_err("fs.open_workspace"))?;
    let target = if path.is_empty() || path == "." {
        root
    } else {
        fs::open_dir(&root, path).map_err(cap_err("fs.open_dir"))?
    };
    let entries = fs::list_dir(&target).map_err(cap_err("fs.list_dir"))?;
    #[derive(Serialize)]
    struct Entry {
        name: String,
        kind: String,
        size_bytes: Option<u64>,
    }
    let data: Vec<Entry> = entries
        .into_iter()
        .map(|entry| Entry {
            name: entry.name,
            kind: format!("{:?}", entry.kind),
            size_bytes: entry.size_bytes,
        })
        .collect();
    to_json(&data)
}

fn run_command(spec: &str) -> Result<String, AgentErr> {
    let mut parts = spec
        .split_whitespace()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(AgentErr::fatal("no program specified"));
    }
    let program = parts.remove(0);
    let options = proc::SpawnOptions {
        argv: parts.clone(),
        working_dir: None,
        env: Vec::new(),
        stdin: proc::StdioMode::Null,
        stdout: proc::StdioMode::Pipe,
        stderr: proc::StdioMode::Pipe,
        timeout_ms: None,
    };
    let process = proc::spawn(&program, &options).map_err(cap_err("proc.spawn"))?;
    let stdout = read_stream(&process, |p| p.read_stdout(4096))?;
    let stderr = read_stream(&process, |p| p.read_stderr(4096))?;
    let status = process.wait(None).map_err(cap_err("proc.wait"))?;
    process.close();

    #[derive(Serialize)]
    struct RunResult {
        program: String,
        argv: Vec<String>,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    }

    let result = RunResult {
        program,
        argv: parts,
        exit_code: status.code,
        stdout: bytes_to_text(stdout),
        stderr: bytes_to_text(stderr),
    };
    to_json(&result)
}

fn read_stream<F>(process: &proc::Process, mut reader: F) -> Result<Vec<u8>, AgentErr>
where
    F: FnMut(&proc::Process) -> Result<proc::StreamRead, CapabilityError>,
{
    let mut data = Vec::new();
    loop {
        let chunk = reader(process).map_err(cap_err("proc.read"))?;
        data.extend_from_slice(&chunk.data);
        if chunk.eof {
            break;
        }
    }
    Ok(data)
}

fn bytes_to_text(data: Vec<u8>) -> String {
    String::from_utf8_lossy(&data).to_string()
}

fn to_json<T: Serialize>(value: &T) -> Result<String, AgentErr> {
    serde_json::to_string(value).map_err(|err| AgentErr::fatal(format!("json error: {err}")))
}

fn cap_err(op: &'static str) -> impl Fn(CapabilityError) -> AgentErr {
    move |err| {
        let retryable = matches!(err.code, CapabilityErrorCode::Unavailable);
        AgentErr::new(
            retryable,
            format!("{op} failed: {} ({:?})", err.message, err.code),
        )
    }
}

fn agent_error(err: AgentErr) -> AgentError {
    AgentError {
        retryable: err.retryable,
        message: err.message,
    }
}

struct AgentErr {
    retryable: bool,
    message: String,
}

impl AgentErr {
    fn new(retryable: bool, message: impl Into<String>) -> Self {
        Self {
            retryable,
            message: message.into(),
        }
    }

    fn fatal(message: impl Into<String>) -> Self {
        Self::new(false, message)
    }
}

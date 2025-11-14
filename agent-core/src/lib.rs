use serde::Deserialize;
use serde_json::Value;

mod bindings {
    wit_bindgen::generate!({
        path: "../wit",
        world: "control",
        generate_all,
        default_bindings_module: "bindings",
    });
}

use bindings::exports::osagent::agent::planner::{
    self, AgentError, CompletePlan, ContinuePlan, PlannedAction, StepResponse,
};
use bindings::osagent::common::types::{CapabilityError, CapabilityErrorCode};
use bindings::osagent::llm::llm::{self, Message, Role};

const SYSTEM_PROMPT: &str = r#"
You are an expert automation planner operating inside a secure agent runtime.
Respond ONLY with JSON matching this schema:
{
  "status": "continue" | "complete",
  "thought": "human-readable reasoning",
  "actions": [
     { "capability": "fs.list_dir" | "proc.spawn" | "fs.read_file",
       "input": { ... json arguments ... }
     }
  ],
  "result": { ... final result json when status == "complete" },
  "reason": "short explanation when status == \"complete\""
}
When status is \"continue\" you MUST include at least one action describing the next capability call.
Capabilities available:
- fs.list_dir { "path": "<relative path>" }
- fs.read_file { "path": "<relative path>", "max_bytes": 4096 }
- proc.spawn { "command": "<program>", "args": ["..."] }
Always keep paths relative to the provided workspace.
"#;

struct Agent;

impl planner::Guest for Agent {
    fn step(task: String, observation: planner::Observation) -> Result<StepResponse, AgentError> {
        plan_with_llm(task, observation).map_err(agent_error)
    }
}

bindings::export!(Agent);

fn plan_with_llm(
    task: String,
    observation: planner::Observation,
) -> Result<StepResponse, AgentErr> {
    let messages = build_messages(&task, &observation);
    let options = llm::Options {
        max_tokens: Some(600),
        temperature: Some(0.2),
        top_p: Some(0.9),
        stop: Vec::new(),
        presence_penalty: None,
        frequency_penalty: None,
    };
    let completion = llm::complete(&messages, &options).map_err(cap_err("llm.complete"))?;
    let envelope: PlanEnvelope = serde_json::from_str(&completion.content).map_err(|err| {
        AgentErr::fatal(format!(
            "failed to parse LLM response: {err}; content: {}",
            completion.content
        ))
    })?;

    match envelope.status {
        PlanStatus::Continue => {
            let actions = envelope
                .actions
                .ok_or_else(|| AgentErr::fatal("LLM continuation missing actions"))?;
            if actions.is_empty() {
                return Err(AgentErr::fatal("LLM returned no actions"));
            }
            let planned = actions
                .into_iter()
                .map(to_planned_action)
                .collect::<Result<_, _>>()?;
            let thought = envelope
                .thought
                .unwrap_or_else(|| "No reasoning provided.".to_string());
            Ok(StepResponse::Continue(ContinuePlan {
                thought,
                actions: planned,
            }))
        }
        PlanStatus::Complete => {
            let reason = envelope
                .reason
                .or(envelope.thought)
                .unwrap_or_else(|| "task complete".to_string());
            let outcome = envelope.result.unwrap_or(Value::Null).to_string();
            Ok(StepResponse::Complete(CompletePlan { reason, outcome }))
        }
    }
}

fn build_messages(task: &str, observation: &planner::Observation) -> Vec<Message> {
    vec![
        Message {
            role: Role::System,
            content: SYSTEM_PROMPT.to_string(),
            name: None,
        },
        Message {
            role: Role::User,
            content: format!(
                "Task: {task}\nCurrent step #: {}\nLast observation summary: {}\nObservation data: {}",
                observation.step, observation.summary, observation.data
            ),
            name: None,
        },
    ]
}

fn to_planned_action(action: LlmAction) -> Result<PlannedAction, AgentErr> {
    Ok(PlannedAction {
        capability: action.capability,
        input: action.input.to_string(),
        audit_tag: None,
    })
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

#[derive(Deserialize)]
struct PlanEnvelope {
    status: PlanStatus,
    thought: Option<String>,
    actions: Option<Vec<LlmAction>>,
    result: Option<Value>,
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PlanStatus {
    Continue,
    Complete,
}

#[derive(Deserialize)]
struct LlmAction {
    capability: String,
    input: Value,
}

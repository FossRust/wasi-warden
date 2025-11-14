use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{info, warn};
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker},
};
use wasmtime_wasi::add_to_linker_sync;

use crate::bindings;
use crate::bindings::exports::osagent::agent::planner::{AgentError, Observation, StepResponse};
use crate::cli::StepArgs;
use crate::config::HostConfig;
use crate::state::HostState;

pub async fn run_step(args: StepArgs) -> Result<()> {
    let config = HostConfig::from_step_args(&args)?;
    let engine = build_engine()?;
    let component = load_component(&engine, &args.component)?;

    let observation_json = validate_json(&args.observation)?;
    let observation = Observation {
        step: args.step,
        summary: format!("host bootstrap step {}", args.step),
        data: observation_json,
    };

    let mut linker: Linker<HostState> = Linker::new(&engine);
    add_to_linker_sync(&mut linker).context("failed to add WASI to linker")?;
    bindings::Control::add_to_linker(&mut linker, |state: &mut HostState| state)?;

    let mut store = Store::new(&engine, HostState::new(config));
    let control = bindings::Control::instantiate(&mut store, &component, &linker)
        .context("failed to instantiate component")?;
    let planner = control.osagent_agent_planner();

    let planner_result = planner
        .call_step(&mut store, &args.task, &observation)
        .context("planner.step failed")?;
    let response = planner_result.map_err(agent_failure)?;

    match response {
        StepResponse::Continue(plan) => {
            info!(
                thought = plan.thought,
                actions = plan.actions.len(),
                "planner requested additional capability calls"
            );
            for action in plan.actions {
                warn!(
                    capability = action.capability,
                    "capability stubs deny-by-default (input: {})", action.input
                );
            }
        }
        StepResponse::Complete(done) => {
            info!(
                reason = done.reason,
                outcome = done.outcome,
                "planner completed task"
            );
        }
    }

    Ok(())
}

fn build_engine() -> Result<Engine> {
    let mut config = Config::default();
    config.wasm_backtrace(true);
    config.wasm_component_model(true);
    config.async_support(false);
    Engine::new(&config).context("failed to build Wasmtime engine")
}

fn load_component(engine: &Engine, path: &PathBuf) -> Result<Component> {
    Component::from_file(engine, path)
        .with_context(|| format!("failed to load component {}", path.display()))
}

fn validate_json(input: &str) -> Result<String> {
    let json: Value = serde_json::from_str(input)
        .with_context(|| format!("observation is not valid JSON: {input}"))?;
    Ok(json.to_string())
}

fn agent_failure(err: AgentError) -> anyhow::Error {
    anyhow::anyhow!(
        "agent-core reported error (retryable={}): {}",
        err.retryable,
        err.message
    )
}

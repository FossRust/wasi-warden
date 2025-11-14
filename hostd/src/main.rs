mod resources;
mod bindings {
    wasmtime::component::bindgen!({
        path: "../wit",
        world: "control",
        with: {
            "osagent:fs/fs/dir-handle": crate::resources::DirHandleResource,
            "osagent:fs/fs/file-handle": crate::resources::FileHandleResource,
        },
    });
}
mod capabilities;
mod cli;
mod config;
mod logging;
mod runtime;
mod state;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    logging::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Step(args) => runtime::run_step(args).await?,
    }
    Ok(())
}

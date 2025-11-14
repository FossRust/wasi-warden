use wasmtime::component::ResourceTable;
use wasmtime_wasi::{IoView, WasiCtx, WasiCtxBuilder, WasiView};

use crate::config::HostConfig;

#[allow(dead_code)]
pub struct HostState {
    pub config: HostConfig,
    pub resources: ResourceTable,
    pub wasi_ctx: WasiCtx,
}

impl HostState {
    pub fn new(config: HostConfig) -> Self {
        let wasi_ctx = WasiCtxBuilder::new().build();
        Self {
            config,
            resources: ResourceTable::new(),
            wasi_ctx,
        }
    }
}

impl IoView for HostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resources
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

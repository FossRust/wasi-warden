use wasmtime::component::ResourceTable;

use crate::config::HostConfig;

#[allow(dead_code)]
#[derive(Debug)]
pub struct HostState {
    pub config: HostConfig,
    pub resources: ResourceTable,
}

impl HostState {
    pub fn new(config: HostConfig) -> Self {
        Self {
            config,
            resources: ResourceTable::new(),
        }
    }
}

use crate::args::{LoadMode, Protocol};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolAdapterError {
    pub message: String,
}

pub trait ProtocolAdapter: Send + Sync {
    fn protocol(&self) -> Protocol;
    fn display_name(&self) -> &'static str;
    fn executes_traffic(&self) -> bool;
    fn supports_stateful_connections(&self) -> bool;
    fn supported_load_modes(&self) -> &'static [LoadMode];
}

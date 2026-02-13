mod builtins;
mod registry;
mod runtime;
mod traits;

#[cfg(test)]
pub(crate) mod examples;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use registry::ProtocolRegistry;
pub use registry::protocol_registry;
pub use runtime::setup_request_sender;
pub use traits::{ProtocolAdapter, ProtocolAdapterError};

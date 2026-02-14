use std::sync::{Arc, OnceLock};

use crate::domain::run::{LoadMode, ProtocolKind};

use super::builtins;
use super::{ProtocolAdapterError, TransportAdapter};

#[derive(Clone)]
pub struct ProtocolRegistry {
    adapters: Vec<Arc<dyn TransportAdapter>>,
}

impl ProtocolRegistry {
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self {
            adapters: Vec::new(),
        };
        for builtin in builtins::builtins() {
            if let Err(err) = registry.register_adapter(builtin) {
                tracing::warn!(
                    "Skipping duplicate builtin protocol adapter: {}",
                    err.message
                );
            }
        }
        registry
    }

    /// Registers a protocol adapter into this registry.
    ///
    /// # Errors
    ///
    /// Returns an error when an adapter with the same `Protocol` key is already
    /// registered.
    pub fn register_adapter<P>(&mut self, adapter: P) -> Result<(), ProtocolAdapterError>
    where
        P: TransportAdapter + 'static,
    {
        let protocol = adapter.protocol();
        if self
            .adapters
            .iter()
            .any(|existing| existing.protocol() == protocol)
        {
            return Err(ProtocolAdapterError {
                message: format!("Protocol adapter already registered: {}", protocol.as_str()),
            });
        }
        self.adapters.push(Arc::new(adapter));
        Ok(())
    }

    pub fn adapter(&self, protocol: ProtocolKind) -> Option<&dyn TransportAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.protocol() == protocol)
            .map(Arc::as_ref)
    }

    #[must_use]
    pub fn supports_execution(&self, protocol: ProtocolKind) -> bool {
        self.adapter(protocol)
            .map(|adapter| adapter.executes_traffic())
            .unwrap_or(false)
    }

    #[must_use]
    pub fn supports_load_mode(&self, protocol: ProtocolKind, load_mode: LoadMode) -> bool {
        self.adapter(protocol)
            .map(|adapter| adapter.supported_load_modes().contains(&load_mode))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn executable_protocols_csv(&self) -> String {
        let mut protocols: Vec<&'static str> = self
            .adapters
            .iter()
            .filter(|adapter| adapter.executes_traffic())
            .map(|adapter| adapter.protocol().as_str())
            .collect();
        protocols.sort_unstable();
        protocols.join(", ")
    }
}

pub fn protocol_registry() -> &'static ProtocolRegistry {
    static REGISTRY: OnceLock<ProtocolRegistry> = OnceLock::new();
    REGISTRY.get_or_init(ProtocolRegistry::with_builtins)
}

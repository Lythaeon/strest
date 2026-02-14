use crate::domain::run::{LoadMode, ProtocolKind};

use crate::protocol::{ProtocolAdapter, TransportAdapter};

const LOAD_MODES: &[LoadMode] = &[
    LoadMode::Arrival,
    LoadMode::Step,
    LoadMode::Ramp,
    LoadMode::Burst,
    LoadMode::Soak,
];

#[derive(Clone)]
pub struct ChatWebSocketPlugin;

impl ProtocolAdapter for ChatWebSocketPlugin {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::Websocket
    }

    fn display_name(&self) -> &'static str {
        "Chat WebSocket Example"
    }

    fn executes_traffic(&self) -> bool {
        false
    }

    fn supports_stateful_connections(&self) -> bool {
        true
    }

    fn supported_load_modes(&self) -> &'static [LoadMode] {
        LOAD_MODES
    }
}

impl TransportAdapter for ChatWebSocketPlugin {}

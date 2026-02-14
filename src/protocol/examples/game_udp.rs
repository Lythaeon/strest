use crate::domain::run::{LoadMode, ProtocolKind};

use crate::protocol::{ProtocolAdapter, TransportAdapter};

const LOAD_MODES: &[LoadMode] = &[LoadMode::Arrival, LoadMode::Jitter, LoadMode::Burst];

#[derive(Clone)]
pub struct GameUdpPlugin;

impl ProtocolAdapter for GameUdpPlugin {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::Udp
    }

    fn display_name(&self) -> &'static str {
        "Game UDP Example"
    }

    fn executes_traffic(&self) -> bool {
        false
    }

    fn supports_stateful_connections(&self) -> bool {
        false
    }

    fn supported_load_modes(&self) -> &'static [LoadMode] {
        LOAD_MODES
    }
}

impl TransportAdapter for GameUdpPlugin {}

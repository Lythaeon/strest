use crate::args::{LoadMode, Protocol};

use crate::protocol::ProtocolAdapter;

const LOAD_MODES: &[LoadMode] = &[LoadMode::Soak, LoadMode::Burst];

#[derive(Clone)]
pub struct TelemetryMqttPlugin;

impl ProtocolAdapter for TelemetryMqttPlugin {
    fn protocol(&self) -> Protocol {
        Protocol::Mqtt
    }

    fn display_name(&self) -> &'static str {
        "Telemetry MQTT Example"
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

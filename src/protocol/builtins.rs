use crate::args::{LoadMode, Protocol};

use super::ProtocolAdapter;

const ALL_LOAD_MODES: &[LoadMode] = &[
    LoadMode::Arrival,
    LoadMode::Step,
    LoadMode::Ramp,
    LoadMode::Jitter,
    LoadMode::Burst,
    LoadMode::Soak,
];
const SOAK_BURST_ONLY: &[LoadMode] = &[LoadMode::Burst, LoadMode::Soak];
const ARRIVAL_RAMP_ONLY: &[LoadMode] = &[LoadMode::Arrival, LoadMode::Ramp];

#[derive(Clone)]
pub(super) struct StaticProtocolAdapter {
    protocol: Protocol,
    display_name: &'static str,
    executes_traffic: bool,
    supports_stateful_connections: bool,
    supported_load_modes: &'static [LoadMode],
}

impl StaticProtocolAdapter {
    const fn new(
        protocol: Protocol,
        display_name: &'static str,
        executes_traffic: bool,
        supports_stateful_connections: bool,
        supported_load_modes: &'static [LoadMode],
    ) -> Self {
        Self {
            protocol,
            display_name,
            executes_traffic,
            supports_stateful_connections,
            supported_load_modes,
        }
    }

    pub(super) const fn http() -> Self {
        Self::new(Protocol::Http, "HTTP", true, true, ALL_LOAD_MODES)
    }

    pub(super) const fn grpc_unary() -> Self {
        Self::new(
            Protocol::GrpcUnary,
            "gRPC Unary",
            true,
            true,
            ARRIVAL_RAMP_ONLY,
        )
    }

    pub(super) const fn grpc_streaming() -> Self {
        Self::new(
            Protocol::GrpcStreaming,
            "gRPC Streaming",
            true,
            true,
            ALL_LOAD_MODES,
        )
    }

    pub(super) const fn websocket() -> Self {
        Self::new(Protocol::Websocket, "WebSocket", true, true, ALL_LOAD_MODES)
    }

    pub(super) const fn tcp() -> Self {
        Self::new(Protocol::Tcp, "TCP", true, true, ALL_LOAD_MODES)
    }

    pub(super) const fn udp() -> Self {
        Self::new(Protocol::Udp, "UDP", true, false, ALL_LOAD_MODES)
    }

    pub(super) const fn quic() -> Self {
        Self::new(Protocol::Quic, "QUIC", true, true, ALL_LOAD_MODES)
    }

    pub(super) const fn mqtt() -> Self {
        Self::new(Protocol::Mqtt, "MQTT", true, true, SOAK_BURST_ONLY)
    }

    pub(super) const fn enet() -> Self {
        Self::new(Protocol::Enet, "ENet", true, true, ALL_LOAD_MODES)
    }

    pub(super) const fn kcp() -> Self {
        Self::new(Protocol::Kcp, "KCP", true, true, ALL_LOAD_MODES)
    }

    pub(super) const fn raknet() -> Self {
        Self::new(Protocol::Raknet, "RakNet", true, true, ALL_LOAD_MODES)
    }
}

impl ProtocolAdapter for StaticProtocolAdapter {
    fn protocol(&self) -> Protocol {
        self.protocol
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn executes_traffic(&self) -> bool {
        self.executes_traffic
    }

    fn supports_stateful_connections(&self) -> bool {
        self.supports_stateful_connections
    }

    fn supported_load_modes(&self) -> &'static [LoadMode] {
        self.supported_load_modes
    }
}

pub(super) const fn builtins() -> [StaticProtocolAdapter; 11] {
    [
        StaticProtocolAdapter::http(),
        StaticProtocolAdapter::grpc_unary(),
        StaticProtocolAdapter::grpc_streaming(),
        StaticProtocolAdapter::websocket(),
        StaticProtocolAdapter::tcp(),
        StaticProtocolAdapter::udp(),
        StaticProtocolAdapter::quic(),
        StaticProtocolAdapter::mqtt(),
        StaticProtocolAdapter::enet(),
        StaticProtocolAdapter::kcp(),
        StaticProtocolAdapter::raknet(),
    ]
}

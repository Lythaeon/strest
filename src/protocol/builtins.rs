use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::args::TesterArgs;
use crate::domain::run::{LoadMode, ProtocolKind};
use crate::error::AppResult;
use crate::metrics::{LogSink, Metrics};
use crate::shutdown::ShutdownSender;

use super::{ProtocolAdapter, TransportAdapter};

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
    protocol: ProtocolKind,
    display_name: &'static str,
    executes_traffic: bool,
    supports_stateful_connections: bool,
    supported_load_modes: &'static [LoadMode],
    setup_request_sender: SetupRequestSenderFn,
}

type SetupRequestSenderFn = fn(
    &TesterArgs,
    &ShutdownSender,
    &mpsc::Sender<Metrics>,
    Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>>;

impl StaticProtocolAdapter {
    const fn new(
        protocol: ProtocolKind,
        display_name: &'static str,
        executes_traffic: bool,
        supports_stateful_connections: bool,
        supported_load_modes: &'static [LoadMode],
        setup_request_sender: SetupRequestSenderFn,
    ) -> Self {
        Self {
            protocol,
            display_name,
            executes_traffic,
            supports_stateful_connections,
            supported_load_modes,
            setup_request_sender,
        }
    }

    pub(super) const fn http() -> Self {
        Self::new(
            ProtocolKind::Http,
            "HTTP",
            true,
            true,
            ALL_LOAD_MODES,
            crate::http::setup_request_sender,
        )
    }

    pub(super) const fn grpc_unary() -> Self {
        Self::new(
            ProtocolKind::GrpcUnary,
            "gRPC Unary",
            true,
            true,
            ARRIVAL_RAMP_ONLY,
            super::runtime::setup_grpc_unary_sender,
        )
    }

    pub(super) const fn grpc_streaming() -> Self {
        Self::new(
            ProtocolKind::GrpcStreaming,
            "gRPC Streaming",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_grpc_streaming_sender,
        )
    }

    pub(super) const fn websocket() -> Self {
        Self::new(
            ProtocolKind::Websocket,
            "WebSocket",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_websocket_sender,
        )
    }

    pub(super) const fn tcp() -> Self {
        Self::new(
            ProtocolKind::Tcp,
            "TCP",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_tcp_sender,
        )
    }

    pub(super) const fn udp() -> Self {
        Self::new(
            ProtocolKind::Udp,
            "UDP",
            true,
            false,
            ALL_LOAD_MODES,
            super::runtime::setup_udp_sender,
        )
    }

    pub(super) const fn quic() -> Self {
        Self::new(
            ProtocolKind::Quic,
            "QUIC",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_quic_sender,
        )
    }

    pub(super) const fn mqtt() -> Self {
        Self::new(
            ProtocolKind::Mqtt,
            "MQTT",
            true,
            true,
            SOAK_BURST_ONLY,
            super::runtime::setup_mqtt_sender,
        )
    }

    pub(super) const fn enet() -> Self {
        Self::new(
            ProtocolKind::Enet,
            "ENet",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_enet_sender,
        )
    }

    pub(super) const fn kcp() -> Self {
        Self::new(
            ProtocolKind::Kcp,
            "KCP",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_kcp_sender,
        )
    }

    pub(super) const fn raknet() -> Self {
        Self::new(
            ProtocolKind::Raknet,
            "RakNet",
            true,
            true,
            ALL_LOAD_MODES,
            super::runtime::setup_raknet_sender,
        )
    }
}

impl ProtocolAdapter for StaticProtocolAdapter {
    fn protocol(&self) -> ProtocolKind {
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

impl TransportAdapter for StaticProtocolAdapter {
    fn setup_request_sender(
        &self,
        args: &TesterArgs,
        shutdown_tx: &ShutdownSender,
        metrics_tx: &mpsc::Sender<Metrics>,
        log_sink: Option<&Arc<LogSink>>,
    ) -> AppResult<JoinHandle<()>> {
        (self.setup_request_sender)(args, shutdown_tx, metrics_tx, log_sink)
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

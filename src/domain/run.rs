#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolKind {
    Http,
    GrpcUnary,
    GrpcStreaming,
    Websocket,
    Tcp,
    Udp,
    Quic,
    Mqtt,
    Enet,
    Kcp,
    Raknet,
}

impl ProtocolKind {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            ProtocolKind::Http => "http",
            ProtocolKind::GrpcUnary => "grpc-unary",
            ProtocolKind::GrpcStreaming => "grpc-streaming",
            ProtocolKind::Websocket => "websocket",
            ProtocolKind::Tcp => "tcp",
            ProtocolKind::Udp => "udp",
            ProtocolKind::Quic => "quic",
            ProtocolKind::Mqtt => "mqtt",
            ProtocolKind::Enet => "enet",
            ProtocolKind::Kcp => "kcp",
            ProtocolKind::Raknet => "raknet",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadMode {
    Arrival,
    Step,
    Ramp,
    Jitter,
    Burst,
    Soak,
}

impl LoadMode {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            LoadMode::Arrival => "arrival",
            LoadMode::Step => "step",
            LoadMode::Ramp => "ramp",
            LoadMode::Jitter => "jitter",
            LoadMode::Burst => "burst",
            LoadMode::Soak => "soak",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scenario {
    pub(crate) base_url: Option<String>,
    pub(crate) vars_count: usize,
    pub(crate) step_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RunConfig {
    pub(crate) protocol: ProtocolKind,
    pub(crate) load_mode: LoadMode,
    pub(crate) target_url: Option<String>,
    pub(crate) scenario: Option<Scenario>,
}

impl RunConfig {
    #[must_use]
    pub(crate) fn scenario_step_count(&self) -> usize {
        self.scenario
            .as_ref()
            .map(|scenario| scenario.step_count)
            .unwrap_or(0)
    }

    #[must_use]
    pub(crate) fn scenario_vars_count(&self) -> usize {
        self.scenario
            .as_ref()
            .map(|scenario| scenario.vars_count)
            .unwrap_or(0)
    }

    #[must_use]
    pub(crate) fn scenario_base_url(&self) -> Option<&str> {
        self.scenario
            .as_ref()
            .and_then(|scenario| scenario.base_url.as_deref())
    }
}

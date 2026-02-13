use super::*;
use crate::args::{LoadMode, Protocol};

#[derive(Clone)]
struct FakeAdapter;

impl ProtocolAdapter for FakeAdapter {
    fn protocol(&self) -> Protocol {
        Protocol::Http
    }

    fn display_name(&self) -> &'static str {
        "fake-http"
    }

    fn executes_traffic(&self) -> bool {
        true
    }

    fn supports_stateful_connections(&self) -> bool {
        true
    }

    fn supported_load_modes(&self) -> &'static [LoadMode] {
        &[
            LoadMode::Arrival,
            LoadMode::Step,
            LoadMode::Ramp,
            LoadMode::Jitter,
            LoadMode::Burst,
            LoadMode::Soak,
        ]
    }
}

#[test]
fn builtins_register_http_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Http).is_some());
    assert!(registry.supports_execution(Protocol::Http));
    assert!(registry.supports_load_mode(Protocol::Http, LoadMode::Soak));
}

#[test]
fn builtins_mark_websocket_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Websocket).is_some());
    assert!(registry.supports_execution(Protocol::Websocket));
}

#[test]
fn builtins_mark_grpc_unary_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::GrpcUnary).is_some());
    assert!(registry.supports_execution(Protocol::GrpcUnary));
}

#[test]
fn builtins_mark_grpc_streaming_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::GrpcStreaming).is_some());
    assert!(registry.supports_execution(Protocol::GrpcStreaming));
}

#[test]
fn builtins_mark_quic_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Quic).is_some());
    assert!(registry.supports_execution(Protocol::Quic));
}

#[test]
fn builtins_mark_mqtt_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Mqtt).is_some());
    assert!(registry.supports_execution(Protocol::Mqtt));
}

#[test]
fn builtins_mark_enet_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Enet).is_some());
    assert!(registry.supports_execution(Protocol::Enet));
}

#[test]
fn builtins_mark_kcp_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Kcp).is_some());
    assert!(registry.supports_execution(Protocol::Kcp));
}

#[test]
fn builtins_mark_raknet_as_executable() {
    let registry = ProtocolRegistry::with_builtins();
    assert!(registry.adapter(Protocol::Raknet).is_some());
    assert!(registry.supports_execution(Protocol::Raknet));
}

#[test]
fn duplicate_registration_is_rejected() {
    let mut registry = ProtocolRegistry::with_builtins();
    let second = registry.register_adapter(FakeAdapter);
    assert!(second.is_err());
}

#[test]
fn example_adapters_are_registerable() {
    let mut registry = ProtocolRegistry::with_builtins();
    let game = examples::GameUdpPlugin;
    let chat = examples::ChatWebSocketPlugin;
    let mqtt = examples::TelemetryMqttPlugin;

    assert!(registry.register_adapter(game).is_err());
    assert!(registry.register_adapter(chat).is_err());
    assert!(registry.register_adapter(mqtt).is_err());
}

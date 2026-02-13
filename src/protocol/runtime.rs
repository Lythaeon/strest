mod datagram;
mod grpc;
mod mqtt;
mod resolve;
mod spawner;
mod transports;
mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use url::Url;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ValidationError};
use crate::metrics::{LogSink, Metrics};
use crate::shutdown::ShutdownSender;

use datagram::{datagram_payload, setup_datagram_sender};
use grpc::{build_grpc_client, grpc_frame, grpc_request_once};
use mqtt::{mqtt_request_once, topic_from_path};
use resolve::{resolve_endpoint, resolve_grpc_url, resolve_websocket_url};
use spawner::spawn_transport_sender;
use transports::{tcp_request_once, websocket_request_once};

/// Creates protocol-specific request sender task.
///
/// # Errors
///
/// Returns an error when protocol settings are invalid or unsupported.
pub fn setup_request_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    match args.protocol {
        crate::args::Protocol::Http => {
            crate::http::setup_request_sender(args, shutdown_tx, metrics_tx, log_sink)
        }
        crate::args::Protocol::Tcp => setup_tcp_sender(args, shutdown_tx, metrics_tx, log_sink),
        crate::args::Protocol::Udp => setup_udp_sender(args, shutdown_tx, metrics_tx, log_sink),
        crate::args::Protocol::Websocket => {
            setup_websocket_sender(args, shutdown_tx, metrics_tx, log_sink)
        }
        crate::args::Protocol::GrpcUnary => {
            setup_grpc_unary_sender(args, shutdown_tx, metrics_tx, log_sink)
        }
        crate::args::Protocol::GrpcStreaming => {
            setup_grpc_streaming_sender(args, shutdown_tx, metrics_tx, log_sink)
        }
        crate::args::Protocol::Quic => setup_quic_sender(args, shutdown_tx, metrics_tx, log_sink),
        crate::args::Protocol::Mqtt => setup_mqtt_sender(args, shutdown_tx, metrics_tx, log_sink),
        crate::args::Protocol::Enet => setup_enet_sender(args, shutdown_tx, metrics_tx, log_sink),
        crate::args::Protocol::Kcp => setup_kcp_sender(args, shutdown_tx, metrics_tx, log_sink),
        crate::args::Protocol::Raknet => {
            setup_raknet_sender(args, shutdown_tx, metrics_tx, log_sink)
        }
    }
}

fn setup_tcp_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    let endpoint = resolve_endpoint(args, &[("tcp", 80), ("http", 80), ("https", 443)])?;
    let payload = args.data.clone().into_bytes();
    Ok(spawn_transport_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        move |request_timeout, connect_timeout| {
            let endpoint = endpoint;
            let payload = payload.clone();
            Box::pin(async move {
                tcp_request_once(endpoint, &payload, request_timeout, connect_timeout).await
            })
        },
    ))
}

fn setup_udp_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_datagram_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        &[("udp", 80), ("http", 80), ("https", 443)],
        datagram_payload(args),
    )
}

fn setup_quic_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_datagram_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        &[("quic", 4433), ("udp", 4433), ("http", 80), ("https", 443)],
        datagram_payload(args),
    )
}

fn setup_enet_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_datagram_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        &[("enet", 7777), ("udp", 7777), ("http", 80), ("https", 443)],
        datagram_payload(args),
    )
}

fn setup_kcp_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_datagram_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        &[("kcp", 4000), ("udp", 4000), ("http", 80), ("https", 443)],
        datagram_payload(args),
    )
}

fn setup_raknet_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_datagram_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        &[
            ("raknet", 19132),
            ("udp", 19132),
            ("http", 80),
            ("https", 443),
        ],
        datagram_payload(args),
    )
}

fn setup_mqtt_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    let endpoint = resolve_endpoint(args, &[("mqtt", 1883), ("tcp", 1883), ("http", 80)])?;
    let raw_url = args
        .url
        .as_deref()
        .ok_or_else(|| AppError::validation(ValidationError::MissingUrl))?;
    let topic = Url::parse(raw_url).ok().map_or_else(
        || "strest/loadtest".to_owned(),
        |url| topic_from_path(url.path()),
    );
    let payload = datagram_payload(args);

    Ok(spawn_transport_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        move |request_timeout, connect_timeout| {
            let endpoint = endpoint;
            let topic = topic.clone();
            let payload = payload.clone();
            Box::pin(async move {
                mqtt_request_once(endpoint, &topic, &payload, request_timeout, connect_timeout)
                    .await
            })
        },
    ))
}

fn setup_websocket_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    let ws_url = resolve_websocket_url(args)?;
    let payload = args.data.clone();
    Ok(spawn_transport_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        move |request_timeout, connect_timeout| {
            let ws_url = ws_url.clone();
            let payload = payload.clone();
            Box::pin(async move {
                websocket_request_once(&ws_url, &payload, request_timeout, connect_timeout).await
            })
        },
    ))
}

fn setup_grpc_unary_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_grpc_sender(args, shutdown_tx, metrics_tx, log_sink, false)
}

fn setup_grpc_streaming_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<JoinHandle<()>> {
    setup_grpc_sender(args, shutdown_tx, metrics_tx, log_sink, true)
}

fn setup_grpc_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
    streaming: bool,
) -> AppResult<JoinHandle<()>> {
    let (grpc_url, prior_knowledge) = resolve_grpc_url(args)?;
    let client = build_grpc_client(args.connect_timeout, prior_knowledge)?;
    let payload = Arc::<[u8]>::from(grpc_frame(args.data.as_bytes()));

    Ok(spawn_transport_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        move |request_timeout, _connect_timeout| {
            let client = client.clone();
            let grpc_url = grpc_url.clone();
            let payload = Arc::clone(&payload);
            Box::pin(async move {
                grpc_request_once(&client, &grpc_url, &payload, request_timeout, streaming).await
            })
        },
    ))
}

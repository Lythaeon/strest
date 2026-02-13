use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::args::TesterArgs;
use crate::error::AppResult;
use crate::metrics::{LogSink, Metrics};
use crate::shutdown::ShutdownSender;

use super::resolve::resolve_endpoint;
use super::spawner::spawn_transport_sender;
use super::transports::udp_request_once;

pub(super) fn setup_datagram_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
    allowed_schemes: &[(&'static str, u16)],
    payload: Vec<u8>,
) -> AppResult<JoinHandle<()>> {
    let endpoint = resolve_endpoint(args, allowed_schemes)?;
    Ok(spawn_transport_sender(
        args,
        shutdown_tx,
        metrics_tx,
        log_sink,
        move |request_timeout, _connect_timeout| {
            let endpoint = endpoint;
            let payload = payload.clone();
            Box::pin(async move { udp_request_once(endpoint, &payload, request_timeout).await })
        },
    ))
}

pub(super) fn datagram_payload(args: &TesterArgs) -> Vec<u8> {
    if args.data.is_empty() {
        vec![0_u8]
    } else {
        args.data.clone().into_bytes()
    }
}

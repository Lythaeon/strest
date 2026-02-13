use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{broadcast, mpsc};

use crate::error::{AppError, AppResult};
use crate::metrics::Metrics;

use super::{
    SHUTDOWN_CHANNEL_CAPACITY, join_handle, join_result_handle, parse_args, permission_denied,
    run_async_test, setup_request_sender, spawn_mqtt_mock_server, spawn_udp_echo_server,
    wait_metric,
};

#[test]
fn datagram_protocols_emit_success_metric() -> AppResult<()> {
    run_async_test(async {
        match UdpSocket::bind("127.0.0.1:0").await {
            Ok(socket) => {
                drop(socket);
            }
            Err(err) => {
                if permission_denied(&err) {
                    return Ok(());
                }
                return Err(AppError::validation(format!(
                    "Failed to bind UDP test probe: {}",
                    err
                )));
            }
        }

        let protocols = [
            ("quic", "quic"),
            ("enet", "enet"),
            ("kcp", "kcp"),
            ("raknet", "raknet"),
        ];

        for (protocol, scheme) in protocols {
            let (addr, server_task) = spawn_udp_echo_server(2, protocol).await?;
            let url = format!("{scheme}://{addr}");
            let args = parse_args(protocol, "arrival", &url)?;
            let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
            let (metrics_tx, mut metrics_rx) = mpsc::channel::<Metrics>(8);

            let sender_task = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None)?;
            let metric = wait_metric(&mut metrics_rx, protocol).await?;
            if metric.timed_out {
                return Err(AppError::validation(format!(
                    "Unexpected timeout for {}",
                    protocol
                )));
            }
            if metric.transport_error {
                return Err(AppError::validation(format!(
                    "Unexpected transport error for {}",
                    protocol
                )));
            }
            if metric.response_bytes == 0 {
                return Err(AppError::validation(format!(
                    "Expected response bytes for {}",
                    protocol
                )));
            }

            drop(shutdown_tx.send(()));
            join_handle(sender_task, protocol).await?;
            join_result_handle(server_task, protocol).await?;
        }
        Ok(())
    })
}

#[test]
fn mqtt_protocol_emits_success_metric() -> AppResult<()> {
    run_async_test(async {
        match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => {
                drop(listener);
            }
            Err(err) => {
                if permission_denied(&err) {
                    return Ok(());
                }
                return Err(AppError::validation(format!(
                    "Failed to bind MQTT test probe: {}",
                    err
                )));
            }
        }

        let (addr, server_task) = spawn_mqtt_mock_server().await?;
        let url = format!("mqtt://{addr}/devices/alpha");
        let args = parse_args("mqtt", "soak", &url)?;
        let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
        let (metrics_tx, mut metrics_rx) = mpsc::channel::<Metrics>(8);

        let sender_task = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None)?;
        let metric = wait_metric(&mut metrics_rx, "mqtt").await?;
        if metric.timed_out {
            return Err(AppError::validation("Unexpected timeout for mqtt"));
        }
        if metric.transport_error {
            return Err(AppError::validation("Unexpected transport error for mqtt"));
        }
        if metric.response_bytes == 0 {
            return Err(AppError::validation(
                "Expected non-zero response bytes for mqtt",
            ));
        }

        drop(shutdown_tx.send(()));
        join_handle(sender_task, "mqtt").await?;
        join_result_handle(server_task, "mqtt").await?;
        Ok(())
    })
}

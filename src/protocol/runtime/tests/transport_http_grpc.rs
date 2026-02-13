use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

use crate::error::{AppError, AppResult};
use crate::metrics::Metrics;

use super::{
    SHUTDOWN_CHANNEL_CAPACITY, join_handle, join_result_handle, parse_args, permission_denied,
    run_async_test, setup_request_sender, spawn_grpc_mock_server, spawn_http_mock_server,
    spawn_tcp_echo_server, spawn_websocket_mock_server, wait_metric,
};

#[test]
fn transport_and_http_protocols_emit_success_metric() -> AppResult<()> {
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
                    "Failed to bind TCP test probe: {}",
                    err
                )));
            }
        }

        let cases = [
            ("tcp", "arrival", "tcp", "tcp", 2_usize),
            ("websocket", "arrival", "ws", "websocket", 2_usize),
            ("http", "arrival", "http", "http", 2_usize),
        ];

        for (protocol, load_mode, scheme, label, expected_connections) in cases {
            let (addr, server_task) = match protocol {
                "tcp" => spawn_tcp_echo_server(expected_connections, protocol).await?,
                "websocket" => spawn_websocket_mock_server(expected_connections).await?,
                "http" => spawn_http_mock_server(expected_connections).await?,
                other => {
                    return Err(AppError::validation(format!(
                        "Unexpected protocol in test case: {}",
                        other
                    )));
                }
            };

            let url = format!("{scheme}://{addr}");
            let args = parse_args(protocol, load_mode, &url)?;
            let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
            let (metrics_tx, mut metrics_rx) = mpsc::channel::<Metrics>(8);

            let sender_task = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None)?;
            let metric = wait_metric(&mut metrics_rx, label).await?;
            if metric.timed_out {
                return Err(AppError::validation(format!(
                    "Unexpected timeout for {}",
                    label
                )));
            }
            if metric.transport_error {
                return Err(AppError::validation(format!(
                    "Unexpected transport error for {}",
                    label
                )));
            }
            if metric.response_bytes == 0 {
                return Err(AppError::validation(format!(
                    "Expected response bytes for {}",
                    label
                )));
            }

            drop(shutdown_tx.send(()));
            join_handle(sender_task, label).await?;
            join_result_handle(server_task, label).await?;
        }

        Ok(())
    })
}

#[test]
fn grpc_protocols_emit_success_metric() -> AppResult<()> {
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
                    "Failed to bind gRPC test probe: {}",
                    err
                )));
            }
        }

        let cases = [
            ("grpc-unary", "arrival", "grpc", "grpc-unary"),
            ("grpc-streaming", "arrival", "grpc", "grpc-streaming"),
        ];

        for (protocol, load_mode, scheme, label) in cases {
            let (addr, server_task) = spawn_grpc_mock_server(1).await?;
            let url = format!("{scheme}://{addr}/test.Service/Method");
            let args = parse_args(protocol, load_mode, &url)?;
            let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
            let (metrics_tx, mut metrics_rx) = mpsc::channel::<Metrics>(8);

            let sender_task = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None)?;
            let metric = wait_metric(&mut metrics_rx, label).await?;
            if metric.timed_out {
                return Err(AppError::validation(format!(
                    "Unexpected timeout for {}",
                    label
                )));
            }
            if metric.transport_error {
                return Err(AppError::validation(format!(
                    "Unexpected transport error for {}",
                    label
                )));
            }
            if metric.response_bytes == 0 {
                return Err(AppError::validation(format!(
                    "Expected response bytes for {}",
                    label
                )));
            }

            drop(shutdown_tx.send(()));
            join_handle(sender_task, label).await?;
            join_result_handle(server_task, label).await?;
        }

        Ok(())
    })
}

use std::future::Future;
use std::time::Duration;

use bytes::Bytes;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use http::Response;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ValidationError};
use crate::metrics::Metrics;

use super::resolve::resolve_endpoint;
use super::setup_request_sender;

mod datagram_mqtt;
mod scheme_resolution;
mod transport_http_grpc;

const SHUTDOWN_CHANNEL_CAPACITY: usize = 16;
const TEST_TIMEOUT: Duration = Duration::from_secs(2);

fn run_async_test<F>(future: F) -> AppResult<()>
where
    F: Future<Output = AppResult<()>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::validation(format!("Failed to build runtime: {}", err)))?;
    runtime.block_on(future)
}

fn permission_denied(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::PermissionDenied
}

fn parse_args(protocol: &str, load_mode: &str, url: &str) -> AppResult<TesterArgs> {
    TesterArgs::try_parse_from([
        "strest",
        "--url",
        url,
        "--protocol",
        protocol,
        "--load-mode",
        load_mode,
        "--requests",
        "1",
        "--max-tasks",
        "1",
        "--spawn-rate",
        "1",
        "--spawn-interval",
        "1",
        "--timeout",
        "1s",
        "--connect-timeout",
        "1s",
        "--data",
        "ping",
    ])
    .map_err(|err| AppError::validation(format!("Expected parse success: {}", err)))
}

fn validation_error(err: AppError) -> AppResult<ValidationError> {
    if let AppError::Validation(value) = err {
        Ok(value)
    } else {
        Err(AppError::validation(format!(
            "Expected validation error, got: {}",
            err
        )))
    }
}

async fn wait_metric(
    metrics_rx: &mut mpsc::Receiver<Metrics>,
    protocol: &str,
) -> AppResult<Metrics> {
    let received = timeout(TEST_TIMEOUT, metrics_rx.recv())
        .await
        .map_err(|_err| {
            AppError::validation(format!("Timed out waiting for metric for {}", protocol))
        })?;
    received.ok_or_else(|| AppError::validation(format!("Metric channel closed for {}", protocol)))
}

fn join_handle(handle: JoinHandle<()>, protocol: &str) -> impl Future<Output = AppResult<()>> {
    let protocol = protocol.to_owned();
    async move {
        timeout(TEST_TIMEOUT, handle)
            .await
            .map_err(|_err| AppError::validation(format!("Sender task timeout for {}", protocol)))?
            .map_err(|err| {
                AppError::validation(format!("Sender task failed for {}: {}", protocol, err))
            })
    }
}

fn join_result_handle(
    handle: JoinHandle<AppResult<()>>,
    protocol: &str,
) -> impl Future<Output = AppResult<()>> {
    let protocol = protocol.to_owned();
    async move {
        let output = timeout(TEST_TIMEOUT, handle)
            .await
            .map_err(|_err| AppError::validation(format!("Server task timeout for {}", protocol)))?
            .map_err(|err| {
                AppError::validation(format!("Server task failed for {}: {}", protocol, err))
            })?;
        output?;
        Ok(())
    }
}

async fn spawn_udp_echo_server(
    expected_packets: usize,
    protocol: &str,
) -> AppResult<(std::net::SocketAddr, JoinHandle<AppResult<()>>)> {
    let socket = UdpSocket::bind("127.0.0.1:0").await.map_err(|err| {
        AppError::validation(format!(
            "Failed to bind UDP server for {}: {}",
            protocol, err
        ))
    })?;
    let addr = socket.local_addr().map_err(|err| {
        AppError::validation(format!("Failed to read UDP addr for {}: {}", protocol, err))
    })?;
    let protocol_name = protocol.to_owned();

    let task = tokio::spawn(async move {
        let mut buf = [0_u8; 2048];
        for _ in 0..expected_packets {
            let (bytes, peer) = timeout(TEST_TIMEOUT, socket.recv_from(&mut buf))
                .await
                .map_err(|_err| {
                    AppError::validation(format!("UDP recv timeout for {}", protocol_name))
                })?
                .map_err(|err| {
                    AppError::validation(format!("UDP recv failed for {}: {}", protocol_name, err))
                })?;
            if bytes == 0 {
                return Err(AppError::validation(format!(
                    "UDP server received empty datagram for {}",
                    protocol_name
                )));
            }
            socket.send_to(b"ok", peer).await.map_err(|err| {
                AppError::validation(format!("UDP send failed for {}: {}", protocol_name, err))
            })?;
        }
        Ok(())
    });
    Ok((addr, task))
}

async fn spawn_mqtt_mock_server() -> AppResult<(std::net::SocketAddr, JoinHandle<AppResult<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| AppError::validation(format!("Failed to bind MQTT server: {}", err)))?;
    let addr = listener
        .local_addr()
        .map_err(|err| AppError::validation(format!("Failed to read MQTT addr: {}", err)))?;

    let task = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = timeout(TEST_TIMEOUT, listener.accept())
                .await
                .map_err(|_err| AppError::validation("MQTT accept timed out"))?
                .map_err(|err| AppError::validation(format!("MQTT accept failed: {}", err)))?;

            let mut connect_buf = [0_u8; 512];
            let connect_len = timeout(TEST_TIMEOUT, stream.read(&mut connect_buf))
                .await
                .map_err(|_err| AppError::validation("MQTT connect read timed out"))?
                .map_err(|err| {
                    AppError::validation(format!("MQTT connect read failed: {}", err))
                })?;
            if connect_len == 0 {
                return Err(AppError::validation("MQTT connect frame was empty"));
            }
            let packet_type = connect_buf.first().copied().unwrap_or(0) & 0xF0;
            if packet_type != 0x10 {
                return Err(AppError::validation(format!(
                    "Expected MQTT CONNECT packet, got 0x{packet_type:02x}"
                )));
            }

            stream
                .write_all(&[0x20, 0x02, 0x00, 0x00])
                .await
                .map_err(|err| {
                    AppError::validation(format!("MQTT connack write failed: {}", err))
                })?;

            let mut publish_buf = [0_u8; 512];
            let publish_len = timeout(TEST_TIMEOUT, stream.read(&mut publish_buf))
                .await
                .map_err(|_err| AppError::validation("MQTT publish read timed out"))?
                .map_err(|err| {
                    AppError::validation(format!("MQTT publish read failed: {}", err))
                })?;
            if publish_len == 0 {
                return Err(AppError::validation("MQTT publish frame was empty"));
            }
            let publish_type = publish_buf.first().copied().unwrap_or(0) & 0xF0;
            if publish_type != 0x30 {
                return Err(AppError::validation(format!(
                    "Expected MQTT PUBLISH packet, got 0x{publish_type:02x}"
                )));
            }
        }
        Ok(())
    });
    Ok((addr, task))
}

async fn spawn_tcp_echo_server(
    expected_connections: usize,
    protocol: &str,
) -> AppResult<(std::net::SocketAddr, JoinHandle<AppResult<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|err| {
        AppError::validation(format!(
            "Failed to bind TCP server for {}: {}",
            protocol, err
        ))
    })?;
    let addr = listener.local_addr().map_err(|err| {
        AppError::validation(format!("Failed to read TCP addr for {}: {}", protocol, err))
    })?;
    let protocol_name = protocol.to_owned();

    let task = tokio::spawn(async move {
        for _ in 0..expected_connections {
            let (mut stream, _) = timeout(TEST_TIMEOUT, listener.accept())
                .await
                .map_err(|_err| {
                    AppError::validation(format!("TCP accept timeout for {}", protocol_name))
                })?
                .map_err(|err| {
                    AppError::validation(format!(
                        "TCP accept failed for {}: {}",
                        protocol_name, err
                    ))
                })?;
            let mut buf = [0_u8; 2048];
            let _ = timeout(TEST_TIMEOUT, stream.read(&mut buf))
                .await
                .map_err(|_err| {
                    AppError::validation(format!("TCP read timeout for {}", protocol_name))
                })?
                .map_err(|err| {
                    AppError::validation(format!("TCP read failed for {}: {}", protocol_name, err))
                })?;
            timeout(TEST_TIMEOUT, stream.write_all(b"ok"))
                .await
                .map_err(|_err| {
                    AppError::validation(format!("TCP write timeout for {}", protocol_name))
                })?
                .map_err(|err| {
                    AppError::validation(format!("TCP write failed for {}: {}", protocol_name, err))
                })?;
        }
        Ok(())
    });
    Ok((addr, task))
}

async fn spawn_http_mock_server(
    expected_connections: usize,
) -> AppResult<(std::net::SocketAddr, JoinHandle<AppResult<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| AppError::validation(format!("Failed to bind HTTP server: {}", err)))?;
    let addr = listener
        .local_addr()
        .map_err(|err| AppError::validation(format!("Failed to read HTTP addr: {}", err)))?;

    let task = tokio::spawn(async move {
        for _ in 0..expected_connections {
            let (mut stream, _) = timeout(TEST_TIMEOUT, listener.accept())
                .await
                .map_err(|_err| AppError::validation("HTTP accept timed out"))?
                .map_err(|err| AppError::validation(format!("HTTP accept failed: {}", err)))?;
            let mut req = Vec::with_capacity(1024);
            loop {
                let mut chunk = [0_u8; 1024];
                let read = timeout(TEST_TIMEOUT, stream.read(&mut chunk))
                    .await
                    .map_err(|_err| AppError::validation("HTTP read timed out"))?
                    .map_err(|err| AppError::validation(format!("HTTP read failed: {}", err)))?;
                if read == 0 {
                    break;
                }
                let chunk_prefix = chunk.get(..read).ok_or_else(|| {
                    AppError::validation("HTTP server failed to access read buffer prefix")
                })?;
                req.extend_from_slice(chunk_prefix);
                if req.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }

            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
            timeout(TEST_TIMEOUT, stream.write_all(response))
                .await
                .map_err(|_err| AppError::validation("HTTP write timed out"))?
                .map_err(|err| AppError::validation(format!("HTTP write failed: {}", err)))?;
        }
        Ok(())
    });
    Ok((addr, task))
}

async fn spawn_websocket_mock_server(
    expected_connections: usize,
) -> AppResult<(std::net::SocketAddr, JoinHandle<AppResult<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| AppError::validation(format!("Failed to bind websocket server: {}", err)))?;
    let addr = listener.local_addr().map_err(|err| {
        AppError::validation(format!("Failed to read websocket server addr: {}", err))
    })?;

    let task = tokio::spawn(async move {
        for _ in 0..expected_connections {
            let (stream, _) = timeout(TEST_TIMEOUT, listener.accept())
                .await
                .map_err(|_err| AppError::validation("Websocket accept timed out"))?
                .map_err(|err| AppError::validation(format!("Websocket accept failed: {}", err)))?;

            let mut ws = timeout(TEST_TIMEOUT, accept_async(stream))
                .await
                .map_err(|_err| AppError::validation("Websocket handshake timed out"))?
                .map_err(|err| {
                    AppError::validation(format!("Websocket handshake failed: {}", err))
                })?;

            let incoming = timeout(TEST_TIMEOUT, ws.next())
                .await
                .map_err(|_err| AppError::validation("Websocket recv timed out"))?
                .ok_or_else(|| AppError::validation("Websocket stream closed unexpectedly"))?
                .map_err(|err| AppError::validation(format!("Websocket recv failed: {}", err)))?;
            if !incoming.is_text() && !incoming.is_binary() {
                return Err(AppError::validation("Unexpected websocket message type"));
            }

            timeout(TEST_TIMEOUT, ws.send(Message::Text("ok".to_owned())))
                .await
                .map_err(|_err| AppError::validation("Websocket send timed out"))?
                .map_err(|err| AppError::validation(format!("Websocket send failed: {}", err)))?;
        }
        Ok(())
    });
    Ok((addr, task))
}

fn grpc_frame(payload: &[u8]) -> Vec<u8> {
    let payload_len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
    let mut framed = Vec::with_capacity(payload.len().saturating_add(5));
    framed.push(0);
    framed.extend_from_slice(&payload_len.to_be_bytes());
    framed.extend_from_slice(payload);
    framed
}

async fn spawn_grpc_mock_server(
    expected_connections: usize,
) -> AppResult<(std::net::SocketAddr, JoinHandle<AppResult<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| AppError::validation(format!("Failed to bind gRPC server: {}", err)))?;
    let addr = listener
        .local_addr()
        .map_err(|err| AppError::validation(format!("Failed to read gRPC addr: {}", err)))?;

    let task = tokio::spawn(async move {
        for _ in 0..expected_connections {
            let (stream, _) = timeout(TEST_TIMEOUT, listener.accept())
                .await
                .map_err(|_err| AppError::validation("gRPC accept timed out"))?
                .map_err(|err| AppError::validation(format!("gRPC accept failed: {}", err)))?;
            let mut conn = h2::server::handshake(stream).await.map_err(|err| {
                AppError::validation(format!("gRPC h2 handshake failed: {}", err))
            })?;

            let req = timeout(TEST_TIMEOUT, conn.accept())
                .await
                .map_err(|_err| AppError::validation("gRPC request timed out"))?
                .ok_or_else(|| AppError::validation("gRPC stream closed unexpectedly"))?
                .map_err(|err| {
                    AppError::validation(format!("gRPC accept stream failed: {}", err))
                })?;

            let (_request, mut respond) = req;
            let response = Response::builder()
                .status(200)
                .header("content-type", "application/grpc")
                .header("grpc-status", "0")
                .body(())
                .map_err(|err| {
                    AppError::validation(format!("gRPC response build failed: {}", err))
                })?;

            let mut send = respond.send_response(response, false).map_err(|err| {
                AppError::validation(format!("gRPC send response failed: {}", err))
            })?;
            let data = grpc_frame(b"ok");
            send.send_data(Bytes::from(data), true)
                .map_err(|err| AppError::validation(format!("gRPC send data failed: {}", err)))?;
        }
        Ok(())
    });
    Ok((addr, task))
}

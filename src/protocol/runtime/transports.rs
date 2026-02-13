use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use super::types::RequestOutcome;

pub(super) async fn tcp_request_once(
    endpoint: SocketAddr,
    payload: &[u8],
    request_timeout: Duration,
    connect_timeout: Duration,
) -> RequestOutcome {
    let stream = match timeout(connect_timeout, TcpStream::connect(endpoint)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(_)) => return RequestOutcome::transport_error(),
        Err(_) => return RequestOutcome::timeout(),
    };

    let mut stream = stream;
    if !payload.is_empty() {
        match timeout(request_timeout, stream.write_all(payload)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return RequestOutcome::transport_error(),
            Err(_) => return RequestOutcome::timeout(),
        }
    }

    let mut buffer = [0_u8; 16 * 1024];
    match timeout(request_timeout, stream.read(&mut buffer)).await {
        Ok(Ok(bytes)) => RequestOutcome::success(u64::try_from(bytes).unwrap_or(u64::MAX)),
        Ok(Err(_)) => RequestOutcome::transport_error(),
        Err(_) => RequestOutcome::success(0),
    }
}

pub(super) async fn udp_request_once(
    endpoint: SocketAddr,
    payload: &[u8],
    request_timeout: Duration,
) -> RequestOutcome {
    let bind_addr = if endpoint.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = match UdpSocket::bind(bind_addr).await {
        Ok(socket) => socket,
        Err(_) => return RequestOutcome::transport_error(),
    };
    if socket.connect(endpoint).await.is_err() {
        return RequestOutcome::transport_error();
    }
    if socket.send(payload).await.is_err() {
        return RequestOutcome::transport_error();
    }

    let mut buffer = [0_u8; 16 * 1024];
    match timeout(request_timeout, socket.recv(&mut buffer)).await {
        Ok(Ok(bytes)) => RequestOutcome::success(u64::try_from(bytes).unwrap_or(u64::MAX)),
        Ok(Err(_)) => RequestOutcome::transport_error(),
        Err(_) => RequestOutcome::success(0),
    }
}

pub(super) async fn websocket_request_once(
    ws_url: &Url,
    payload: &str,
    request_timeout: Duration,
    connect_timeout: Duration,
) -> RequestOutcome {
    let connect = timeout(connect_timeout, connect_async(ws_url.as_str())).await;
    let (mut stream, _) = match connect {
        Ok(Ok(values)) => values,
        Ok(Err(_)) => return RequestOutcome::transport_error(),
        Err(_) => return RequestOutcome::timeout(),
    };

    if !payload.is_empty() {
        match timeout(
            request_timeout,
            stream.send(Message::Text(payload.to_owned())),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return RequestOutcome::transport_error(),
            Err(_) => return RequestOutcome::timeout(),
        }
    }

    let next_message = timeout(request_timeout, stream.next()).await;
    let response_bytes = match next_message {
        Ok(Some(Ok(message))) => message_bytes(&message),
        Ok(Some(Err(_))) => return RequestOutcome::transport_error(),
        Ok(None) => 0,
        Err(_) => 0,
    };

    drop(stream.close(None).await);
    RequestOutcome::success(response_bytes)
}

fn message_bytes(message: &Message) -> u64 {
    match message {
        Message::Text(value) => u64::try_from(value.len()).unwrap_or(u64::MAX),
        Message::Binary(value) => u64::try_from(value.len()).unwrap_or(u64::MAX),
        Message::Ping(value) => u64::try_from(value.len()).unwrap_or(u64::MAX),
        Message::Pong(value) => u64::try_from(value.len()).unwrap_or(u64::MAX),
        Message::Close(_) => 0,
        Message::Frame(_) => 0,
    }
}

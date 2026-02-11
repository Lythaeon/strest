use std::collections::HashMap;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{AppError, AppResult, DistributedError};

use super::control::{ControlError, ControlResponse};

pub(super) struct HttpRequest {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Vec<u8>,
}

pub(super) async fn read_http_request(socket: &mut TcpStream) -> Result<HttpRequest, ControlError> {
    const MAX_REQUEST_BYTES: usize = 1024 * 1024;
    let mut buffer: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let header_end;

    loop {
        let bytes = socket
            .read(&mut chunk)
            .await
            .map_err(|err| ControlError::new(400, format!("Failed to read request: {}", err)))?;
        if bytes == 0 {
            return Err(ControlError::new(400, "Empty request"));
        }
        let read_slice = chunk
            .get(..bytes)
            .ok_or_else(|| ControlError::new(400, "Invalid read length"))?;
        buffer.extend_from_slice(read_slice);
        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(ControlError::new(413, "Request too large"));
        }
        if let Some(pos) = find_header_end(&buffer) {
            header_end = pos;
            break;
        }
    }

    let header_bytes = buffer
        .get(..header_end)
        .ok_or_else(|| ControlError::new(400, "Malformed request headers"))?;
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|err| ControlError::new(400, format!("Invalid request encoding: {}", err)))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| ControlError::new(400, "Missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| ControlError::new(400, "Missing HTTP method"))?;
    let path = parts
        .next()
        .ok_or_else(|| ControlError::new(400, "Missing request path"))?;

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(ControlError::new(400, "Malformed header"));
        };
        headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_owned());
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end
        .checked_add(4)
        .ok_or_else(|| ControlError::new(400, "Malformed request headers"))?;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let bytes = socket
            .read(&mut chunk)
            .await
            .map_err(|err| ControlError::new(400, format!("Failed to read body: {}", err)))?;
        if bytes == 0 {
            break;
        }
        let read_slice = chunk
            .get(..bytes)
            .ok_or_else(|| ControlError::new(400, "Invalid read length"))?;
        body.extend_from_slice(read_slice);
        if body.len() > MAX_REQUEST_BYTES {
            return Err(ControlError::new(413, "Request body too large"));
        }
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

const fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    }
}

pub(super) async fn write_json_response(
    socket: &mut TcpStream,
    status: u16,
    response: &ControlResponse,
) -> AppResult<()> {
    let body = serde_json::to_vec(response).map_err(|err| {
        AppError::distributed(DistributedError::Serialize {
            context: "control response",
            source: err,
        })
    })?;
    write_response(socket, status, &body).await
}

pub(super) async fn write_error_response(
    socket: &mut TcpStream,
    status: u16,
    message: &str,
) -> AppResult<()> {
    #[derive(Serialize)]
    struct ErrorResponse<'msg> {
        error: &'msg str,
    }
    let body = serde_json::to_vec(&ErrorResponse { error: message }).map_err(|err| {
        AppError::distributed(DistributedError::Serialize {
            context: "control error response",
            source: err,
        })
    })?;
    write_response(socket, status, &body).await
}

async fn write_response(socket: &mut TcpStream, status: u16, body: &[u8]) -> AppResult<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        status_text(status),
        body.len()
    );
    socket.write_all(response.as_bytes()).await.map_err(|err| {
        AppError::distributed(DistributedError::Io {
            context: "write control response",
            source: err,
        })
    })?;
    socket.write_all(body).await.map_err(|err| {
        AppError::distributed(DistributedError::Io {
            context: "write control response body",
            source: err,
        })
    })?;
    Ok(())
}

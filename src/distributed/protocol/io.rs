use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::{AppError, AppResult, DistributedError};

use super::types::WireMessage;

pub(in crate::distributed) async fn read_message(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> AppResult<WireMessage> {
    const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
    let mut buffer: Vec<u8> = Vec::with_capacity(1024);
    let bytes = reader.read_until(b'\n', &mut buffer).await.map_err(|err| {
        AppError::distributed(DistributedError::Io {
            context: "read wire message",
            source: err,
        })
    })?;
    if bytes == 0 {
        return Err(AppError::distributed(DistributedError::ConnectionClosed));
    }
    if buffer.len() > MAX_MESSAGE_BYTES {
        return Err(AppError::distributed(
            DistributedError::WireMessageTooLarge {
                max_bytes: MAX_MESSAGE_BYTES,
            },
        ));
    }
    if buffer.ends_with(b"\n") {
        buffer.pop();
        if buffer.ends_with(b"\r") {
            buffer.pop();
        }
    }
    let line = std::str::from_utf8(&buffer).map_err(|err| {
        AppError::distributed(DistributedError::WireMessageInvalidUtf8 { source: err })
    })?;
    serde_json::from_str::<WireMessage>(line).map_err(|err| {
        AppError::distributed(DistributedError::Deserialize {
            context: "wire message",
            source: err,
        })
    })
}

pub(in crate::distributed) async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    message: &WireMessage,
) -> AppResult<()> {
    let mut payload = serde_json::to_string(message).map_err(|err| {
        AppError::distributed(DistributedError::Serialize {
            context: "wire message",
            source: err,
        })
    })?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.map_err(|err| {
        AppError::distributed(DistributedError::Io {
            context: "send wire message",
            source: err,
        })
    })
}

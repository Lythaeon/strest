use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::info;

use crate::error::{AppError, AppResult, DistributedError};

use super::super::protocol::{ErrorMessage, WireMessage, read_message, send_message};

pub(super) struct AgentConn {
    pub(super) agent_id: String,
    pub(super) weight: u64,
    pub(super) reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    pub(super) writer: tokio::net::tcp::OwnedWriteHalf,
}

const AGENT_HELLO_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) async fn accept_agent(
    stream: TcpStream,
    auth_token: Option<&str>,
) -> AppResult<AgentConn> {
    let peer = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "<unknown>".to_owned());
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let hello = match timeout(AGENT_HELLO_TIMEOUT, read_message(&mut reader)).await {
        Ok(result) => match result? {
            WireMessage::Hello(message) => message,
            WireMessage::Error(message) => {
                return Err(AppError::distributed(DistributedError::Remote {
                    message: message.message,
                }));
            }
            WireMessage::Config(_)
            | WireMessage::Start(_)
            | WireMessage::Stop(_)
            | WireMessage::Heartbeat(_)
            | WireMessage::Report(_)
            | WireMessage::Stream(_) => {
                return Err(AppError::distributed(
                    DistributedError::ExpectedHelloFromAgent,
                ));
            }
        },
        Err(_) => {
            return Err(AppError::distributed(DistributedError::AgentHelloTimeout));
        }
    };

    if let Some(expected) = auth_token {
        let provided = hello.auth_token.as_deref().unwrap_or("");
        if provided != expected {
            let mut writer = write_half;
            send_message(
                &mut writer,
                &WireMessage::Error(ErrorMessage {
                    message: "Invalid auth token.".to_owned(),
                }),
            )
            .await?;
            return Err(AppError::distributed(DistributedError::InvalidAuthToken));
        }
    }

    info!(
        "Accepted agent {} from {} (weight={})",
        hello.agent_id, peer, hello.weight
    );
    Ok(AgentConn {
        agent_id: hello.agent_id,
        weight: hello.weight.max(1),
        reader,
        writer: write_half,
    })
}

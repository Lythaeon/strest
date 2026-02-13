use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{CONTENT_TYPE, HeaderMap};
use tokio::time::timeout;
use url::Url;

use crate::error::{AppError, AppResult, HttpError};

use super::types::RequestOutcome;

pub(super) fn build_grpc_client(
    connect_timeout: Duration,
    prior_knowledge: bool,
) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .http2_adaptive_window(true)
        .tcp_nodelay(true);
    if prior_knowledge {
        builder = builder.http2_prior_knowledge();
    }
    builder
        .build()
        .map_err(|source| AppError::http(HttpError::BuildClientFailed { source }))
}

pub(super) async fn grpc_request_once(
    client: &reqwest::Client,
    grpc_url: &Url,
    payload: &[u8],
    request_timeout: Duration,
    streaming: bool,
) -> RequestOutcome {
    let request_future = async {
        let response = match client
            .post(grpc_url.as_str())
            .header(CONTENT_TYPE, "application/grpc")
            .header("te", "trailers")
            .body(payload.to_vec())
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => return RequestOutcome::transport_error(),
        };

        if !response.status().is_success() {
            return RequestOutcome::transport_error();
        }
        if grpc_status_non_zero(response.headers()) {
            return RequestOutcome::transport_error();
        }

        let response_bytes = if streaming {
            let mut total_bytes: u64 = 0;
            let mut body_stream = response.bytes_stream();
            while let Some(item) = body_stream.next().await {
                let chunk = match item {
                    Ok(chunk) => chunk,
                    Err(_) => return RequestOutcome::transport_error(),
                };
                let chunk_len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
                total_bytes = total_bytes.saturating_add(chunk_len);
                if total_bytes > 0 {
                    break;
                }
            }
            total_bytes
        } else {
            match response.bytes().await {
                Ok(body) => u64::try_from(body.len()).unwrap_or(u64::MAX),
                Err(_) => return RequestOutcome::transport_error(),
            }
        };

        RequestOutcome::success(response_bytes)
    };

    timeout(request_timeout, request_future)
        .await
        .unwrap_or_else(|_| RequestOutcome::timeout())
}

pub(super) fn grpc_frame(payload: &[u8]) -> Vec<u8> {
    let payload_len = u32::try_from(payload.len()).map_or(u32::MAX, |value| value);
    let mut framed = Vec::with_capacity(payload.len().saturating_add(5));
    framed.push(0);
    framed.extend_from_slice(&payload_len.to_be_bytes());
    framed.extend_from_slice(payload);
    framed
}

fn grpc_status_non_zero(headers: &HeaderMap) -> bool {
    let Some(raw) = headers.get("grpc-status") else {
        return false;
    };
    let Ok(value) = raw.to_str() else {
        return true;
    };
    value.trim() != "0"
}

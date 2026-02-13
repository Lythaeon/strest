use futures_util::StreamExt;
use reqwest::{Client, Request};
use tracing::error;

#[derive(Debug)]
pub(super) struct RequestOutcome {
    pub(super) status: u16,
    pub(super) success: bool,
    pub(super) timed_out: bool,
    pub(super) transport_error: bool,
    pub(super) response_bytes: u64,
}

pub(super) async fn execute_request_with_asserts(
    client: &Client,
    request: Request,
    expected_status_code: u16,
    assert_status: Option<u16>,
    assert_body_contains: Option<&str>,
) -> RequestOutcome {
    match client.execute(request).await {
        Ok(response) => {
            let status = response.status().as_u16();
            let expected = assert_status.unwrap_or(expected_status_code);
            let status_ok = status == expected;

            let body_result = match assert_body_contains {
                Some(fragment) => drain_body_contains(response, fragment).await,
                None => drain_response_body(response)
                    .await
                    .map(|bytes| (true, bytes)),
            };
            let mut timed_out = false;
            let mut transport_error = false;
            let (body_ok, response_bytes) = match (assert_body_contains, body_result) {
                (Some(_), Ok((found, bytes))) => (found, bytes),
                (Some(_), Err(err)) => {
                    timed_out = err.is_timeout();
                    transport_error = !timed_out;
                    error!("Failed to read response body: {}", err);
                    (false, 0)
                }
                (None, Ok((found, bytes))) => (found, bytes),
                (None, Err(err)) => {
                    timed_out = err.is_timeout();
                    transport_error = !timed_out;
                    error!("Failed to read response body: {}", err);
                    (false, 0)
                }
            };

            RequestOutcome {
                status,
                success: status_ok && body_ok,
                timed_out,
                transport_error,
                response_bytes,
            }
        }
        Err(err) => {
            error!("Request failed: {}", err);
            let timed_out = err.is_timeout();
            RequestOutcome {
                status: 500,
                success: false,
                timed_out,
                transport_error: !timed_out,
                response_bytes: 0,
            }
        }
    }
}

pub(super) async fn execute_request(
    client: &Client,
    request: Request,
    drain_body: bool,
) -> Result<u16, reqwest::Error> {
    let response = client.execute(request).await?;
    let status = response.status().as_u16();
    if drain_body {
        let _ = drain_response_body(response).await?;
    }
    Ok(status)
}

pub(super) async fn execute_request_status(
    client: &Client,
    request: Request,
) -> (u16, bool, bool, u64) {
    match client.execute(request).await {
        Ok(response) => {
            let status = response.status().as_u16();
            match drain_response_body(response).await {
                Ok(bytes) => (status, false, false, bytes),
                Err(err) => {
                    let timed_out = err.is_timeout();
                    (500, timed_out, !timed_out, 0)
                }
            }
        }
        Err(err) => {
            let timed_out = err.is_timeout();
            (500, timed_out, !timed_out, 0)
        }
    }
}

async fn drain_response_body(response: reqwest::Response) -> Result<u64, reqwest::Error> {
    let mut stream = response.bytes_stream();
    let mut total_bytes: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        total_bytes = total_bytes.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    }
    Ok(total_bytes)
}

async fn drain_body_contains(
    response: reqwest::Response,
    fragment: &str,
) -> Result<(bool, u64), reqwest::Error> {
    let needle = fragment.as_bytes();
    if needle.is_empty() {
        let bytes = drain_response_body(response).await?;
        return Ok((true, bytes));
    }
    let mut found = false;
    let mut carry: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    let mut total_bytes: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        total_bytes = total_bytes.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if !found {
            let mut window = std::mem::take(&mut carry);
            window.extend_from_slice(&bytes);
            if window.windows(needle.len()).any(|slice| slice == needle) {
                found = true;
            }
            let keep = needle.len().saturating_sub(1);
            if keep > 0 {
                let len = window.len();
                let start = len.saturating_sub(keep);
                if let Some(tail) = window.get(start..) {
                    carry = tail.to_vec();
                } else {
                    carry.clear();
                }
            } else {
                carry.clear();
            }
        }
    }
    Ok((found, total_bytes))
}

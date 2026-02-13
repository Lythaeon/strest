use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use super::super::control::{ControlCommand, ControlError, ControlStartRequest};
use super::super::http::{read_http_request, write_error_response, write_json_response};

pub(super) async fn handle_control_connection(
    mut socket: TcpStream,
    auth_token: Option<&str>,
    control_tx: mpsc::UnboundedSender<ControlCommand>,
) {
    let request = match read_http_request(&mut socket).await {
        Ok(request) => request,
        Err(err) => {
            if write_error_response(&mut socket, err.status, &err.message)
                .await
                .is_err()
            {
                // Socket closed while writing error response.
            }
            return;
        }
    };

    if let Some(token) = auth_token {
        let header = request
            .headers
            .get("authorization")
            .map(|value| value.trim().to_owned());
        let expected = format!("Bearer {}", token);
        if header.as_deref() != Some(expected.as_str()) {
            if write_error_response(&mut socket, 401, "Unauthorized")
                .await
                .is_err()
            {
                // Socket closed while writing error response.
            }
            return;
        }
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/start") => {
            let start_request = if request.body.is_empty() {
                ControlStartRequest::default()
            } else {
                match serde_json::from_slice::<ControlStartRequest>(&request.body) {
                    Ok(value) => value,
                    Err(err) => {
                        if write_error_response(&mut socket, 400, &format!("Invalid JSON: {}", err))
                            .await
                            .is_err()
                        {
                            // Socket closed while writing error response.
                        }
                        return;
                    }
                }
            };

            let (respond_to, response_rx) = oneshot::channel();
            if control_tx
                .send(ControlCommand::Start {
                    request: start_request,
                    respond_to,
                })
                .is_err()
            {
                if write_error_response(&mut socket, 503, "Controller unavailable")
                    .await
                    .is_err()
                {
                    // Socket closed while writing error response.
                }
                return;
            }

            let response = tokio::time::timeout(Duration::from_secs(5), response_rx)
                .await
                .map_or_else(
                    |_| Err(ControlError::new(504, "Controller response timed out")),
                    |result| {
                        result.unwrap_or_else(|_| {
                            Err(ControlError::new(503, "Controller unavailable"))
                        })
                    },
                );

            match response {
                Ok(response) => {
                    if write_json_response(&mut socket, 200, &response)
                        .await
                        .is_err()
                    {
                        // Socket closed while writing response.
                    }
                }
                Err(err) => {
                    if write_error_response(&mut socket, err.status, &err.message)
                        .await
                        .is_err()
                    {
                        // Socket closed while writing error response.
                    }
                }
            }
        }
        ("POST", "/stop") => {
            let (respond_to, response_rx) = oneshot::channel();
            if control_tx
                .send(ControlCommand::Stop { respond_to })
                .is_err()
            {
                if write_error_response(&mut socket, 503, "Controller unavailable")
                    .await
                    .is_err()
                {
                    // Socket closed while writing error response.
                }
                return;
            }

            let response = tokio::time::timeout(Duration::from_secs(5), response_rx)
                .await
                .map_or_else(
                    |_| Err(ControlError::new(504, "Controller response timed out")),
                    |result| {
                        result.unwrap_or_else(|_| {
                            Err(ControlError::new(503, "Controller unavailable"))
                        })
                    },
                );

            match response {
                Ok(response) => {
                    if write_json_response(&mut socket, 200, &response)
                        .await
                        .is_err()
                    {
                        // Socket closed while writing response.
                    }
                }
                Err(err) => {
                    if write_error_response(&mut socket, err.status, &err.message)
                        .await
                        .is_err()
                    {
                        // Socket closed while writing error response.
                    }
                }
            }
        }
        _ => {
            if write_error_response(&mut socket, 404, "Not found")
                .await
                .is_err()
            {
                // Socket closed while writing error response.
            }
        }
    }
}

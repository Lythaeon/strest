use tokio::sync::{broadcast, mpsc};

use crate::error::{AppError, AppResult, ValidationError};
use crate::metrics::Metrics;

use super::{
    SHUTDOWN_CHANNEL_CAPACITY, parse_args, resolve_endpoint, run_async_test, setup_request_sender,
    validation_error,
};

#[test]
fn planned_protocols_resolve_expected_default_ports() -> AppResult<()> {
    run_async_test(async {
        let cases = [
            ("quic", "arrival", "quic://localhost", 4433_u16),
            ("enet", "arrival", "enet://localhost", 7777_u16),
            ("kcp", "arrival", "kcp://localhost", 4000_u16),
            ("raknet", "arrival", "raknet://localhost", 19132_u16),
            ("mqtt", "soak", "mqtt://localhost/topic", 1883_u16),
        ];

        for (protocol, load_mode, url, expected_port) in cases {
            let args = parse_args(protocol, load_mode, url)?;
            let allowed_schemes = match protocol {
                "quic" => &[("quic", 4433), ("udp", 4433), ("http", 80), ("https", 443)][..],
                "enet" => &[("enet", 7777), ("udp", 7777), ("http", 80), ("https", 443)][..],
                "kcp" => &[("kcp", 4000), ("udp", 4000), ("http", 80), ("https", 443)][..],
                "raknet" => &[
                    ("raknet", 19132),
                    ("udp", 19132),
                    ("http", 80),
                    ("https", 443),
                ][..],
                "mqtt" => &[("mqtt", 1883), ("tcp", 1883), ("http", 80)][..],
                _ => {
                    return Err(AppError::validation(format!(
                        "Unexpected protocol in test case: {}",
                        protocol
                    )));
                }
            };
            let endpoint = resolve_endpoint(&args, allowed_schemes)?;
            if endpoint.port() != expected_port {
                return Err(AppError::validation(format!(
                    "Expected default port {} for {}, got {}",
                    expected_port,
                    protocol,
                    endpoint.port()
                )));
            }
        }
        Ok(())
    })
}

#[test]
fn planned_protocols_reject_unsupported_url_schemes() -> AppResult<()> {
    run_async_test(async {
        let cases = [
            ("quic", "arrival", "mqtt://localhost:1883", "mqtt"),
            ("enet", "arrival", "tcp://localhost:7777", "tcp"),
            ("kcp", "arrival", "ws://localhost:4000", "ws"),
            ("raknet", "arrival", "mqtt://localhost:19132", "mqtt"),
            ("mqtt", "soak", "udp://localhost:1883", "udp"),
        ];

        for (protocol, load_mode, url, scheme) in cases {
            let args = parse_args(protocol, load_mode, url)?;
            let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
            let (metrics_tx, _metrics_rx) = mpsc::channel::<Metrics>(2);
            let err = match setup_request_sender(&args, &shutdown_tx, &metrics_tx, None) {
                Ok(_) => {
                    return Err(AppError::validation(
                        "Expected unsupported scheme error but sender setup succeeded",
                    ));
                }
                Err(err) => err,
            };
            let validation = validation_error(err)?;
            let ValidationError::UnsupportedProtocolUrlScheme {
                protocol: found_protocol,
                scheme: found_scheme,
            } = validation
            else {
                return Err(AppError::validation(format!(
                    "Expected UnsupportedProtocolUrlScheme, got {}",
                    validation
                )));
            };
            if found_protocol != protocol {
                return Err(AppError::validation(format!(
                    "Expected protocol {}, got {}",
                    protocol, found_protocol
                )));
            }
            if found_scheme != scheme {
                return Err(AppError::validation(format!(
                    "Expected scheme {}, got {}",
                    scheme, found_scheme
                )));
            }
        }

        Ok(())
    })
}

#[test]
fn grpc_protocols_accept_grpc_scheme_aliases_in_endpoint_resolution() -> AppResult<()> {
    run_async_test(async {
        let cases = [
            (
                "grpc-unary",
                "arrival",
                "grpc://localhost/test.Service/Method",
                80_u16,
            ),
            (
                "grpc-streaming",
                "arrival",
                "grpcs://localhost/test.Service/Method",
                443_u16,
            ),
        ];

        for (protocol, load_mode, url, expected_port) in cases {
            let args = parse_args(protocol, load_mode, url)?;
            let endpoint = resolve_endpoint(&args, &[("http", 80), ("https", 443)])?;
            if endpoint.port() != expected_port {
                return Err(AppError::validation(format!(
                    "Expected default port {} for {}, got {}",
                    expected_port,
                    protocol,
                    endpoint.port()
                )));
            }
        }

        Ok(())
    })
}

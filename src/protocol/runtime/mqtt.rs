use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::types::RequestOutcome;

const MQTT_PROTOCOL_NAME: &str = "MQTT";
const MQTT_PROTOCOL_LEVEL_3_1_1: u8 = 4;
const MQTT_CONNECT_PACKET_TYPE: u8 = 0x10;
const MQTT_PUBLISH_PACKET_TYPE_QOS0: u8 = 0x30;
const MQTT_CONNACK_PACKET_TYPE: u8 = 0x20;
const MQTT_CLEAN_SESSION_FLAG: u8 = 0x02;
const MQTT_KEEPALIVE_SECS: u16 = 60;

pub(super) async fn mqtt_request_once(
    endpoint: SocketAddr,
    topic: &str,
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

    let connect_packet = build_connect_packet("strest");
    match timeout(request_timeout, stream.write_all(&connect_packet)).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return RequestOutcome::transport_error(),
        Err(_) => return RequestOutcome::timeout(),
    }

    let mut connack = [0_u8; 4];
    match timeout(request_timeout, stream.read_exact(&mut connack)).await {
        Ok(Ok(_)) => {}
        Ok(Err(_)) => return RequestOutcome::transport_error(),
        Err(_) => return RequestOutcome::timeout(),
    }
    if !is_connack_ok(connack) {
        return RequestOutcome::transport_error();
    }

    if !payload.is_empty() {
        let publish_packet = build_publish_packet(topic, payload);
        match timeout(request_timeout, stream.write_all(&publish_packet)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return RequestOutcome::transport_error(),
            Err(_) => return RequestOutcome::timeout(),
        }
    }

    RequestOutcome::success(4)
}

pub(super) fn topic_from_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "strest/loadtest".to_owned();
    }
    trimmed.trim_start_matches('/').to_owned()
}

fn build_connect_packet(client_id: &str) -> Vec<u8> {
    let mut variable_header = Vec::with_capacity(10);
    push_utf8(&mut variable_header, MQTT_PROTOCOL_NAME);
    variable_header.push(MQTT_PROTOCOL_LEVEL_3_1_1);
    variable_header.push(MQTT_CLEAN_SESSION_FLAG);
    variable_header.extend_from_slice(&MQTT_KEEPALIVE_SECS.to_be_bytes());

    let mut payload = Vec::new();
    push_utf8(&mut payload, client_id);

    let remaining_len = variable_header.len().saturating_add(payload.len());
    let capacity = 1_usize.saturating_add(remaining_len).saturating_add(4);
    let mut packet = Vec::with_capacity(capacity);
    packet.push(MQTT_CONNECT_PACKET_TYPE);
    encode_remaining_length(&mut packet, remaining_len);
    packet.extend_from_slice(&variable_header);
    packet.extend_from_slice(&payload);
    packet
}

fn build_publish_packet(topic: &str, payload: &[u8]) -> Vec<u8> {
    let topic_len = topic.len();
    let remaining_len = 2_usize
        .saturating_add(topic_len)
        .saturating_add(payload.len());
    let capacity = 1_usize.saturating_add(remaining_len).saturating_add(4);
    let mut packet = Vec::with_capacity(capacity);
    packet.push(MQTT_PUBLISH_PACKET_TYPE_QOS0);
    encode_remaining_length(&mut packet, remaining_len);
    push_utf8(&mut packet, topic);
    packet.extend_from_slice(payload);
    packet
}

fn push_utf8(buffer: &mut Vec<u8>, value: &str) {
    let max_len = usize::from(u16::MAX);
    let len = value.len().min(max_len);
    let len_u16 = u16::try_from(len).unwrap_or(u16::MAX);
    buffer.extend_from_slice(&len_u16.to_be_bytes());
    if let Some(slice) = value.as_bytes().get(..len) {
        buffer.extend_from_slice(slice);
    }
}

fn encode_remaining_length(out: &mut Vec<u8>, mut len: usize) {
    loop {
        let mut byte = u8::try_from(len % 128).unwrap_or(127);
        len /= 128;
        if len > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if len == 0 {
            break;
        }
    }
}

const fn is_connack_ok(buffer: [u8; 4]) -> bool {
    buffer[0] == MQTT_CONNACK_PACKET_TYPE && buffer[1] == 0x02 && buffer[3] == 0x00
}

#[cfg(test)]
mod tests {
    use super::topic_from_path;

    #[test]
    fn topic_from_path_normalizes_default() {
        assert_eq!(topic_from_path(""), "strest/loadtest");
        assert_eq!(topic_from_path("/"), "strest/loadtest");
    }

    #[test]
    fn topic_from_path_trims_leading_slash() {
        assert_eq!(topic_from_path("/devices/one"), "devices/one");
    }
}

use super::*;
use clap::Parser;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn parse_header_valid() -> Result<(), String> {
    let parsed = parse_header("Content-Type: application/json");
    match parsed {
        Ok((key, value)) => {
            if key != "Content-Type" {
                return Err(format!("Unexpected key: {}", key));
            }
            if value != "application/json" {
                return Err(format!("Unexpected value: {}", value));
            }
            Ok(())
        }
        Err(err) => Err(format!("Expected Ok, got Err: {}", err)),
    }
}

#[test]
fn parse_header_invalid() -> Result<(), String> {
    let parsed = parse_header("MissingDelimiter");
    if parsed.is_err() {
        Ok(())
    } else {
        Err("Expected Err for invalid header".to_owned())
    }
}

#[test]
fn parse_args_defaults() -> Result<(), String> {
    let args_result = TesterArgs::try_parse_from(["strest", "-u", "http://localhost"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };

    if !matches!(args.method, HttpMethod::Get) {
        return Err("Expected HttpMethod::Get".to_owned());
    }
    if args.url.as_deref() != Some("http://localhost") {
        return Err("Unexpected url".to_owned());
    }
    if args.accept_header.is_some() {
        return Err("Expected accept_header to be None".to_owned());
    }
    if args.content_type.is_some() {
        return Err("Expected content_type to be None".to_owned());
    }
    if args.target_duration.get() != 30 {
        return Err(format!(
            "Unexpected target_duration: {}",
            args.target_duration.get()
        ));
    }
    if args.requests.is_some() {
        return Err("Expected requests to be None".to_owned());
    }
    if args.expected_status_code != 200 {
        return Err(format!(
            "Unexpected expected_status_code: {}",
            args.expected_status_code
        ));
    }
    if args.request_timeout != Duration::from_secs(10) {
        return Err(format!(
            "Unexpected request_timeout: {:?}",
            args.request_timeout
        ));
    }
    if args.redirect_limit != 10 {
        return Err(format!(
            "Unexpected redirect_limit: {}",
            args.redirect_limit
        ));
    }
    if args.disable_keepalive {
        return Err("Expected disable_keepalive to be false".to_owned());
    }
    if args.disable_compression {
        return Err("Expected disable_compression to be false".to_owned());
    }
    if args.http_version.is_some() {
        return Err("Expected http_version to be None".to_owned());
    }
    if args.connect_timeout != Duration::from_secs(5) {
        return Err(format!(
            "Unexpected connect_timeout: {:?}",
            args.connect_timeout
        ));
    }
    let expected_charts = default_charts_path();
    if args.charts_path != expected_charts {
        return Err(format!("Unexpected charts_path: {}", args.charts_path));
    }
    if args.no_charts {
        return Err("Expected no_charts to be false".to_owned());
    }
    if args.no_ua {
        return Err("Expected no_ua to be false".to_owned());
    }
    if args.authorized {
        return Err("Expected authorized to be false".to_owned());
    }
    if args.data_file.is_some() {
        return Err("Expected data_file to be None".to_owned());
    }
    if args.data_lines.is_some() {
        return Err("Expected data_lines to be None".to_owned());
    }
    if args.verbose {
        return Err("Expected verbose to be false".to_owned());
    }
    if args.config.is_some() {
        return Err("Expected config to be None".to_owned());
    }
    let expected_tmp = default_tmp_path();
    if args.tmp_path != expected_tmp {
        return Err(format!("Unexpected tmp_path: {}", args.tmp_path));
    }
    if args.no_ui {
        return Err("Expected no_ui to be false".to_owned());
    }
    if args.ui_window_ms.get() != 10_000 {
        return Err(format!(
            "Unexpected ui_window_ms: {}",
            args.ui_window_ms.get()
        ));
    }
    if args.summary {
        return Err("Expected summary to be false".to_owned());
    }
    if args.keep_tmp {
        return Err("Expected keep_tmp to be false".to_owned());
    }
    if args.warmup.is_some() {
        return Err("Expected warmup to be None".to_owned());
    }
    if args.tls_min.is_some() {
        return Err("Expected tls_min to be None".to_owned());
    }
    if args.tls_max.is_some() {
        return Err("Expected tls_max to be None".to_owned());
    }
    if args.http2 {
        return Err("Expected http2 to be false".to_owned());
    }
    if !args.alpn.is_empty() {
        return Err("Expected alpn to be empty".to_owned());
    }
    if !args.proxy_headers.is_empty() {
        return Err("Expected proxy_headers to be empty".to_owned());
    }
    if args.proxy_http_version.is_some() {
        return Err("Expected proxy_http_version to be None".to_owned());
    }
    if args.proxy_http2 {
        return Err("Expected proxy_http2 to be false".to_owned());
    }
    if args.export_csv.is_some() {
        return Err("Expected export_csv to be None".to_owned());
    }
    if args.export_json.is_some() {
        return Err("Expected export_json to be None".to_owned());
    }
    if args.export_jsonl.is_some() {
        return Err("Expected export_jsonl to be None".to_owned());
    }
    if args.log_shards.get() != 1 {
        return Err(format!("Unexpected log_shards: {}", args.log_shards.get()));
    }
    if args.max_tasks.get() != 1000 {
        return Err(format!("Unexpected max_tasks: {}", args.max_tasks.get()));
    }
    if args.spawn_rate_per_tick.get() != 1 {
        return Err(format!(
            "Unexpected spawn_rate_per_tick: {}",
            args.spawn_rate_per_tick.get()
        ));
    }
    if args.tick_interval.get() != 100 {
        return Err(format!(
            "Unexpected tick_interval: {}",
            args.tick_interval.get()
        ));
    }
    if args.rate_limit.is_some() {
        return Err("Expected rate_limit to be None".to_owned());
    }
    if !args.connect_to.is_empty() {
        return Err("Expected connect_to to be empty".to_owned());
    }
    if args.host_header.is_some() {
        return Err("Expected host_header to be None".to_owned());
    }
    if args.ipv6_only {
        return Err("Expected ipv6_only to be false".to_owned());
    }
    if args.ipv4_only {
        return Err("Expected ipv4_only to be false".to_owned());
    }
    if args.no_pre_lookup {
        return Err("Expected no_pre_lookup to be false".to_owned());
    }
    if args.no_color {
        return Err("Expected no_color to be false".to_owned());
    }
    if args.ui_fps != 16 {
        return Err(format!("Unexpected ui_fps: {}", args.ui_fps));
    }
    if args.stats_success_breakdown {
        return Err("Expected stats_success_breakdown to be false".to_owned());
    }
    if args.unix_socket.is_some() {
        return Err("Expected unix_socket to be None".to_owned());
    }
    if args.load_profile.is_some() {
        return Err("Expected load_profile to be None".to_owned());
    }
    if args.controller_listen.is_some() {
        return Err("Expected controller_listen to be None".to_owned());
    }
    if args.controller_mode != ControllerMode::Auto {
        return Err("Expected controller_mode to be auto".to_owned());
    }
    if args.control_listen.is_some() {
        return Err("Expected control_listen to be None".to_owned());
    }
    if args.control_auth_token.is_some() {
        return Err("Expected control_auth_token to be None".to_owned());
    }
    if args.agent_join.is_some() {
        return Err("Expected agent_join to be None".to_owned());
    }
    if args.auth_token.is_some() {
        return Err("Expected auth_token to be None".to_owned());
    }
    if args.agent_id.is_some() {
        return Err("Expected agent_id to be None".to_owned());
    }
    if args.agent_weight.get() != 1 {
        return Err(format!(
            "Unexpected agent_weight: {}",
            args.agent_weight.get()
        ));
    }
    if args.min_agents.get() != 1 {
        return Err(format!("Unexpected min_agents: {}", args.min_agents.get()));
    }
    if args.agent_wait_timeout_ms.is_some() {
        return Err("Expected agent_wait_timeout_ms to be None".to_owned());
    }
    if args.agent_standby {
        return Err("Expected agent_standby to be false".to_owned());
    }
    if args.agent_reconnect_ms.get() != 1000 {
        return Err(format!(
            "Unexpected agent_reconnect_ms: {}",
            args.agent_reconnect_ms.get()
        ));
    }
    if args.agent_heartbeat_interval_ms.get() != 1000 {
        return Err(format!(
            "Unexpected agent_heartbeat_interval_ms: {}",
            args.agent_heartbeat_interval_ms.get()
        ));
    }
    if args.agent_heartbeat_timeout_ms.get() != 3000 {
        return Err(format!(
            "Unexpected agent_heartbeat_timeout_ms: {}",
            args.agent_heartbeat_timeout_ms.get()
        ));
    }
    if args.distributed_stream_interval_ms.is_some() {
        return Err("Expected distributed_stream_interval_ms to be None".to_owned());
    }
    if args.replay {
        return Err("Expected replay to be false".to_owned());
    }
    if args.replay_start.is_some() {
        return Err("Expected replay_start to be None".to_owned());
    }
    if args.replay_end.is_some() {
        return Err("Expected replay_end to be None".to_owned());
    }
    if args.replay_step.is_some() {
        return Err("Expected replay_step to be None".to_owned());
    }
    if args.replay_snapshot_interval.is_some() {
        return Err("Expected replay_snapshot_interval to be None".to_owned());
    }
    if args.replay_snapshot_start.is_some() {
        return Err("Expected replay_snapshot_start to be None".to_owned());
    }
    if args.replay_snapshot_end.is_some() {
        return Err("Expected replay_snapshot_end to be None".to_owned());
    }
    if args.replay_snapshot_out.is_some() {
        return Err("Expected replay_snapshot_out to be None".to_owned());
    }
    if args.replay_snapshot_format != "json" {
        return Err(format!(
            "Unexpected replay_snapshot_format: {}",
            args.replay_snapshot_format
        ));
    }
    if args.scenario.is_some() {
        return Err("Expected scenario to be None".to_owned());
    }
    if args.sinks.is_some() {
        return Err("Expected sinks to be None".to_owned());
    }
    if args.distributed_silent {
        return Err("Expected distributed_silent to be false".to_owned());
    }
    if args.distributed_stream_summaries {
        return Err("Expected distributed_stream_summaries to be false".to_owned());
    }
    if args.http3 {
        return Err("Expected http3 to be false".to_owned());
    }
    if args.install_service {
        return Err("Expected install_service to be false".to_owned());
    }
    if args.uninstall_service {
        return Err("Expected uninstall_service to be false".to_owned());
    }
    if args.service_name.is_some() {
        return Err("Expected service_name to be None".to_owned());
    }
    if args.metrics_max.get() != 1_000_000 {
        return Err(format!(
            "Unexpected metrics_max: {}",
            args.metrics_max.get()
        ));
    }

    Ok(())
}

#[test]
fn parse_args_proxy_alias() -> Result<(), String> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--proxy-url",
        "http://127.0.0.1:8080",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.proxy_url.as_deref() != Some("http://127.0.0.1:8080") {
        return Err("Unexpected proxy_url".to_owned());
    }
    Ok(())
}

#[test]
fn parse_args_concurrency_alias() -> Result<(), String> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--concurrency", "12"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.max_tasks.get() != 12 {
        return Err(format!("Unexpected max_tasks: {}", args.max_tasks.get()));
    }
    Ok(())
}

#[test]
fn parse_args_connections_alias() -> Result<(), String> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--connections", "7"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.max_tasks.get() != 7 {
        return Err(format!("Unexpected max_tasks: {}", args.max_tasks.get()));
    }
    Ok(())
}

#[test]
fn parse_args_accept_and_content_type() -> Result<(), String> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--accept",
        "application/json",
        "--content-type",
        "text/plain",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.accept_header.as_deref() != Some("application/json") {
        return Err("Unexpected accept_header".to_owned());
    }
    if args.content_type.as_deref() != Some("text/plain") {
        return Err("Unexpected content_type".to_owned());
    }
    Ok(())
}

#[test]
fn parse_args_requests_limit() -> Result<(), String> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--requests", "15"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.requests.map(u64::from) != Some(15) {
        return Err("Unexpected requests".to_owned());
    }
    Ok(())
}

#[test]
fn parse_args_data_file_and_lines() -> Result<(), String> {
    let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
    let file_path = dir.path().join("payload.txt");
    std::fs::write(&file_path, "hello\nworld").map_err(|err| format!("write failed: {}", err))?;

    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--data-file",
        file_path.to_str().unwrap_or("payload.txt"),
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.data_file.is_none() {
        return Err("Expected data_file to be Some".to_owned());
    }

    let args_result_lines = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--data-lines",
        file_path.to_str().unwrap_or("payload.txt"),
    ]);
    let args_lines = match args_result_lines {
        Ok(parsed) => parsed,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args_lines.data_lines.is_none() {
        return Err("Expected data_lines to be Some".to_owned());
    }

    Ok(())
}

#[test]
fn parse_args_network_flags() -> Result<(), String> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--http-version",
        "1.1",
        "--proxy-header",
        "Proxy-Auth: secret",
        "--proxy-http-version",
        "2",
        "--host",
        "example.com",
        "--connect-to",
        "example.com:443:localhost:8443",
        "--ipv4",
        "--no-pre-lookup",
        "--no-color",
        "--fps",
        "30",
        "--stats-success-breakdown",
        "--unix-socket",
        "/tmp/strest.sock",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.http_version != Some(HttpVersion::V1_1) {
        return Err("Unexpected http_version".to_owned());
    }
    if args.proxy_headers.len() != 1 {
        return Err("Unexpected proxy_headers".to_owned());
    }
    if args.proxy_http_version != Some(HttpVersion::V2) {
        return Err("Unexpected proxy_http_version".to_owned());
    }
    if args.host_header.as_deref() != Some("example.com") {
        return Err("Unexpected host_header".to_owned());
    }
    if args.connect_to.len() != 1 {
        return Err("Unexpected connect_to".to_owned());
    }
    if !args.ipv4_only {
        return Err("Expected ipv4_only to be true".to_owned());
    }
    if !args.no_pre_lookup {
        return Err("Expected no_pre_lookup to be true".to_owned());
    }
    if !args.no_color {
        return Err("Expected no_color to be true".to_owned());
    }
    if args.ui_fps != 30 {
        return Err("Unexpected ui_fps".to_owned());
    }
    if !args.stats_success_breakdown {
        return Err("Expected stats_success_breakdown to be true".to_owned());
    }
    if args.unix_socket.as_deref() != Some("/tmp/strest.sock") {
        return Err("Unexpected unix_socket".to_owned());
    }
    Ok(())
}
#[test]
fn parse_args_metrics_range() -> Result<(), String> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--metrics-range",
        "10-30",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(format!("Expected Ok, got Err: {}", err));
        }
    };
    if args.metrics_range.is_some() {
        Ok(())
    } else {
        Err("Expected metrics_range to be Some".to_owned())
    }
}

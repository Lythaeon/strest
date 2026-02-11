use super::*;
use crate::args::parsers::parse_bool_env;
use crate::error::{AppError, AppResult};
use clap::Parser;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn parse_header_valid() -> AppResult<()> {
    let parsed = parse_header("Content-Type: application/json");
    match parsed {
        Ok((key, value)) => {
            if key != "Content-Type" {
                return Err(AppError::validation(format!("Unexpected key: {}", key)));
            }
            if value != "application/json" {
                return Err(AppError::validation(format!("Unexpected value: {}", value)));
            }
            Ok(())
        }
        Err(err) => Err(AppError::validation(format!(
            "Expected Ok, got Err: {}",
            err
        ))),
    }
}

#[test]
fn parse_header_invalid() -> AppResult<()> {
    let parsed = parse_header("MissingDelimiter");
    if parsed.is_err() {
        Ok(())
    } else {
        Err(AppError::validation("Expected Err for invalid header"))
    }
}

#[test]
fn parse_args_defaults() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from(["strest", "-u", "http://localhost"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };

    if !matches!(args.method, HttpMethod::Get) {
        return Err(AppError::validation("Expected HttpMethod::Get"));
    }
    if args.url.as_deref() != Some("http://localhost") {
        return Err(AppError::validation("Unexpected url"));
    }
    if args.urls_from_file {
        return Err(AppError::validation("Expected urls_from_file to be false"));
    }
    if args.rand_regex_url {
        return Err(AppError::validation("Expected rand_regex_url to be false"));
    }
    if args.max_repeat.get() != 4 {
        return Err(AppError::validation(format!(
            "Unexpected max_repeat: {}",
            args.max_repeat.get()
        )));
    }
    if args.dump_urls.is_some() {
        return Err(AppError::validation("Expected dump_urls to be None"));
    }
    if args.accept_header.is_some() {
        return Err(AppError::validation("Expected accept_header to be None"));
    }
    if args.content_type.is_some() {
        return Err(AppError::validation("Expected content_type to be None"));
    }
    if args.target_duration.get() != 30 {
        return Err(AppError::validation(format!(
            "Unexpected target_duration: {}",
            args.target_duration.get()
        )));
    }
    if args.requests.is_some() {
        return Err(AppError::validation("Expected requests to be None"));
    }
    if args.expected_status_code != 200 {
        return Err(AppError::validation(format!(
            "Unexpected expected_status_code: {}",
            args.expected_status_code
        )));
    }
    if args.request_timeout != Duration::from_secs(10) {
        return Err(AppError::validation(format!(
            "Unexpected request_timeout: {:?}",
            args.request_timeout
        )));
    }
    if args.redirect_limit != 10 {
        return Err(AppError::validation(format!(
            "Unexpected redirect_limit: {}",
            args.redirect_limit
        )));
    }
    if args.disable_keepalive {
        return Err(AppError::validation(
            "Expected disable_keepalive to be false",
        ));
    }
    if args.disable_compression {
        return Err(AppError::validation(
            "Expected disable_compression to be false",
        ));
    }
    if args.http_version.is_some() {
        return Err(AppError::validation("Expected http_version to be None"));
    }
    if args.connect_timeout != Duration::from_secs(5) {
        return Err(AppError::validation(format!(
            "Unexpected connect_timeout: {:?}",
            args.connect_timeout
        )));
    }
    let expected_charts = default_charts_path();
    if args.charts_path != expected_charts {
        return Err(AppError::validation(format!(
            "Unexpected charts_path: {}",
            args.charts_path
        )));
    }
    if args.no_charts {
        return Err(AppError::validation("Expected no_charts to be false"));
    }
    if args.charts_latency_bucket_ms.get() != 100 {
        return Err(AppError::validation(format!(
            "Unexpected charts_latency_bucket_ms: {}",
            args.charts_latency_bucket_ms.get()
        )));
    }
    if args.no_ua {
        return Err(AppError::validation("Expected no_ua to be false"));
    }
    if args.authorized {
        return Err(AppError::validation("Expected authorized to be false"));
    }
    if !args.form.is_empty() {
        return Err(AppError::validation("Expected form to be empty"));
    }
    if args.data_file.is_some() {
        return Err(AppError::validation("Expected data_file to be None"));
    }
    if args.data_lines.is_some() {
        return Err(AppError::validation("Expected data_lines to be None"));
    }
    if args.basic_auth.is_some() {
        return Err(AppError::validation("Expected basic_auth to be None"));
    }
    if args.aws_session.is_some() {
        return Err(AppError::validation("Expected aws_session to be None"));
    }
    if args.aws_sigv4.is_some() {
        return Err(AppError::validation("Expected aws_sigv4 to be None"));
    }
    if args.wait_ongoing_requests_after_deadline {
        return Err(AppError::validation(
            "Expected wait_ongoing_requests_after_deadline to be false",
        ));
    }
    if args.verbose {
        return Err(AppError::validation("Expected verbose to be false"));
    }
    if args.config.is_some() {
        return Err(AppError::validation("Expected config to be None"));
    }
    let expected_tmp = default_tmp_path();
    if args.tmp_path != expected_tmp {
        return Err(AppError::validation(format!(
            "Unexpected tmp_path: {}",
            args.tmp_path
        )));
    }
    if args.no_ui {
        return Err(AppError::validation("Expected no_ui to be false"));
    }
    if args.ui_window_ms.get() != 10_000 {
        return Err(AppError::validation(format!(
            "Unexpected ui_window_ms: {}",
            args.ui_window_ms.get()
        )));
    }
    if args.summary {
        return Err(AppError::validation("Expected summary to be false"));
    }
    if args.keep_tmp {
        return Err(AppError::validation("Expected keep_tmp to be false"));
    }
    if args.warmup.is_some() {
        return Err(AppError::validation("Expected warmup to be None"));
    }
    if args.tls_min.is_some() {
        return Err(AppError::validation("Expected tls_min to be None"));
    }
    if args.tls_max.is_some() {
        return Err(AppError::validation("Expected tls_max to be None"));
    }
    if args.cacert.is_some() {
        return Err(AppError::validation("Expected cacert to be None"));
    }
    if args.cert.is_some() {
        return Err(AppError::validation("Expected cert to be None"));
    }
    if args.key.is_some() {
        return Err(AppError::validation("Expected key to be None"));
    }
    if args.insecure {
        return Err(AppError::validation("Expected insecure to be false"));
    }
    if args.http2 {
        return Err(AppError::validation("Expected http2 to be false"));
    }
    if args.http2_parallel.get() != 1 {
        return Err(AppError::validation(format!(
            "Unexpected http2_parallel: {}",
            args.http2_parallel.get()
        )));
    }
    if !args.alpn.is_empty() {
        return Err(AppError::validation("Expected alpn to be empty"));
    }
    if !args.proxy_headers.is_empty() {
        return Err(AppError::validation("Expected proxy_headers to be empty"));
    }
    if args.proxy_http_version.is_some() {
        return Err(AppError::validation(
            "Expected proxy_http_version to be None",
        ));
    }
    if args.proxy_http2 {
        return Err(AppError::validation("Expected proxy_http2 to be false"));
    }
    if args.output.is_some() {
        return Err(AppError::validation("Expected output to be None"));
    }
    if args.output_format.is_some() {
        return Err(AppError::validation("Expected output_format to be None"));
    }
    if args.time_unit.is_some() {
        return Err(AppError::validation("Expected time_unit to be None"));
    }
    if args.export_csv.is_some() {
        return Err(AppError::validation("Expected export_csv to be None"));
    }
    if args.export_json.is_some() {
        return Err(AppError::validation("Expected export_json to be None"));
    }
    if args.export_jsonl.is_some() {
        return Err(AppError::validation("Expected export_jsonl to be None"));
    }
    if args.log_shards.get() != 1 {
        return Err(AppError::validation(format!(
            "Unexpected log_shards: {}",
            args.log_shards.get()
        )));
    }
    if args.max_tasks.get() != 1000 {
        return Err(AppError::validation(format!(
            "Unexpected max_tasks: {}",
            args.max_tasks.get()
        )));
    }
    if args.spawn_rate_per_tick.get() != 1 {
        return Err(AppError::validation(format!(
            "Unexpected spawn_rate_per_tick: {}",
            args.spawn_rate_per_tick.get()
        )));
    }
    if args.tick_interval.get() != 100 {
        return Err(AppError::validation(format!(
            "Unexpected tick_interval: {}",
            args.tick_interval.get()
        )));
    }
    if args.rate_limit.is_some() {
        return Err(AppError::validation("Expected rate_limit to be None"));
    }
    if args.burst_delay.is_some() {
        return Err(AppError::validation("Expected burst_delay to be None"));
    }
    if args.burst_rate.get() != 1 {
        return Err(AppError::validation(format!(
            "Unexpected burst_rate: {}",
            args.burst_rate.get()
        )));
    }
    if args.latency_correction {
        return Err(AppError::validation(
            "Expected latency_correction to be false",
        ));
    }
    if !args.connect_to.is_empty() {
        return Err(AppError::validation("Expected connect_to to be empty"));
    }
    if args.host_header.is_some() {
        return Err(AppError::validation("Expected host_header to be None"));
    }
    if args.ipv6_only {
        return Err(AppError::validation("Expected ipv6_only to be false"));
    }
    if args.ipv4_only {
        return Err(AppError::validation("Expected ipv4_only to be false"));
    }
    if args.no_pre_lookup {
        return Err(AppError::validation("Expected no_pre_lookup to be false"));
    }
    let expected_no_color = std::env::var("NO_COLOR")
        .ok()
        .and_then(|value| parse_bool_env(&value).ok())
        .unwrap_or(false);
    if args.no_color != expected_no_color {
        return Err(AppError::validation(format!(
            "Unexpected no_color default: {}",
            args.no_color
        )));
    }
    if args.ui_fps != 16 {
        return Err(AppError::validation(format!(
            "Unexpected ui_fps: {}",
            args.ui_fps
        )));
    }
    if args.stats_success_breakdown {
        return Err(AppError::validation(
            "Expected stats_success_breakdown to be false",
        ));
    }
    if args.unix_socket.is_some() {
        return Err(AppError::validation("Expected unix_socket to be None"));
    }
    if args.load_profile.is_some() {
        return Err(AppError::validation("Expected load_profile to be None"));
    }
    if args.controller_listen.is_some() {
        return Err(AppError::validation(
            "Expected controller_listen to be None",
        ));
    }
    if args.controller_mode != ControllerMode::Auto {
        return Err(AppError::validation("Expected controller_mode to be auto"));
    }
    if args.control_listen.is_some() {
        return Err(AppError::validation("Expected control_listen to be None"));
    }
    if args.control_auth_token.is_some() {
        return Err(AppError::validation(
            "Expected control_auth_token to be None",
        ));
    }
    if args.agent_join.is_some() {
        return Err(AppError::validation("Expected agent_join to be None"));
    }
    if args.auth_token.is_some() {
        return Err(AppError::validation("Expected auth_token to be None"));
    }
    if args.agent_id.is_some() {
        return Err(AppError::validation("Expected agent_id to be None"));
    }
    if args.agent_weight.get() != 1 {
        return Err(AppError::validation(format!(
            "Unexpected agent_weight: {}",
            args.agent_weight.get()
        )));
    }
    if args.min_agents.get() != 1 {
        return Err(AppError::validation(format!(
            "Unexpected min_agents: {}",
            args.min_agents.get()
        )));
    }
    if args.agent_wait_timeout_ms.is_some() {
        return Err(AppError::validation(
            "Expected agent_wait_timeout_ms to be None",
        ));
    }
    if args.agent_standby {
        return Err(AppError::validation("Expected agent_standby to be false"));
    }
    if args.agent_reconnect_ms.get() != 1000 {
        return Err(AppError::validation(format!(
            "Unexpected agent_reconnect_ms: {}",
            args.agent_reconnect_ms.get()
        )));
    }
    if args.agent_heartbeat_interval_ms.get() != 1000 {
        return Err(AppError::validation(format!(
            "Unexpected agent_heartbeat_interval_ms: {}",
            args.agent_heartbeat_interval_ms.get()
        )));
    }
    if args.agent_heartbeat_timeout_ms.get() != 3000 {
        return Err(AppError::validation(format!(
            "Unexpected agent_heartbeat_timeout_ms: {}",
            args.agent_heartbeat_timeout_ms.get()
        )));
    }
    if args.distributed_stream_interval_ms.is_some() {
        return Err(AppError::validation(
            "Expected distributed_stream_interval_ms to be None",
        ));
    }
    if args.replay {
        return Err(AppError::validation("Expected replay to be false"));
    }
    if args.replay_start.is_some() {
        return Err(AppError::validation("Expected replay_start to be None"));
    }
    if args.replay_end.is_some() {
        return Err(AppError::validation("Expected replay_end to be None"));
    }
    if args.replay_step.is_some() {
        return Err(AppError::validation("Expected replay_step to be None"));
    }
    if args.replay_snapshot_interval.is_some() {
        return Err(AppError::validation(
            "Expected replay_snapshot_interval to be None",
        ));
    }
    if args.replay_snapshot_start.is_some() {
        return Err(AppError::validation(
            "Expected replay_snapshot_start to be None",
        ));
    }
    if args.replay_snapshot_end.is_some() {
        return Err(AppError::validation(
            "Expected replay_snapshot_end to be None",
        ));
    }
    if args.replay_snapshot_out.is_some() {
        return Err(AppError::validation(
            "Expected replay_snapshot_out to be None",
        ));
    }
    if args.replay_snapshot_format != "json" {
        return Err(AppError::validation(format!(
            "Unexpected replay_snapshot_format: {}",
            args.replay_snapshot_format
        )));
    }
    if args.scenario.is_some() {
        return Err(AppError::validation("Expected scenario to be None"));
    }
    if args.sinks.is_some() {
        return Err(AppError::validation("Expected sinks to be None"));
    }
    if args.distributed_silent {
        return Err(AppError::validation(
            "Expected distributed_silent to be false",
        ));
    }
    if args.distributed_stream_summaries {
        return Err(AppError::validation(
            "Expected distributed_stream_summaries to be false",
        ));
    }
    if args.http3 {
        return Err(AppError::validation("Expected http3 to be false"));
    }
    if args.install_service {
        return Err(AppError::validation("Expected install_service to be false"));
    }
    if args.uninstall_service {
        return Err(AppError::validation(
            "Expected uninstall_service to be false",
        ));
    }
    if args.service_name.is_some() {
        return Err(AppError::validation("Expected service_name to be None"));
    }
    if args.metrics_max.get() != 1_000_000 {
        return Err(AppError::validation(format!(
            "Unexpected metrics_max: {}",
            args.metrics_max.get()
        )));
    }

    Ok(())
}

#[test]
fn parse_args_proxy_alias() -> AppResult<()> {
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
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.proxy_url.as_deref() != Some("http://127.0.0.1:8080") {
        return Err(AppError::validation("Unexpected proxy_url"));
    }
    Ok(())
}

#[test]
fn parse_args_concurrency_alias() -> AppResult<()> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--concurrency", "12"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.max_tasks.get() != 12 {
        return Err(AppError::validation(format!(
            "Unexpected max_tasks: {}",
            args.max_tasks.get()
        )));
    }
    Ok(())
}

#[test]
fn parse_args_connections_alias() -> AppResult<()> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--connections", "7"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.max_tasks.get() != 7 {
        return Err(AppError::validation(format!(
            "Unexpected max_tasks: {}",
            args.max_tasks.get()
        )));
    }
    Ok(())
}

#[test]
fn parse_args_accept_and_content_type() -> AppResult<()> {
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
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.accept_header.as_deref() != Some("application/json") {
        return Err(AppError::validation("Unexpected accept_header"));
    }
    if args.content_type.as_deref() != Some("text/plain") {
        return Err(AppError::validation("Unexpected content_type"));
    }
    Ok(())
}

#[test]
fn parse_args_requests_limit() -> AppResult<()> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--requests", "15"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.requests.map(u64::from) != Some(15) {
        return Err(AppError::validation("Unexpected requests"));
    }
    Ok(())
}

#[test]
fn parse_args_requests_short_n() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "-n", "7"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.requests.map(u64::from) != Some(7) {
        return Err(AppError::validation("Unexpected requests"));
    }
    Ok(())
}

#[test]
fn parse_args_rate_short_q() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "-q", "9"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.rate_limit.map(u64::from) != Some(9) {
        return Err(AppError::validation("Unexpected rate_limit"));
    }
    Ok(())
}

#[test]
fn parse_args_urls_from_file_flag() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from(["strest", "-u", "urls.txt", "--urls-from-file"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if !args.urls_from_file {
        return Err(AppError::validation("Expected urls_from_file to be true"));
    }
    Ok(())
}

#[test]
fn parse_args_rand_regex_flag() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost/[a-z]{2}",
        "--rand-regex-url",
        "--max-repeat",
        "6",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if !args.rand_regex_url {
        return Err(AppError::validation("Expected rand_regex_url to be true"));
    }
    if args.max_repeat.get() != 6 {
        return Err(AppError::validation("Unexpected max_repeat"));
    }
    Ok(())
}

#[test]
fn parse_args_dump_urls_requires_regex() -> AppResult<()> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--dump-urls", "2"]);
    if args_result.is_ok() {
        return Err(AppError::validation(
            "Expected Err for dump-urls without rand-regex-url",
        ));
    }
    Ok(())
}

#[test]
fn parse_args_form_fields() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--form",
        "name=demo",
        "--form",
        "file=@payload.txt",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.form.len() != 2 {
        return Err(AppError::validation(format!(
            "Unexpected form length: {}",
            args.form.len()
        )));
    }
    Ok(())
}

#[test]
fn parse_args_time_unit() -> AppResult<()> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--time-unit", "ms"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.time_unit != Some(TimeUnit::Ms) {
        return Err(AppError::validation("Unexpected time_unit"));
    }
    Ok(())
}

#[test]
fn parse_args_http2_parallel() -> AppResult<()> {
    let args_result =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--http2-parallel", "4"]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.http2_parallel.get() != 4 {
        return Err(AppError::validation("Unexpected http2_parallel"));
    }
    Ok(())
}

#[test]
fn parse_args_burst_and_latency_flags() -> AppResult<()> {
    let args_result = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--burst-delay",
        "10s",
        "--burst-rate",
        "3",
        "--latency-correction",
    ]);
    let args = match args_result {
        Ok(args) => args,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.burst_delay != Some(Duration::from_secs(10)) {
        return Err(AppError::validation("Unexpected burst_delay"));
    }
    if args.burst_rate.get() != 3 {
        return Err(AppError::validation("Unexpected burst_rate"));
    }
    if !args.latency_correction {
        return Err(AppError::validation(
            "Expected latency_correction to be true",
        ));
    }
    Ok(())
}

#[test]
fn parse_args_data_file_and_lines() -> AppResult<()> {
    let dir = tempdir().map_err(|err| AppError::validation(format!("tempdir failed: {}", err)))?;
    let file_path = dir.path().join("payload.txt");
    std::fs::write(&file_path, "hello\nworld")
        .map_err(|err| AppError::validation(format!("write failed: {}", err)))?;

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
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.data_file.is_none() {
        return Err(AppError::validation("Expected data_file to be Some"));
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
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args_lines.data_lines.is_none() {
        return Err(AppError::validation("Expected data_lines to be Some"));
    }

    Ok(())
}

#[test]
fn parse_args_network_flags() -> AppResult<()> {
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
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.http_version != Some(HttpVersion::V1_1) {
        return Err(AppError::validation("Unexpected http_version"));
    }
    if args.proxy_headers.len() != 1 {
        return Err(AppError::validation("Unexpected proxy_headers"));
    }
    if args.proxy_http_version != Some(HttpVersion::V2) {
        return Err(AppError::validation("Unexpected proxy_http_version"));
    }
    if args.host_header.as_deref() != Some("example.com") {
        return Err(AppError::validation("Unexpected host_header"));
    }
    if args.connect_to.len() != 1 {
        return Err(AppError::validation("Unexpected connect_to"));
    }
    if !args.ipv4_only {
        return Err(AppError::validation("Expected ipv4_only to be true"));
    }
    if !args.no_pre_lookup {
        return Err(AppError::validation("Expected no_pre_lookup to be true"));
    }
    if !args.no_color {
        return Err(AppError::validation("Expected no_color to be true"));
    }
    if args.ui_fps != 30 {
        return Err(AppError::validation("Unexpected ui_fps"));
    }
    if !args.stats_success_breakdown {
        return Err(AppError::validation(
            "Expected stats_success_breakdown to be true",
        ));
    }
    if args.unix_socket.as_deref() != Some("/tmp/strest.sock") {
        return Err(AppError::validation("Unexpected unix_socket"));
    }
    Ok(())
}
#[test]
fn parse_args_metrics_range() -> AppResult<()> {
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
            return Err(AppError::validation(format!(
                "Expected Ok, got Err: {}",
                err
            )));
        }
    };
    if args.metrics_range.is_some() {
        Ok(())
    } else {
        Err(AppError::validation("Expected metrics_range to be Some"))
    }
}

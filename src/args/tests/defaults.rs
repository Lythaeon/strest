use super::*;

#[test]
fn parse_args_defaults() -> AppResult<()> {
    let args = parse_test_args(["strest", "-u", "http://localhost"])?;

    let expected_no_color = std::env::var("NO_COLOR")
        .ok()
        .and_then(|value| parse_bool_env(&value).ok())
        .unwrap_or(false);

    let expected_charts = default_charts_path();
    let expected_tmp = default_tmp_path();

    let checks = [
        (
            matches!(args.method, HttpMethod::Get),
            "Expected HttpMethod::Get",
        ),
        (
            matches!(args.protocol, Protocol::Http),
            "Expected Protocol::Http",
        ),
        (
            matches!(args.load_mode, LoadMode::Arrival),
            "Expected LoadMode::Arrival",
        ),
        (
            args.url.as_deref() == Some("http://localhost"),
            "Unexpected url",
        ),
        (!args.urls_from_file, "Expected urls_from_file to be false"),
        (!args.rand_regex_url, "Expected rand_regex_url to be false"),
        (args.max_repeat.get() == 4, "Unexpected max_repeat"),
        (args.dump_urls.is_none(), "Expected dump_urls to be None"),
        (
            args.accept_header.is_none(),
            "Expected accept_header to be None",
        ),
        (
            args.content_type.is_none(),
            "Expected content_type to be None",
        ),
        (
            args.target_duration.get() == 30,
            "Unexpected target_duration",
        ),
        (args.requests.is_none(), "Expected requests to be None"),
        (
            args.expected_status_code == 200,
            "Unexpected expected_status_code",
        ),
        (
            args.request_timeout == Duration::from_secs(10),
            "Unexpected request_timeout",
        ),
        (args.redirect_limit == 10, "Unexpected redirect_limit"),
        (
            !args.disable_keepalive,
            "Expected disable_keepalive to be false",
        ),
        (
            !args.disable_compression,
            "Expected disable_compression to be false",
        ),
        (
            args.http_version.is_none(),
            "Expected http_version to be None",
        ),
        (
            args.connect_timeout == Duration::from_secs(5),
            "Unexpected connect_timeout",
        ),
        (
            args.charts_path == expected_charts,
            "Unexpected charts_path",
        ),
        (!args.no_charts, "Expected no_charts to be false"),
        (
            args.charts_latency_bucket_ms.get() == 100,
            "Unexpected charts_latency_bucket_ms",
        ),
        (!args.no_ua, "Expected no_ua to be false"),
        (!args.authorized, "Expected authorized to be false"),
        (args.form.is_empty(), "Expected form to be empty"),
        (args.data_file.is_none(), "Expected data_file to be None"),
        (args.data_lines.is_none(), "Expected data_lines to be None"),
        (args.basic_auth.is_none(), "Expected basic_auth to be None"),
        (
            args.aws_session.is_none(),
            "Expected aws_session to be None",
        ),
        (args.aws_sigv4.is_none(), "Expected aws_sigv4 to be None"),
        (
            !args.wait_ongoing_requests_after_deadline,
            "Expected wait_ongoing_requests_after_deadline to be false",
        ),
        (!args.verbose, "Expected verbose to be false"),
        (args.config.is_none(), "Expected config to be None"),
        (args.tmp_path == expected_tmp, "Unexpected tmp_path"),
        (!args.no_ui, "Expected no_ui to be false"),
        (args.ui_window_ms.get() == 10_000, "Unexpected ui_window_ms"),
        (!args.summary, "Expected summary to be false"),
        (!args.keep_tmp, "Expected keep_tmp to be false"),
        (args.warmup.is_none(), "Expected warmup to be None"),
        (args.tls_min.is_none(), "Expected tls_min to be None"),
        (args.tls_max.is_none(), "Expected tls_max to be None"),
        (args.cacert.is_none(), "Expected cacert to be None"),
        (args.cert.is_none(), "Expected cert to be None"),
        (args.key.is_none(), "Expected key to be None"),
        (!args.insecure, "Expected insecure to be false"),
        (!args.http2, "Expected http2 to be false"),
        (args.http2_parallel.get() == 1, "Unexpected http2_parallel"),
        (args.alpn.is_empty(), "Expected alpn to be empty"),
        (
            args.proxy_headers.is_empty(),
            "Expected proxy_headers to be empty",
        ),
        (
            args.proxy_http_version.is_none(),
            "Expected proxy_http_version to be None",
        ),
        (!args.proxy_http2, "Expected proxy_http2 to be false"),
        (args.output.is_none(), "Expected output to be None"),
        (
            args.output_format.is_none(),
            "Expected output_format to be None",
        ),
        (args.time_unit.is_none(), "Expected time_unit to be None"),
        (args.export_csv.is_none(), "Expected export_csv to be None"),
        (
            args.export_json.is_none(),
            "Expected export_json to be None",
        ),
        (
            args.export_jsonl.is_none(),
            "Expected export_jsonl to be None",
        ),
        (args.log_shards.get() == 1, "Unexpected log_shards"),
        (args.max_tasks.get() == 1000, "Unexpected max_tasks"),
        (
            args.spawn_rate_per_tick.get() == 1,
            "Unexpected spawn_rate_per_tick",
        ),
        (args.tick_interval.get() == 100, "Unexpected tick_interval"),
        (args.rate_limit.is_none(), "Expected rate_limit to be None"),
        (
            args.burst_delay.is_none(),
            "Expected burst_delay to be None",
        ),
        (args.burst_rate.get() == 1, "Unexpected burst_rate"),
        (
            !args.latency_correction,
            "Expected latency_correction to be false",
        ),
        (
            args.connect_to.is_empty(),
            "Expected connect_to to be empty",
        ),
        (
            args.host_header.is_none(),
            "Expected host_header to be None",
        ),
        (!args.ipv6_only, "Expected ipv6_only to be false"),
        (!args.ipv4_only, "Expected ipv4_only to be false"),
        (!args.no_pre_lookup, "Expected no_pre_lookup to be false"),
        (
            args.no_color == expected_no_color,
            "Unexpected no_color default",
        ),
        (args.ui_fps == 16, "Unexpected ui_fps"),
        (
            !args.stats_success_breakdown,
            "Expected stats_success_breakdown to be false",
        ),
        (
            args.unix_socket.is_none(),
            "Expected unix_socket to be None",
        ),
        (
            args.load_profile.is_none(),
            "Expected load_profile to be None",
        ),
        (
            args.controller_listen.is_none(),
            "Expected controller_listen to be None",
        ),
        (
            args.controller_mode == ControllerMode::Auto,
            "Expected controller_mode to be auto",
        ),
        (
            args.control_listen.is_none(),
            "Expected control_listen to be None",
        ),
        (
            args.control_auth_token.is_none(),
            "Expected control_auth_token to be None",
        ),
        (args.agent_join.is_none(), "Expected agent_join to be None"),
        (args.auth_token.is_none(), "Expected auth_token to be None"),
        (args.agent_id.is_none(), "Expected agent_id to be None"),
        (args.agent_weight.get() == 1, "Unexpected agent_weight"),
        (args.min_agents.get() == 1, "Unexpected min_agents"),
        (
            args.agent_wait_timeout_ms.is_none(),
            "Expected agent_wait_timeout_ms to be None",
        ),
        (!args.agent_standby, "Expected agent_standby to be false"),
        (
            args.agent_reconnect_ms.get() == 1000,
            "Unexpected agent_reconnect_ms",
        ),
        (
            args.agent_heartbeat_interval_ms.get() == 1000,
            "Unexpected agent_heartbeat_interval_ms",
        ),
        (
            args.agent_heartbeat_timeout_ms.get() == 3000,
            "Unexpected agent_heartbeat_timeout_ms",
        ),
        (
            args.distributed_stream_interval_ms.is_none(),
            "Expected distributed_stream_interval_ms to be None",
        ),
        (!args.replay, "Expected replay to be false"),
        (
            args.replay_start.is_none(),
            "Expected replay_start to be None",
        ),
        (args.replay_end.is_none(), "Expected replay_end to be None"),
        (
            args.replay_step.is_none(),
            "Expected replay_step to be None",
        ),
        (
            args.replay_snapshot_interval.is_none(),
            "Expected replay_snapshot_interval to be None",
        ),
        (
            args.replay_snapshot_start.is_none(),
            "Expected replay_snapshot_start to be None",
        ),
        (
            args.replay_snapshot_end.is_none(),
            "Expected replay_snapshot_end to be None",
        ),
        (
            args.replay_snapshot_out.is_none(),
            "Expected replay_snapshot_out to be None",
        ),
        (
            args.replay_snapshot_format == "json",
            "Unexpected replay_snapshot_format",
        ),
        (args.scenario.is_none(), "Expected scenario to be None"),
        (args.sinks.is_none(), "Expected sinks to be None"),
        (
            !args.distributed_silent,
            "Expected distributed_silent to be false",
        ),
        (
            !args.distributed_stream_summaries,
            "Expected distributed_stream_summaries to be false",
        ),
        (!args.http3, "Expected http3 to be false"),
        (
            !args.install_service,
            "Expected install_service to be false",
        ),
        (
            !args.uninstall_service,
            "Expected uninstall_service to be false",
        ),
        (
            args.service_name.is_none(),
            "Expected service_name to be None",
        ),
        (
            args.metrics_max.get() == 1_000_000,
            "Unexpected metrics_max",
        ),
    ];

    for (ok, msg) in checks {
        if !ok {
            return Err(AppError::validation(msg));
        }
    }

    Ok(())
}

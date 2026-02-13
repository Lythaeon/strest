use super::*;

#[test]
fn parse_args_time_unit() -> AppResult<()> {
    let args =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--time-unit", "ms"])
            .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.time_unit != Some(TimeUnit::Ms) {
        return Err(AppError::validation("Unexpected time_unit"));
    }
    Ok(())
}

#[test]
fn parse_args_http2_parallel() -> AppResult<()> {
    let args =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--http2-parallel", "4"])
            .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.http2_parallel.get() != 4 {
        return Err(AppError::validation("Unexpected http2_parallel"));
    }
    Ok(())
}

#[test]
fn parse_args_burst_and_latency_flags() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--burst-delay",
        "10s",
        "--burst-rate",
        "3",
        "--latency-correction",
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.burst_delay != Some(Duration::from_secs(10))
        || args.burst_rate.get() != 3
        || !args.latency_correction
    {
        return Err(AppError::validation("Unexpected burst flags"));
    }
    Ok(())
}

#[test]
fn parse_args_data_file_and_lines() -> AppResult<()> {
    let dir = tempdir().map_err(|err| AppError::validation(format!("tempdir failed: {}", err)))?;
    let file_path = dir.path().join("payload.txt");
    std::fs::write(&file_path, "hello\nworld")
        .map_err(|err| AppError::validation(format!("write failed: {}", err)))?;

    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--data-file",
        file_path.to_str().unwrap_or("payload.txt"),
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.data_file.is_none() {
        return Err(AppError::validation("Expected data_file to be Some"));
    }

    let args_lines = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--data-lines",
        file_path.to_str().unwrap_or("payload.txt"),
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args_lines.data_lines.is_none() {
        return Err(AppError::validation("Expected data_lines to be Some"));
    }

    Ok(())
}

#[test]
fn parse_args_network_flags() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
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
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;

    if args.http_version != Some(HttpVersion::V1_1)
        || args.proxy_headers.len() != 1
        || args.proxy_http_version != Some(HttpVersion::V2)
        || args.host_header.as_deref() != Some("example.com")
        || args.connect_to.len() != 1
        || !args.ipv4_only
        || !args.no_pre_lookup
        || !args.no_color
        || args.ui_fps != 30
        || !args.stats_success_breakdown
        || args.unix_socket.as_deref() != Some("/tmp/strest.sock")
    {
        return Err(AppError::validation("Unexpected network flags"));
    }
    Ok(())
}

#[test]
fn parse_args_metrics_range() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--metrics-range",
        "10-30",
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.metrics_range.is_none() {
        return Err(AppError::validation("Expected metrics_range to be Some"));
    }
    Ok(())
}

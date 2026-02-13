use super::*;

#[test]
fn parse_args_protocol_and_load_mode() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--protocol",
        "websocket",
        "--load-mode",
        "burst",
    ])
    .map_err(|err| AppError::validation(format!("Expected parse success: {}", err)))?;

    if !matches!(args.protocol, Protocol::Websocket) {
        return Err(AppError::validation("Expected Protocol::Websocket"));
    }
    if !matches!(args.load_mode, LoadMode::Burst) {
        return Err(AppError::validation("Expected LoadMode::Burst"));
    }
    Ok(())
}

#[test]
fn parse_args_proxy_alias() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--proxy-url",
        "http://127.0.0.1:8080",
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.proxy_url.as_deref() != Some("http://127.0.0.1:8080") {
        return Err(AppError::validation("Unexpected proxy_url"));
    }
    Ok(())
}

#[test]
fn parse_args_concurrency_alias() -> AppResult<()> {
    let args =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--concurrency", "12"])
            .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.max_tasks.get() != 12 {
        return Err(AppError::validation("Unexpected max_tasks"));
    }
    Ok(())
}

#[test]
fn parse_args_connections_alias() -> AppResult<()> {
    let args =
        TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--connections", "7"])
            .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.max_tasks.get() != 7 {
        return Err(AppError::validation("Unexpected max_tasks"));
    }
    Ok(())
}

#[test]
fn parse_args_accept_and_content_type() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--accept",
        "application/json",
        "--content-type",
        "text/plain",
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
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
    let args = TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "--requests", "15"])
        .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.requests.map(u64::from) != Some(15) {
        return Err(AppError::validation("Unexpected requests"));
    }
    Ok(())
}

#[test]
fn parse_args_requests_short_n() -> AppResult<()> {
    let args = TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "-n", "7"])
        .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.requests.map(u64::from) != Some(7) {
        return Err(AppError::validation("Unexpected requests"));
    }
    Ok(())
}

#[test]
fn parse_args_rate_short_q() -> AppResult<()> {
    let args = TesterArgs::try_parse_from(["strest", "-u", "http://localhost", "-q", "9"])
        .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.rate_limit.map(u64::from) != Some(9) {
        return Err(AppError::validation("Unexpected rate_limit"));
    }
    Ok(())
}

#[test]
fn parse_args_urls_from_file_flag() -> AppResult<()> {
    let args = TesterArgs::try_parse_from(["strest", "-u", "urls.txt", "--urls-from-file"])
        .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if !args.urls_from_file {
        return Err(AppError::validation("Expected urls_from_file to be true"));
    }
    Ok(())
}

#[test]
fn parse_args_rand_regex_flag() -> AppResult<()> {
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost/[a-z]{2}",
        "--rand-regex-url",
        "--max-repeat",
        "6",
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if !args.rand_regex_url || args.max_repeat.get() != 6 {
        return Err(AppError::validation("Unexpected rand_regex_url/max_repeat"));
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
    let args = TesterArgs::try_parse_from([
        "strest",
        "-u",
        "http://localhost",
        "--form",
        "name=demo",
        "--form",
        "file=@payload.txt",
    ])
    .map_err(|err| AppError::validation(format!("Expected Ok, got Err: {}", err)))?;
    if args.form.len() != 2 {
        return Err(AppError::validation("Unexpected form length"));
    }
    Ok(())
}

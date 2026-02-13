use clap::ArgMatches;

use crate::args::{PositiveU64, TesterArgs};
use crate::error::{AppError, AppResult, ConfigError};

use super::super::types::ConfigFile;
use super::util::{ensure_positive_u64, ensure_positive_usize, is_cli, parse_headers};

pub(super) fn apply_basic_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
) -> AppResult<()> {
    if !is_cli(matches, "method")
        && let Some(method) = config.method
    {
        args.method = method;
    }

    if !is_cli(matches, "url")
        && let Some(url) = config.url.clone()
    {
        args.url = Some(url);
    }

    if !is_cli(matches, "urls_from_file")
        && let Some(value) = config.urls_from_file
    {
        args.urls_from_file = value;
    }

    if !is_cli(matches, "rand_regex_url")
        && let Some(value) = config.rand_regex_url
    {
        args.rand_regex_url = value;
    }

    if !is_cli(matches, "max_repeat")
        && let Some(value) = config.max_repeat
    {
        args.max_repeat = ensure_positive_usize(value, "max_repeat")?;
    }

    if !is_cli(matches, "dump_urls")
        && let Some(value) = config.dump_urls
    {
        args.dump_urls = Some(ensure_positive_usize(value, "dump_urls")?);
    }

    if !is_cli(matches, "headers")
        && let Some(headers) = config.headers.as_ref()
    {
        args.headers = parse_headers(headers)?;
    }

    if !is_cli(matches, "accept_header")
        && let Some(accept) = config.accept.clone()
    {
        args.accept_header = Some(accept);
    }

    if !is_cli(matches, "content_type")
        && let Some(content_type) = config.content_type.clone()
    {
        args.content_type = Some(content_type);
    }

    if !is_cli(matches, "data")
        && let Some(data) = config.data.clone()
    {
        args.data = data;
    }

    if !is_cli(matches, "form")
        && let Some(form) = config.form.clone()
    {
        args.form = form;
    }

    if !is_cli(matches, "basic_auth")
        && let Some(auth) = config.basic_auth.clone()
    {
        args.basic_auth = Some(auth);
    }

    if !is_cli(matches, "aws_session")
        && let Some(session) = config.aws_session.clone()
    {
        args.aws_session = Some(session);
    }

    if !is_cli(matches, "aws_sigv4")
        && let Some(params) = config.aws_sigv4.clone()
    {
        args.aws_sigv4 = Some(params);
    }

    if !is_cli(matches, "data_file")
        && let Some(path) = config.data_file.clone()
    {
        args.data_file = Some(path);
    }

    if !is_cli(matches, "data_lines")
        && let Some(path) = config.data_lines.clone()
    {
        args.data_lines = Some(path);
    }

    if !is_cli(matches, "target_duration")
        && let Some(duration) = config.duration
    {
        args.target_duration = ensure_positive_u64(duration, "duration")?;
    }

    if !is_cli(matches, "wait_ongoing_requests_after_deadline")
        && let Some(value) = config.wait_ongoing_requests_after_deadline
    {
        args.wait_ongoing_requests_after_deadline = value;
    }

    if !is_cli(matches, "requests")
        && let Some(requests) = config.requests
    {
        args.requests = Some(ensure_positive_u64(requests, "requests")?);
    }

    if !is_cli(matches, "request_timeout")
        && let Some(timeout) = config.timeout.as_ref()
    {
        args.request_timeout = timeout.to_duration()?;
    }

    if !is_cli(matches, "redirect_limit")
        && let Some(limit) = config.redirect
    {
        args.redirect_limit = limit;
    }

    if !is_cli(matches, "disable_keepalive")
        && let Some(disable) = config.disable_keepalive
    {
        args.disable_keepalive = disable;
    }

    if !is_cli(matches, "disable_compression")
        && let Some(disable) = config.disable_compression
    {
        args.disable_compression = disable;
    }

    if !is_cli(matches, "pool_max_idle_per_host")
        && let Some(value) = config.pool_max_idle_per_host
    {
        args.pool_max_idle_per_host = Some(ensure_positive_usize(value, "pool_max_idle_per_host")?);
    }

    if !is_cli(matches, "pool_idle_timeout_ms")
        && let Some(value) = config.pool_idle_timeout_ms
    {
        args.pool_idle_timeout_ms = Some(ensure_positive_u64(value, "pool_idle_timeout_ms")?);
    }

    if !is_cli(matches, "connect_timeout")
        && let Some(timeout) = config.connect_timeout.as_ref()
    {
        args.connect_timeout = timeout.to_duration()?;
    }

    if !is_cli(matches, "warmup")
        && let Some(warmup) = config.warmup.as_ref()
    {
        args.warmup = Some(warmup.to_duration()?);
    }

    if !is_cli(matches, "expected_status_code")
        && let Some(status) = config.status
    {
        args.expected_status_code = status;
    }

    if !is_cli(matches, "charts_path")
        && let Some(path) = config.charts_path.clone()
    {
        args.charts_path = path;
    }

    if !is_cli(matches, "no_charts")
        && let Some(no_charts) = config.no_charts
    {
        args.no_charts = no_charts;
    }

    if !is_cli(matches, "charts_latency_bucket_ms")
        && let Some(bucket_ms) = config.charts_latency_bucket_ms
    {
        args.charts_latency_bucket_ms = PositiveU64::try_from(bucket_ms).map_err(|err| {
            AppError::config(ConfigError::InvalidChartsLatencyBucket { source: err })
        })?;
    }

    if !is_cli(matches, "no_ua")
        && let Some(no_ua) = config.no_ua
    {
        args.no_ua = no_ua;
    }

    if !is_cli(matches, "authorized")
        && let Some(authorized) = config.authorized
    {
        args.authorized = authorized;
    }

    Ok(())
}

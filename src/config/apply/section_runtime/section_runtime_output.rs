use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::error::AppResult;

use super::super::super::types::ConfigFile;
use super::super::util::{ensure_positive_u64, ensure_positive_usize, is_cli, parse_headers};

pub(super) fn apply_runtime_output_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
) -> AppResult<()> {
    if !is_cli(matches, "tmp_path")
        && let Some(path) = config.tmp_path.clone()
    {
        args.tmp_path = path;
    }

    if !is_cli(matches, "keep_tmp")
        && let Some(keep) = config.keep_tmp
    {
        args.keep_tmp = keep;
    }

    if !is_cli(matches, "output")
        && let Some(output) = config.output.clone()
    {
        args.output = Some(output);
    }

    if !is_cli(matches, "output_format")
        && let Some(format) = config.output_format
    {
        args.output_format = Some(format);
    }

    if !is_cli(matches, "time_unit")
        && let Some(unit) = config.time_unit
    {
        args.time_unit = Some(unit);
    }

    if !is_cli(matches, "export_csv")
        && let Some(path) = config.export_csv.clone()
    {
        args.export_csv = Some(path);
    }

    if !is_cli(matches, "export_json")
        && let Some(path) = config.export_json.clone()
    {
        args.export_json = Some(path);
    }

    if !is_cli(matches, "export_jsonl")
        && let Some(path) = config.export_jsonl.clone()
    {
        args.export_jsonl = Some(path);
    }

    if !is_cli(matches, "db_url")
        && let Some(db_url) = config.db_url.clone()
    {
        args.db_url = Some(db_url);
    }

    if !is_cli(matches, "log_shards")
        && let Some(log_shards) = config.log_shards
    {
        args.log_shards = ensure_positive_usize(log_shards, "log_shards")?;
    }

    if !is_cli(matches, "no_ui")
        && let Some(no_ui) = config.no_ui
    {
        args.no_ui = no_ui;
    }

    if !is_cli(matches, "ui_window_ms")
        && let Some(window_ms) = config.ui_window_ms
    {
        args.ui_window_ms = ensure_positive_u64(window_ms, "ui_window_ms")?;
    }

    if !is_cli(matches, "summary")
        && let Some(summary) = config.summary
    {
        args.summary = summary;
    }

    if !is_cli(matches, "tls_min")
        && let Some(version) = config.tls_min
    {
        args.tls_min = Some(version);
    }

    if !is_cli(matches, "tls_max")
        && let Some(version) = config.tls_max
    {
        args.tls_max = Some(version);
    }

    if !is_cli(matches, "cacert")
        && let Some(path) = config.cacert.clone()
    {
        args.cacert = Some(path);
    }

    if !is_cli(matches, "cert")
        && let Some(path) = config.cert.clone()
    {
        args.cert = Some(path);
    }

    if !is_cli(matches, "key")
        && let Some(path) = config.key.clone()
    {
        args.key = Some(path);
    }

    if !is_cli(matches, "insecure")
        && let Some(flag) = config.insecure
    {
        args.insecure = flag;
    }

    if !is_cli(matches, "http2")
        && let Some(http2) = config.http2
    {
        args.http2 = http2;
    }

    if !is_cli(matches, "http2_parallel")
        && let Some(value) = config.http2_parallel
    {
        args.http2_parallel = ensure_positive_usize(value, "http2_parallel")?;
    }

    if !is_cli(matches, "http_version")
        && let Some(version) = config.http_version
    {
        args.http_version = Some(version);
    }

    if !is_cli(matches, "http3")
        && let Some(http3) = config.http3
    {
        args.http3 = http3;
    }

    if !is_cli(matches, "alpn")
        && let Some(alpn) = config.alpn.clone()
    {
        args.alpn = alpn;
    }

    if !is_cli(matches, "proxy_url")
        && let Some(proxy) = config.proxy_url.clone()
    {
        args.proxy_url = Some(proxy);
    }

    if !is_cli(matches, "proxy_headers")
        && let Some(headers) = config.proxy_headers.as_ref()
    {
        args.proxy_headers = parse_headers(headers)?;
    }

    if !is_cli(matches, "proxy_http_version")
        && let Some(version) = config.proxy_http_version
    {
        args.proxy_http_version = Some(version);
    }

    if !is_cli(matches, "proxy_http2")
        && let Some(proxy_http2) = config.proxy_http2
    {
        args.proxy_http2 = proxy_http2;
    }

    Ok(())
}

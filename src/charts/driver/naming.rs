use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, Timelike};

use crate::args::TesterArgs;

pub(super) fn resolve_chart_output_dir(args: &TesterArgs) -> PathBuf {
    Path::new(&args.charts_path).join(chart_run_dir_name(args))
}

fn chart_run_dir_name(args: &TesterArgs) -> String {
    let now = Local::now();
    let stamp = format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    format!("run-{}_{}", stamp, target_host_port_segment(args))
}

fn target_host_port_segment(args: &TesterArgs) -> String {
    let url_port = args
        .url
        .as_deref()
        .and_then(|value| url::Url::parse(value).ok())
        .and_then(|value| value.port_or_known_default());

    if let Some(host_header) = args.host_header.as_deref()
        && let Some(segment) = host_port_from_header(host_header, url_port)
    {
        return segment;
    }

    if let Some(url) = args.url.as_deref()
        && let Ok(parsed) = url::Url::parse(url)
        && let Some(host) = parsed.host_str()
    {
        let port = parsed.port_or_known_default().unwrap_or(0);
        return sanitize_host_port(host, port);
    }

    "unknown-host-0".to_owned()
}

fn sanitize_segment(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect()
}

fn host_port_from_header(header: &str, fallback_port: Option<u16>) -> Option<String> {
    let trimmed = header.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = format!("http://{}", trimmed);
    let parsed = url::Url::parse(&candidate).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port().unwrap_or_else(|| {
        fallback_port
            .or_else(|| parsed.port_or_known_default())
            .unwrap_or(0)
    });
    Some(sanitize_host_port(host, port))
}

fn sanitize_host_port(host: &str, port: u16) -> String {
    let sanitized_host = sanitize_segment(host);
    let resolved_host = if sanitized_host.is_empty() {
        "unknown-host".to_owned()
    } else {
        sanitized_host
    };
    format!("{}-{}", resolved_host, port)
}

pub(crate) fn is_chart_run_dir_name(name: &str) -> bool {
    if !name.starts_with("run-") {
        return false;
    }
    let rest = &name[4..];
    if rest.matches('_').count() >= 2 {
        return is_new_chart_run_dir(rest);
    }
    is_legacy_chart_run_dir(rest)
}

fn is_new_chart_run_dir(rest: &str) -> bool {
    let mut parts = rest.splitn(3, '_');
    let date = parts.next().unwrap_or("");
    let time = parts.next().unwrap_or("");
    let host = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return false;
    }
    if !is_date_part(date) || !is_time_part(time) {
        return false;
    }
    is_host_port_part(host)
}

fn is_legacy_chart_run_dir(rest: &str) -> bool {
    let Some((stamp, host)) = rest.split_once('-') else {
        return false;
    };
    !stamp.is_empty()
        && stamp.chars().all(|c| c.is_ascii_digit())
        && !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn is_date_part(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, b)| idx == 4 || idx == 7 || b.is_ascii_digit())
}

fn is_time_part(time: &str) -> bool {
    let bytes = time.as_bytes();
    if bytes.len() != 8 {
        return false;
    }
    bytes.get(2) == Some(&b'-')
        && bytes.get(5) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, b)| idx == 2 || idx == 5 || b.is_ascii_digit())
}

fn is_host_port_part(host: &str) -> bool {
    let Some((host_name, port)) = host.rsplit_once('-') else {
        return false;
    };
    !host_name.is_empty()
        && !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
        && host_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

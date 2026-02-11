use std::path::Path;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, MetricsError, ValidationError};
use crate::metrics::MetricRecord;

pub(super) async fn load_replay_records(args: &TesterArgs) -> AppResult<Vec<MetricRecord>> {
    let export_sources = [
        args.export_csv.as_ref(),
        args.export_json.as_ref(),
        args.export_jsonl.as_ref(),
    ]
    .into_iter()
    .filter(|value| value.is_some())
    .count();
    if export_sources > 1 {
        return Err(AppError::validation(
            ValidationError::ReplayExportSourceConflict,
        ));
    }
    if let Some(path) = args.export_csv.as_deref() {
        return read_csv_records(Path::new(path)).await;
    }
    if let Some(path) = args.export_json.as_deref() {
        return read_json_records(Path::new(path)).await;
    }
    if let Some(path) = args.export_jsonl.as_deref() {
        return read_jsonl_records(Path::new(path)).await;
    }
    read_tmp_records(Path::new(&args.tmp_path)).await
}

pub(super) async fn read_tmp_records(path: &Path) -> AppResult<Vec<MetricRecord>> {
    let metadata = tokio::fs::metadata(path).await.map_err(|err| {
        AppError::metrics(MetricsError::Io {
            context: "stat tmp path",
            source: err,
        })
    })?;
    if metadata.is_file() {
        return read_csv_records(path).await;
    }
    if !metadata.is_dir() {
        return Err(AppError::metrics(MetricsError::ReplayTmpPathInvalid));
    }

    let mut entries = tokio::fs::read_dir(path).await.map_err(|err| {
        AppError::metrics(MetricsError::Io {
            context: "read tmp directory",
            source: err,
        })
    })?;
    let mut records = Vec::new();
    let mut found = false;
    while let Some(entry) = entries.next_entry().await.map_err(|err| {
        AppError::metrics(MetricsError::Io {
            context: "read tmp entry",
            source: err,
        })
    })? {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path();
        if !file_name.starts_with("metrics-") || !file_name.ends_with(".log") {
            continue;
        }
        found = true;
        let mut file_records = read_csv_records(&entry_path).await?;
        records.append(&mut file_records);
    }
    if !found {
        return Err(AppError::metrics(MetricsError::ReplayTmpNoLogs));
    }
    Ok(records)
}

pub(super) async fn read_csv_records(path: &Path) -> AppResult<Vec<MetricRecord>> {
    let file = tokio::fs::File::open(path).await.map_err(|err| {
        AppError::metrics(MetricsError::Io {
            context: "open replay file",
            source: err,
        })
    })?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut records = Vec::new();
    let mut saw_header = false;

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await.map_err(|err| {
            AppError::metrics(MetricsError::Io {
                context: "read replay file",
                source: err,
            })
        })?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !saw_header && trimmed.starts_with("elapsed_ms") {
            saw_header = true;
            continue;
        }
        saw_header = true;
        let mut parts = trimmed.split(',');
        let elapsed_ms = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
            Some(value) => value,
            None => continue,
        };
        let latency_ms = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
            Some(value) => value,
            None => continue,
        };
        let status_code = match parts.next().and_then(|value| value.parse::<u16>().ok()) {
            Some(value) => value,
            None => continue,
        };
        let timed_out = parts.next().map(parse_bool).unwrap_or(false);
        let transport_error = parts.next().map(parse_bool).unwrap_or(false);
        records.push(MetricRecord {
            elapsed_ms,
            latency_ms,
            status_code,
            timed_out,
            transport_error,
        });
    }

    Ok(records)
}

pub(super) async fn read_json_records(path: &Path) -> AppResult<Vec<MetricRecord>> {
    let bytes = tokio::fs::read(path).await.map_err(|err| {
        AppError::metrics(MetricsError::Io {
            context: "read replay file",
            source: err,
        })
    })?;
    let payload: ExportJson = serde_json::from_slice(&bytes).map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "parse JSON",
            source: Box::new(err),
        })
    })?;
    Ok(payload
        .records
        .into_iter()
        .map(|record| MetricRecord {
            elapsed_ms: record.elapsed_ms,
            latency_ms: record.latency_ms,
            status_code: record.status_code,
            timed_out: record.timed_out,
            transport_error: record.transport_error,
        })
        .collect())
}

pub(super) async fn read_jsonl_records(path: &Path) -> AppResult<Vec<MetricRecord>> {
    let file = tokio::fs::File::open(path).await.map_err(|err| {
        AppError::metrics(MetricsError::Io {
            context: "open replay file",
            source: err,
        })
    })?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut records = Vec::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await.map_err(|err| {
            AppError::metrics(MetricsError::Io {
                context: "read replay file",
                source: err,
            })
        })?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: ExportJsonlLine = serde_json::from_str(trimmed).map_err(|err| {
            AppError::metrics(MetricsError::External {
                context: "parse JSONL",
                source: Box::new(err),
            })
        })?;
        if let Some(kind) = parsed.kind.as_deref()
            && kind != "record"
        {
            continue;
        }
        let Some(elapsed_ms) = parsed.elapsed_ms else {
            continue;
        };
        let Some(latency_ms) = parsed.latency_ms else {
            continue;
        };
        let Some(status_code) = parsed.status_code else {
            continue;
        };
        records.push(MetricRecord {
            elapsed_ms,
            latency_ms,
            status_code,
            timed_out: parsed.timed_out.unwrap_or(false),
            transport_error: parsed.transport_error.unwrap_or(false),
        });
    }

    Ok(records)
}

fn parse_bool(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
}

#[derive(Debug, Deserialize)]
struct ExportJson {
    records: Vec<ExportRecord>,
}

#[derive(Debug, Deserialize)]
struct ExportRecord {
    elapsed_ms: u64,
    latency_ms: u64,
    status_code: u16,
    timed_out: bool,
    transport_error: bool,
}

#[derive(Debug, Deserialize)]
struct ExportJsonlLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    elapsed_ms: Option<u64>,
    latency_ms: Option<u64>,
    status_code: Option<u16>,
    timed_out: Option<bool>,
    transport_error: Option<bool>,
}

use super::bounds::{BoundDefault, parse_bound, resolve_bound};
use super::records::{read_csv_records, read_json_records, read_jsonl_records};
use super::window_slice;
use crate::error::{AppError, AppResult};
use crate::metrics::MetricRecord;
use std::time::Duration;
use tempfile::tempdir;

fn run_async_test<F>(future: F) -> AppResult<()>
where
    F: std::future::Future<Output = AppResult<()>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::metrics(format!("Failed to build runtime: {}", err)))?;
    runtime.block_on(future)
}

#[test]
fn parse_bound_accepts_min_max_and_duration() -> AppResult<()> {
    match parse_bound("min")? {
        super::bounds::ReplayBound::Min => {}
        super::bounds::ReplayBound::Max | super::bounds::ReplayBound::Duration(_) => {
            return Err(AppError::validation("Expected min bound"));
        }
    }
    match parse_bound("max")? {
        super::bounds::ReplayBound::Max => {}
        super::bounds::ReplayBound::Min | super::bounds::ReplayBound::Duration(_) => {
            return Err(AppError::validation("Expected max bound"));
        }
    }
    match parse_bound("10s")? {
        super::bounds::ReplayBound::Duration(duration) => {
            if duration != Duration::from_secs(10) {
                return Err(AppError::validation("Unexpected duration value"));
            }
        }
        super::bounds::ReplayBound::Min | super::bounds::ReplayBound::Max => {
            return Err(AppError::validation("Expected duration bound"));
        }
    }
    Ok(())
}

#[test]
fn resolve_bound_clamps_to_range() -> AppResult<()> {
    let min = 100;
    let max = 200;
    let resolved_max = resolve_bound(Some("1s"), min, max, BoundDefault::Min)?;
    if resolved_max != max {
        return Err(AppError::validation(format!(
            "Expected clamp to {}, got {}",
            max, resolved_max
        )));
    }
    let resolved_min = resolve_bound(Some("min"), min, max, BoundDefault::Max)?;
    if resolved_min != min {
        return Err(AppError::validation(format!(
            "Expected min {} got {}",
            min, resolved_min
        )));
    }
    Ok(())
}

#[test]
fn window_slice_handles_bounds() -> AppResult<()> {
    let records = vec![
        MetricRecord {
            elapsed_ms: 0,
            latency_ms: 10,
            status_code: 200,
            timed_out: false,
            transport_error: false,
        },
        MetricRecord {
            elapsed_ms: 1000,
            latency_ms: 20,
            status_code: 200,
            timed_out: false,
            transport_error: false,
        },
    ];
    let first_slice = window_slice(&records, 0, 500);
    if first_slice.len() != 1 {
        return Err(AppError::validation(format!(
            "Expected 1 record, got {}",
            first_slice.len()
        )));
    }
    let empty_slice = window_slice(&records, 2000, 3000);
    if !empty_slice.is_empty() {
        return Err(AppError::validation(
            "Expected empty slice for out-of-range window",
        ));
    }
    Ok(())
}

#[test]
fn read_csv_records_parses_header_and_values() -> AppResult<()> {
    run_async_test(async {
        let dir = tempdir().map_err(|err| AppError::metrics(format!("tempdir failed: {}", err)))?;
        let path = dir.path().join("metrics.csv");
        tokio::fs::write(
            &path,
            "elapsed_ms,latency_ms,status_code,timed_out,transport_error\n1,10,200,0,1\n",
        )
        .await
        .map_err(|err| AppError::metrics(format!("write failed: {}", err)))?;

        let records = read_csv_records(&path).await?;
        if records.len() != 1 {
            return Err(AppError::validation(format!(
                "Expected 1 record, got {}",
                records.len()
            )));
        }
        let record = records
            .first()
            .ok_or_else(|| AppError::validation("Missing parsed record"))?;
        if record.elapsed_ms != 1 || record.latency_ms != 10 {
            return Err(AppError::validation("Unexpected record values"));
        }
        if !record.transport_error || record.timed_out {
            return Err(AppError::validation("Unexpected flags in record"));
        }
        Ok(())
    })
}

#[test]
fn read_json_records_parses_payload() -> AppResult<()> {
    run_async_test(async {
        let dir = tempdir().map_err(|err| AppError::metrics(format!("tempdir failed: {}", err)))?;
        let path = dir.path().join("metrics.json");
        let payload = r#"{
                "summary": { "duration_ms": 10 },
                "records": [
                    { "elapsed_ms": 5, "latency_ms": 20, "status_code": 200, "timed_out": false, "transport_error": false }
                ]
            }"#;
        tokio::fs::write(&path, payload)
            .await
            .map_err(|err| AppError::metrics(format!("write failed: {}", err)))?;
        let records = read_json_records(&path).await?;
        if records.len() != 1 {
            return Err(AppError::validation(format!(
                "Expected 1 record, got {}",
                records.len()
            )));
        }
        let record = records
            .first()
            .ok_or_else(|| AppError::validation("Missing parsed record"))?;
        if record.elapsed_ms != 5 || record.latency_ms != 20 {
            return Err(AppError::validation("Unexpected record values"));
        }
        Ok(())
    })
}

#[test]
fn read_jsonl_records_parses_payload() -> AppResult<()> {
    run_async_test(async {
        let dir = tempdir().map_err(|err| AppError::metrics(format!("tempdir failed: {}", err)))?;
        let path = dir.path().join("metrics.jsonl");
        let payload = r#"{"type":"summary","duration_ms":10}
{"type":"record","elapsed_ms":5,"latency_ms":20,"status_code":200,"timed_out":false,"transport_error":false}
"#;
        tokio::fs::write(&path, payload)
            .await
            .map_err(|err| AppError::metrics(format!("write failed: {}", err)))?;
        let records = read_jsonl_records(&path).await?;
        if records.len() != 1 {
            return Err(AppError::validation(format!(
                "Expected 1 record, got {}",
                records.len()
            )));
        }
        let record = records
            .first()
            .ok_or_else(|| AppError::validation("Missing parsed record"))?;
        if record.elapsed_ms != 5 || record.latency_ms != 20 {
            return Err(AppError::validation("Unexpected record values"));
        }
        Ok(())
    })
}

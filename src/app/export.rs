use crate::metrics;
use tokio::io::{AsyncWriteExt, BufWriter};

fn flow_summary(
    records: &[metrics::MetricRecord],
    duration: std::time::Duration,
) -> (u128, u64, u64, u64) {
    let total_response_bytes: u128 = records
        .iter()
        .map(|record| u128::from(record.response_bytes))
        .sum();
    let max_in_flight_ops = records
        .iter()
        .map(|record| record.in_flight_ops)
        .max()
        .unwrap_or(0);
    let last_in_flight_ops = records
        .last()
        .map(|record| record.in_flight_ops)
        .unwrap_or(0);
    let duration_ms = duration.as_millis();
    let avg_response_bytes_per_sec = if duration_ms > 0 {
        let per_sec = total_response_bytes
            .saturating_mul(1000)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(per_sec).unwrap_or(u64::MAX)
    } else {
        0
    };

    (
        total_response_bytes,
        avg_response_bytes_per_sec,
        max_in_flight_ops,
        last_in_flight_ops,
    )
}

pub(crate) async fn export_csv(
    path: &str,
    records: &[metrics::MetricRecord],
) -> Result<(), std::io::Error> {
    let file = tokio::fs::File::create(path).await?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(b"elapsed_ms,latency_ms,status_code,timed_out,transport_error,response_bytes,in_flight_ops\n")
        .await?;
    for record in records {
        let line = format!(
            "{},{},{},{},{},{},{}\n",
            record.elapsed_ms,
            record.latency_ms,
            record.status_code,
            u8::from(record.timed_out),
            u8::from(record.transport_error),
            record.response_bytes,
            record.in_flight_ops
        );
        writer.write_all(line.as_bytes()).await?;
    }
    writer.flush().await?;
    Ok(())
}

pub(crate) async fn export_json(
    path: &str,
    summary: &metrics::MetricsSummary,
    records: &[metrics::MetricRecord],
) -> Result<(), std::io::Error> {
    let records_json: Vec<serde_json::Value> = records
        .iter()
        .map(|record| {
            serde_json::json!({
                "elapsed_ms": record.elapsed_ms,
                "latency_ms": record.latency_ms,
                "status_code": record.status_code,
                "timed_out": record.timed_out,
                "transport_error": record.transport_error,
                "response_bytes": record.response_bytes,
                "in_flight_ops": record.in_flight_ops
            })
        })
        .collect();

    let (total_response_bytes, avg_response_bytes_per_sec, max_in_flight_ops, last_in_flight_ops) =
        flow_summary(records, summary.duration);
    let summary_json = serde_json::json!({
        "duration_ms": summary.duration.as_millis(),
        "total_requests": summary.total_requests,
        "successful_requests": summary.successful_requests,
        "error_requests": summary.error_requests,
        "timeout_requests": summary.timeout_requests,
        "transport_errors": summary.transport_errors,
        "non_expected_status": summary.non_expected_status,
        "success_min_latency_ms": summary.success_min_latency_ms,
        "success_max_latency_ms": summary.success_max_latency_ms,
        "success_avg_latency_ms": summary.success_avg_latency_ms,
        "min_latency_ms": summary.min_latency_ms,
        "max_latency_ms": summary.max_latency_ms,
        "avg_latency_ms": summary.avg_latency_ms,
        "total_response_bytes": total_response_bytes,
        "avg_response_bytes_per_sec": avg_response_bytes_per_sec,
        "max_in_flight_ops": max_in_flight_ops,
        "last_in_flight_ops": last_in_flight_ops
    });

    let payload = serde_json::json!({
        "summary": summary_json,
        "records": records_json
    });

    let file = tokio::fs::File::create(path).await?;
    let mut writer = BufWriter::new(file);
    let json = serde_json::to_vec_pretty(&payload).map_err(std::io::Error::other)?;
    writer.write_all(&json).await?;
    writer.flush().await?;
    Ok(())
}

pub(crate) async fn export_jsonl(
    path: &str,
    summary: &metrics::MetricsSummary,
    records: &[metrics::MetricRecord],
) -> Result<(), std::io::Error> {
    let file = tokio::fs::File::create(path).await?;
    let mut writer = BufWriter::new(file);

    let (total_response_bytes, avg_response_bytes_per_sec, max_in_flight_ops, last_in_flight_ops) =
        flow_summary(records, summary.duration);
    let summary_json = serde_json::json!({
        "type": "summary",
        "duration_ms": summary.duration.as_millis(),
        "total_requests": summary.total_requests,
        "successful_requests": summary.successful_requests,
        "error_requests": summary.error_requests,
        "timeout_requests": summary.timeout_requests,
        "transport_errors": summary.transport_errors,
        "non_expected_status": summary.non_expected_status,
        "success_min_latency_ms": summary.success_min_latency_ms,
        "success_max_latency_ms": summary.success_max_latency_ms,
        "success_avg_latency_ms": summary.success_avg_latency_ms,
        "min_latency_ms": summary.min_latency_ms,
        "max_latency_ms": summary.max_latency_ms,
        "avg_latency_ms": summary.avg_latency_ms,
        "total_response_bytes": total_response_bytes,
        "avg_response_bytes_per_sec": avg_response_bytes_per_sec,
        "max_in_flight_ops": max_in_flight_ops,
        "last_in_flight_ops": last_in_flight_ops
    });
    let summary_line = serde_json::to_vec(&summary_json).map_err(std::io::Error::other)?;
    writer.write_all(&summary_line).await?;
    writer.write_all(b"\n").await?;

    for record in records {
        let line = serde_json::json!({
            "type": "record",
            "elapsed_ms": record.elapsed_ms,
            "latency_ms": record.latency_ms,
            "status_code": record.status_code,
            "timed_out": record.timed_out,
            "transport_error": record.transport_error,
            "response_bytes": record.response_bytes,
            "in_flight_ops": record.in_flight_ops
        });
        let line_bytes = serde_json::to_vec(&line).map_err(std::io::Error::other)?;
        writer.write_all(&line_bytes).await?;
        writer.write_all(b"\n").await?;
    }

    writer.flush().await?;
    Ok(())
}

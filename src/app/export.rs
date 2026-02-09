use crate::metrics;
use tokio::io::{AsyncWriteExt, BufWriter};

pub(crate) async fn export_csv(
    path: &str,
    records: &[metrics::MetricRecord],
) -> Result<(), std::io::Error> {
    let file = tokio::fs::File::create(path).await?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(b"elapsed_ms,latency_ms,status_code,timed_out,transport_error\n")
        .await?;
    for record in records {
        let line = format!(
            "{},{},{},{},{}\n",
            record.elapsed_ms,
            record.latency_ms,
            record.status_code,
            u8::from(record.timed_out),
            u8::from(record.transport_error)
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
                "transport_error": record.transport_error
            })
        })
        .collect();

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
        "avg_latency_ms": summary.avg_latency_ms
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
        "avg_latency_ms": summary.avg_latency_ms
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
            "transport_error": record.transport_error
        });
        let line_bytes = serde_json::to_vec(&line).map_err(std::io::Error::other)?;
        writer.write_all(&line_bytes).await?;
        writer.write_all(b"\n").await?;
    }

    writer.flush().await?;
    Ok(())
}

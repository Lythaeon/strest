use super::config::{
    InfluxSinkConfig, OtelSinkConfig, PrometheusSinkConfig, SinkStats, SinksConfig,
};
use super::format::{format_x100, write_line};

/// Write configured sink outputs to their destinations.
///
/// # Errors
///
/// Returns an error if any sink output fails to serialize or write.
pub async fn write_sinks(config: &SinksConfig, stats: &SinkStats) -> Result<(), String> {
    if let Some(prom) = config.prometheus.as_ref() {
        write_prometheus(prom, stats).await?;
    }
    if let Some(otel) = config.otel.as_ref() {
        write_otel(otel, stats).await?;
    }
    if let Some(influx) = config.influx.as_ref() {
        write_influx(influx, stats).await?;
    }
    Ok(())
}

async fn write_prometheus(config: &PrometheusSinkConfig, stats: &SinkStats) -> Result<(), String> {
    let mut output = String::new();

    write_line(
        &mut output,
        "# HELP strest_duration_seconds Duration of the measured run in seconds.",
    )?;
    write_line(&mut output, "# TYPE strest_duration_seconds gauge")?;
    write_line(
        &mut output,
        &format!("strest_duration_seconds {}", stats.duration.as_secs()),
    )?;

    write_line(
        &mut output,
        "# HELP strest_requests_total Total number of requests.",
    )?;
    write_line(&mut output, "# TYPE strest_requests_total counter")?;
    write_line(
        &mut output,
        &format!("strest_requests_total {}", stats.total_requests),
    )?;

    write_line(
        &mut output,
        "# HELP strest_requests_success_total Successful requests.",
    )?;
    write_line(&mut output, "# TYPE strest_requests_success_total counter")?;
    write_line(
        &mut output,
        &format!(
            "strest_requests_success_total {}",
            stats.successful_requests
        ),
    )?;

    write_line(
        &mut output,
        "# HELP strest_requests_error_total Failed requests.",
    )?;
    write_line(&mut output, "# TYPE strest_requests_error_total counter")?;
    write_line(
        &mut output,
        &format!("strest_requests_error_total {}", stats.error_requests),
    )?;

    write_line(
        &mut output,
        "# HELP strest_requests_timeout_total Timed-out requests.",
    )?;
    write_line(&mut output, "# TYPE strest_requests_timeout_total counter")?;
    write_line(
        &mut output,
        &format!("strest_requests_timeout_total {}", stats.timeout_requests),
    )?;

    write_line(
        &mut output,
        "# HELP strest_success_rate Success rate (percentage).",
    )?;
    write_line(&mut output, "# TYPE strest_success_rate gauge")?;
    write_line(
        &mut output,
        &format!(
            "strest_success_rate {}",
            format_x100(stats.success_rate_x100)
        ),
    )?;

    write_line(
        &mut output,
        "# HELP strest_avg_rps Average requests per second.",
    )?;
    write_line(&mut output, "# TYPE strest_avg_rps gauge")?;
    write_line(
        &mut output,
        &format!("strest_avg_rps {}", format_x100(stats.avg_rps_x100)),
    )?;

    write_line(
        &mut output,
        "# HELP strest_avg_rpm Average requests per minute.",
    )?;
    write_line(&mut output, "# TYPE strest_avg_rpm gauge")?;
    write_line(
        &mut output,
        &format!("strest_avg_rpm {}", format_x100(stats.avg_rpm_x100)),
    )?;

    write_line(
        &mut output,
        "# HELP strest_latency_ms Request latency in milliseconds.",
    )?;
    write_line(&mut output, "# TYPE strest_latency_ms gauge")?;
    write_line(
        &mut output,
        &format!("strest_latency_ms{{stat=\"min\"}} {}", stats.min_latency_ms),
    )?;
    write_line(
        &mut output,
        &format!("strest_latency_ms{{stat=\"avg\"}} {}", stats.avg_latency_ms),
    )?;
    write_line(
        &mut output,
        &format!("strest_latency_ms{{stat=\"max\"}} {}", stats.max_latency_ms),
    )?;
    write_line(
        &mut output,
        &format!(
            "strest_latency_ms{{quantile=\"0.5\"}} {}",
            stats.p50_latency_ms
        ),
    )?;
    write_line(
        &mut output,
        &format!(
            "strest_latency_ms{{quantile=\"0.9\"}} {}",
            stats.p90_latency_ms
        ),
    )?;
    write_line(
        &mut output,
        &format!(
            "strest_latency_ms{{quantile=\"0.99\"}} {}",
            stats.p99_latency_ms
        ),
    )?;

    tokio::fs::write(&config.path, output)
        .await
        .map_err(|err| format!("Failed to write Prometheus sink: {}", err))?;
    Ok(())
}

async fn write_otel(config: &OtelSinkConfig, stats: &SinkStats) -> Result<(), String> {
    let payload = serde_json::json!({
        "resource": {
            "service.name": "strest"
        },
        "metrics": [
            { "name": "strest.duration", "unit": "s", "value": stats.duration.as_secs() },
            { "name": "strest.requests_total", "value": stats.total_requests },
            { "name": "strest.requests_success_total", "value": stats.successful_requests },
            { "name": "strest.requests_error_total", "value": stats.error_requests },
            { "name": "strest.requests_timeout_total", "value": stats.timeout_requests },
            { "name": "strest.latency_min_ms", "value": stats.min_latency_ms },
            { "name": "strest.latency_avg_ms", "value": stats.avg_latency_ms },
            { "name": "strest.latency_max_ms", "value": stats.max_latency_ms },
            { "name": "strest.latency_p50_ms", "value": stats.p50_latency_ms },
            { "name": "strest.latency_p90_ms", "value": stats.p90_latency_ms },
            { "name": "strest.latency_p99_ms", "value": stats.p99_latency_ms },
            { "name": "strest.success_rate", "value": format_x100(stats.success_rate_x100) },
            { "name": "strest.avg_rps", "value": format_x100(stats.avg_rps_x100) },
            { "name": "strest.avg_rpm", "value": format_x100(stats.avg_rpm_x100) }
        ]
    });

    let json = serde_json::to_vec_pretty(&payload)
        .map_err(|err| format!("Failed to serialize OTel sink: {}", err))?;
    tokio::fs::write(&config.path, json)
        .await
        .map_err(|err| format!("Failed to write OTel sink: {}", err))?;
    Ok(())
}

async fn write_influx(config: &InfluxSinkConfig, stats: &SinkStats) -> Result<(), String> {
    let line = format!(
        "strest_summary duration_ms={}i,total_requests={}i,successful_requests={}i,error_requests={}i,timeout_requests={}i,min_latency_ms={}i,max_latency_ms={}i,avg_latency_ms={}i,p50_latency_ms={}i,p90_latency_ms={}i,p99_latency_ms={}i,success_rate={},avg_rps={},avg_rpm={}\n",
        stats.duration.as_millis(),
        stats.total_requests,
        stats.successful_requests,
        stats.error_requests,
        stats.timeout_requests,
        stats.min_latency_ms,
        stats.max_latency_ms,
        stats.avg_latency_ms,
        stats.p50_latency_ms,
        stats.p90_latency_ms,
        stats.p99_latency_ms,
        format_x100(stats.success_rate_x100),
        format_x100(stats.avg_rps_x100),
        format_x100(stats.avg_rpm_x100)
    );

    tokio::fs::write(&config.path, line)
        .await
        .map_err(|err| format!("Failed to write Influx sink: {}", err))?;
    Ok(())
}

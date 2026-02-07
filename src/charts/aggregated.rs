use std::collections::BTreeMap;

use plotters::prelude::*;

use crate::metrics::AggregatedMetricSample;

fn sorted_samples(samples: &[AggregatedMetricSample]) -> Vec<AggregatedMetricSample> {
    let mut out = samples.to_vec();
    out.sort_by_key(|sample| sample.elapsed_ms);
    out
}

fn bucket_last_value(
    samples: &[AggregatedMetricSample],
    bucket_ms: u64,
    value: fn(&AggregatedMetricSample) -> u64,
) -> Vec<(u64, u64)> {
    let mut buckets: BTreeMap<u64, u64> = BTreeMap::new();
    let bucket_ms = bucket_ms.max(1);
    for sample in samples {
        let bucket = sample.elapsed_ms.checked_div(bucket_ms).unwrap_or(0);
        buckets.insert(bucket, value(sample));
    }
    buckets.into_iter().collect()
}

fn bucket_last_value_u64(
    samples: &[AggregatedMetricSample],
    bucket_ms: u64,
    value: fn(&AggregatedMetricSample) -> u64,
) -> Vec<(u64, u64)> {
    bucket_last_value(samples, bucket_ms, value)
}

fn compute_rps_series(samples: &[AggregatedMetricSample]) -> Vec<(u64, u64)> {
    let sorted = sorted_samples(samples);
    let mut buckets: BTreeMap<u64, u64> = BTreeMap::new();
    for window in sorted.windows(2) {
        let Some(prev) = window.first() else { continue };
        let Some(curr) = window.get(1) else { continue };
        if curr.elapsed_ms <= prev.elapsed_ms {
            continue;
        }
        let delta = curr.total_requests.saturating_sub(prev.total_requests);
        let delta_ms = curr.elapsed_ms.saturating_sub(prev.elapsed_ms).max(1);
        let rps = u64::try_from(
            u128::from(delta)
                .saturating_mul(1000)
                .checked_div(u128::from(delta_ms))
                .unwrap_or(0),
        )
        .unwrap_or(u64::MAX);
        let sec_bucket = curr.elapsed_ms.checked_div(1000).unwrap_or(0);
        buckets.insert(sec_bucket, rps);
    }
    buckets.into_iter().collect()
}

pub fn plot_aggregated_average_response_time(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Ok(());
    }
    let data = bucket_last_value_u64(samples, 100, |sample| sample.avg_latency_ms);
    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap_or(1)
        .saturating_add(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Average Response Time", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0u64..x_max, 0u64..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Avg Response Time (ms)")
        .x_labels(20)
        .y_labels(10)
        .x_label_formatter(&|v| format!("{}.{}s", v / 10, v % 10))
        .draw()?;

    chart.draw_series(LineSeries::new(data.into_iter(), &BLUE))?;

    root.present()?;
    Ok(())
}

pub fn plot_aggregated_cumulative_successful_requests(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Ok(());
    }
    let data = bucket_last_value_u64(samples, 100, |sample| sample.successful_requests);
    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data.last().map(|(_, y)| y.saturating_add(1)).unwrap_or(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Cumulative Successful Requests", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(0u64..x_max, 0u64..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Successful Requests")
        .x_labels(20)
        .y_labels(10)
        .x_label_formatter(&|v| format!("{}.{}s", v / 10, v % 10))
        .draw()?;

    chart.draw_series(LineSeries::new(data.into_iter(), &BLUE))?;
    root.present()?;
    Ok(())
}

pub fn plot_aggregated_cumulative_error_rate(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Ok(());
    }
    let data = bucket_last_value_u64(samples, 100, |sample| sample.error_requests);
    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data.last().map(|(_, y)| y.saturating_add(1)).unwrap_or(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Cumulative Error Rate", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(0u64..x_max, 0u64..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Error Requests")
        .x_labels(20)
        .y_labels(10)
        .x_label_formatter(&|v| format!("{}.{}s", v / 10, v % 10))
        .draw()?;

    chart.draw_series(LineSeries::new(data.into_iter(), &RED))?;
    root.present()?;
    Ok(())
}

pub fn plot_aggregated_cumulative_total_requests(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Ok(());
    }
    let data = bucket_last_value_u64(samples, 100, |sample| sample.total_requests);
    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data.last().map(|(_, y)| y.saturating_add(1)).unwrap_or(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Cumulative Total Requests", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(0u64..x_max, 0u64..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Total Requests")
        .x_labels(20)
        .y_labels(10)
        .x_label_formatter(&|v| format!("{}.{}s", v / 10, v % 10))
        .draw()?;

    chart.draw_series(LineSeries::new(data.into_iter(), &BLACK))?;
    root.present()?;
    Ok(())
}

pub fn plot_aggregated_latency_percentiles(
    samples: &[AggregatedMetricSample],
    base_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Ok(());
    }
    let mut p50s = bucket_last_value_u64(samples, 1000, |sample| sample.p50_latency_ms);
    let mut p90s = bucket_last_value_u64(samples, 1000, |sample| sample.p90_latency_ms);
    let mut p99s = bucket_last_value_u64(samples, 1000, |sample| sample.p99_latency_ms);

    p50s.sort_by_key(|(x, _)| *x);
    p90s.sort_by_key(|(x, _)| *x);
    p99s.sort_by_key(|(x, _)| *x);

    let x_min = p50s.first().map(|(x, _)| *x).unwrap_or(0);
    let x_max = p50s.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);

    let y_max_p50 = p50s
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap_or(1)
        .saturating_add(1);
    let y_max_p90 = p90s
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap_or(1)
        .saturating_add(1);
    let y_max_p99 = p99s
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap_or(1)
        .saturating_add(1);

    let draw_chart = |series: &[(u64, u64)],
                      title: &str,
                      color: RGBColor,
                      file_path: &str,
                      y_max: u64|
     -> Result<(), Box<dyn std::error::Error>> {
        let root = BitMapBackend::new(file_path, (1600, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 30))
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(50)
            .build_cartesian_2d(x_min..x_max, 0u64..y_max)?;

        chart
            .configure_mesh()
            .x_desc("Elapsed Time (s)")
            .y_desc("Latency (ms)")
            .draw()?;

        chart.draw_series(LineSeries::new(series.iter().copied(), &color))?;
        root.present()?;
        Ok(())
    };

    draw_chart(
        &p50s,
        "Latency P50",
        BLUE,
        &format!("{}_P50.png", base_path),
        y_max_p50,
    )?;
    draw_chart(
        &p90s,
        "Latency P90",
        GREEN,
        &format!("{}_P90.png", base_path),
        y_max_p90,
    )?;
    draw_chart(
        &p99s,
        "Latency P99",
        RED,
        &format!("{}_P99.png", base_path),
        y_max_p99,
    )?;

    Ok(())
}

pub fn plot_aggregated_requests_per_second(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = compute_rps_series(samples);
    if data.is_empty() {
        return Ok(());
    }
    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap_or(1)
        .saturating_add(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Requests per Second", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0u64..x_max, 0u64..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Requests per Second")
        .draw()?;

    chart.draw_series(LineSeries::new(data.into_iter(), &BLUE))?;

    root.present()?;
    Ok(())
}

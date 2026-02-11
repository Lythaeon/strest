use std::collections::BTreeMap;

use plotters::prelude::*;

use crate::error::AppResult;
use crate::metrics::MetricRecord;

pub fn plot_cumulative_successful_requests(
    metrics: &[MetricRecord],
    expected_status_code: u16,
    path: &str,
) -> AppResult<()> {
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut success_buckets: BTreeMap<u64, u64> = BTreeMap::new();

    for metric in metrics {
        if metric.status_code == expected_status_code {
            let elapsed_ms = metric.elapsed_ms;
            let bucket = elapsed_ms / 100; // 100ms buckets
            let entry = success_buckets.entry(bucket).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }

    let max_bucket = *success_buckets.keys().max().unwrap_or(&0);
    let mut cumulative: u64 = 0;
    let mut data: Vec<(u64, u64)> = Vec::with_capacity(max_bucket.saturating_add(1) as usize);

    for bucket in 0..=max_bucket {
        let count = *success_buckets.get(&bucket).unwrap_or(&0);
        cumulative = cumulative.saturating_add(count);
        data.push((bucket, cumulative));
    }

    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data.last().map(|(_, y)| y.saturating_add(1)).unwrap_or(1);

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

pub fn plot_cumulative_error_rate(
    metrics: &[MetricRecord],
    expected_status_code: u16,
    path: &str,
) -> AppResult<()> {
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut error_buckets: BTreeMap<u64, u64> = BTreeMap::new();

    for metric in metrics {
        if metric.status_code != expected_status_code {
            let elapsed_ms = metric.elapsed_ms;
            let bucket = elapsed_ms / 100; // 100ms buckets
            let entry = error_buckets.entry(bucket).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }

    let max_bucket = *error_buckets.keys().max().unwrap_or(&0);
    let mut cumulative: u64 = 0;
    let mut data: Vec<(u64, u64)> = Vec::with_capacity(max_bucket.saturating_add(1) as usize);

    for bucket in 0..=max_bucket {
        let count = *error_buckets.get(&bucket).unwrap_or(&0);
        cumulative = cumulative.saturating_add(count);
        data.push((bucket, cumulative));
    }

    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data.last().map(|(_, y)| y.saturating_add(1)).unwrap_or(1);

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

pub fn plot_cumulative_total_requests(metrics: &[MetricRecord], path: &str) -> AppResult<()> {
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut buckets: BTreeMap<u64, u64> = BTreeMap::new();

    for metric in metrics {
        let elapsed_ms = metric.elapsed_ms;
        let bucket = elapsed_ms / 100; // 100ms buckets
        let entry = buckets.entry(bucket).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    let max_bucket = *buckets.keys().max().unwrap_or(&0);
    let mut cumulative: u64 = 0;
    let mut data: Vec<(u64, u64)> = Vec::with_capacity(max_bucket.saturating_add(1) as usize);

    for bucket in 0..=max_bucket {
        let count = *buckets.get(&bucket).unwrap_or(&0);
        cumulative = cumulative.saturating_add(count);
        data.push((bucket, cumulative));
    }

    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data.last().map(|(_, y)| y.saturating_add(1)).unwrap_or(1);

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

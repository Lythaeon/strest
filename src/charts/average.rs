use std::collections::BTreeMap;

use plotters::prelude::*;

use crate::error::AppResult;
use crate::metrics::MetricRecord;

pub fn plot_average_response_time(metrics: &[MetricRecord], path: &str) -> AppResult<()> {
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut buckets: BTreeMap<u64, Vec<u128>> = BTreeMap::new();

    for metric in metrics {
        let elapsed_ms = metric.elapsed_ms;
        let bucket_key = elapsed_ms / 100; // 100ms granularity
        buckets
            .entry(bucket_key)
            .or_default()
            .push(u128::from(metric.latency_ms));
    }

    let mut data: Vec<(u64, u64)> = buckets
        .into_iter()
        .map(|(bucket, times)| {
            let sum_ms = times.iter().sum::<u128>();
            let len = times.len().max(1);
            let len_u128 = u128::from(len as u64);
            let avg_ms = sum_ms.checked_div(len_u128).unwrap_or(0);
            let avg_ms_u64 = u64::try_from(avg_ms).unwrap_or(u64::MAX);
            (bucket, avg_ms_u64)
        })
        .collect();

    data.sort_by_key(|(bucket, _)| *bucket);

    let x_max = data.last().map(|(x, _)| x.saturating_add(1)).unwrap_or(1);
    let y_max = data
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap_or(1000)
        .saturating_add(1);

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

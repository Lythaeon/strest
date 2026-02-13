use std::collections::BTreeMap;

use plotters::prelude::*;

use crate::error::AppResult;

pub fn plot_average_response_time_from_buckets(
    buckets: &BTreeMap<u64, (u128, u64)>,
    path: &str,
) -> AppResult<()> {
    if buckets.is_empty() {
        return Ok(());
    }
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut data: Vec<(u64, u64)> = buckets
        .iter()
        .map(|(bucket, (sum_ms, count))| {
            let len = (*count).max(1);
            let avg_ms = sum_ms.checked_div(u128::from(len)).unwrap_or(0);
            (*bucket, u64::try_from(avg_ms).unwrap_or(u64::MAX))
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

pub fn plot_cumulative_successful_requests_from_buckets(
    success_buckets: &BTreeMap<u64, u64>,
    path: &str,
) -> AppResult<()> {
    if success_buckets.is_empty() {
        return Ok(());
    }
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

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

pub fn plot_cumulative_error_rate_from_buckets(
    error_buckets: &BTreeMap<u64, u64>,
    path: &str,
) -> AppResult<()> {
    if error_buckets.is_empty() {
        return Ok(());
    }
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

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

pub fn plot_cumulative_total_requests_from_buckets(
    total_buckets: &BTreeMap<u64, u64>,
    path: &str,
) -> AppResult<()> {
    if total_buckets.is_empty() {
        return Ok(());
    }
    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let max_bucket = *total_buckets.keys().max().unwrap_or(&0);
    let mut cumulative: u64 = 0;
    let mut data: Vec<(u64, u64)> = Vec::with_capacity(max_bucket.saturating_add(1) as usize);

    for bucket in 0..=max_bucket {
        let count = *total_buckets.get(&bucket).unwrap_or(&0);
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

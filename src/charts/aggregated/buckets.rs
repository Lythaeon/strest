use plotters::prelude::*;

use crate::error::AppResult;
use crate::metrics::AggregatedMetricSample;

use super::util::bucket_last_value_u64;

pub fn plot_aggregated_average_response_time(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> AppResult<()> {
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
) -> AppResult<()> {
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
) -> AppResult<()> {
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
) -> AppResult<()> {
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

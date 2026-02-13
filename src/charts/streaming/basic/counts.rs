use plotters::prelude::*;

use crate::error::AppResult;

pub fn plot_requests_per_second_from_counts(counts: &[u32], path: &str) -> AppResult<()> {
    if counts.is_empty() {
        return Ok(());
    }

    let max_sec = counts.len().saturating_sub(1) as u32;
    let x_range = 0u32..max_sec.saturating_add(1);
    let y_max = *counts.iter().max().unwrap_or(&1);
    let y_range = 0u32..y_max.saturating_add(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Requests per Second", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Requests per Second")
        .draw()?;

    chart.draw_series(LineSeries::new(
        counts
            .iter()
            .enumerate()
            .map(|(sec, &count)| (sec as u32, count)),
        &BLUE,
    ))?;

    root.present()?;
    Ok(())
}

pub fn plot_timeouts_per_second_from_counts(counts: &[u32], path: &str) -> AppResult<()> {
    if counts.is_empty() {
        return Ok(());
    }

    let max_sec = counts.len().saturating_sub(1) as u32;
    let x_range = 0u32..max_sec.saturating_add(1);
    let y_max = *counts.iter().max().unwrap_or(&1);
    let y_range = 0u32..y_max.saturating_add(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Timeouts per Second", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Timeouts per Second")
        .draw()?;

    chart.draw_series(LineSeries::new(
        counts
            .iter()
            .enumerate()
            .map(|(sec, &count)| (sec as u32, count)),
        &RED,
    ))?;

    root.present()?;
    Ok(())
}

pub fn plot_inflight_requests_from_counts(inflight: &[u32], path: &str) -> AppResult<()> {
    if inflight.is_empty() {
        return Ok(());
    }

    let max_sec = inflight.len().saturating_sub(1) as u32;
    let x_range = 0u32..max_sec.saturating_add(1);
    let y_max = inflight.iter().copied().max().unwrap_or(1);
    let y_range = 0u32..y_max.saturating_add(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("In-Flight Requests", ("sans-serif", 30).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Concurrent Requests")
        .draw()?;

    chart.draw_series(LineSeries::new(
        inflight
            .iter()
            .enumerate()
            .map(|(sec, &count)| (sec as u32, count)),
        BLUE,
    ))?;

    root.present()?;
    Ok(())
}

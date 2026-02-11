use plotters::prelude::*;

use crate::error::AppResult;
use crate::metrics::MetricRecord;

pub fn plot_requests_per_second(metrics: &[MetricRecord], path: &str) -> AppResult<()> {
    if metrics.is_empty() {
        return Ok(());
    }

    let max_sec = metrics
        .iter()
        .map(|metric| metric.elapsed_ms / 1000)
        .max()
        .unwrap_or(0);

    let mut counts = vec![0u32; max_sec.saturating_add(1) as usize];
    for metric in metrics {
        let sec = (metric.elapsed_ms / 1000) as usize;
        if let Some(slot) = counts.get_mut(sec) {
            *slot = slot.saturating_add(1);
        }
    }

    let max_sec_u32 = u32::try_from(max_sec).unwrap_or(u32::MAX);
    let x_range = 0u32..max_sec_u32.saturating_add(1);
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

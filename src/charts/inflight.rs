use plotters::prelude::*;

use crate::metrics::MetricRecord;

pub fn plot_inflight_requests(
    metrics: &[MetricRecord],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if metrics.is_empty() {
        return Ok(());
    }

    let max_sec = metrics
        .iter()
        .map(|metric| metric.elapsed_ms.saturating_add(metric.latency_ms) / 1000)
        .max()
        .unwrap_or(0);

    let len = max_sec.saturating_add(2) as usize;
    let mut deltas = vec![0i64; len];

    for metric in metrics {
        let start_sec = (metric.elapsed_ms / 1000) as usize;
        let end_total_ms = metric.elapsed_ms.saturating_add(metric.latency_ms);
        let end_sec = (end_total_ms / 1000) as usize;
        if let Some(slot) = deltas.get_mut(start_sec) {
            *slot = slot.saturating_add(1);
        }
        let end_idx = end_sec.saturating_add(1);
        if let Some(slot) = deltas.get_mut(end_idx) {
            *slot = slot.saturating_sub(1);
        }
    }

    let mut inflight = Vec::with_capacity(len);
    let mut current: i64 = 0;
    for delta in deltas {
        current = current.saturating_add(delta);
        inflight.push(u32::try_from(current.max(0)).unwrap_or(u32::MAX));
    }

    let max_sec_u32 = u32::try_from(max_sec).unwrap_or(u32::MAX);
    let x_range = 0u32..max_sec_u32.saturating_add(1);
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
            .take(max_sec.saturating_add(1) as usize)
            .enumerate()
            .map(|(sec, &count)| (sec as u32, count)),
        BLUE,
    ))?;

    root.present()?;
    Ok(())
}

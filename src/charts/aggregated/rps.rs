use plotters::prelude::*;

use crate::error::AppResult;
use crate::metrics::AggregatedMetricSample;

use super::util::compute_rps_series;

pub fn plot_aggregated_requests_per_second(
    samples: &[AggregatedMetricSample],
    path: &str,
) -> AppResult<()> {
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

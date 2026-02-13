use plotters::prelude::*;

use crate::error::AppResult;
use crate::metrics::AggregatedMetricSample;

use super::util::bucket_last_value_u64;

pub fn plot_aggregated_latency_percentiles(
    samples: &[AggregatedMetricSample],
    base_path: &str,
) -> AppResult<()> {
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
     -> AppResult<()> {
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

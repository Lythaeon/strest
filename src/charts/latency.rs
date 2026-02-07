use std::collections::BTreeMap;

use plotters::prelude::*;

use crate::metrics::MetricRecord;

pub fn plot_latency_percentiles(
    metrics: &[MetricRecord],
    base_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if metrics.is_empty() {
        return Ok(());
    }

    let mut grouped: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for metric in metrics {
        let sec = metric.elapsed_ms / 1000;
        grouped.entry(sec).or_default().push(metric.latency_ms);
    }

    let mut seconds: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p50s: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p90s: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p99s: Vec<u64> = Vec::with_capacity(grouped.len());

    for (sec, mut times) in grouped {
        times.sort_unstable();
        p50s.push(percentile(&times, 50));
        p90s.push(percentile(&times, 90));
        p99s.push(percentile(&times, 99));
        seconds.push(sec);
    }

    let y_max_p50 = p50s.iter().copied().max().unwrap_or(1).saturating_add(1);
    let y_max_p90 = p90s.iter().copied().max().unwrap_or(1).saturating_add(1);
    let y_max_p99 = p99s.iter().copied().max().unwrap_or(1).saturating_add(1);

    fn draw_chart(
        seconds: &[u64],
        values: &[u64],
        title: &str,
        color: RGBColor,
        file_path: &str,
        y_max: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = BitMapBackend::new(file_path, (1600, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let x_min = *seconds.first().unwrap_or(&0);
        let x_max = seconds.last().copied().unwrap_or(0).saturating_add(1);

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

        let points: Vec<(u64, u64)> = seconds
            .iter()
            .copied()
            .zip(values.iter().copied())
            .collect();
        chart.draw_series(LineSeries::new(points, &color))?;

        root.present()?;
        Ok(())
    }

    draw_chart(
        &seconds,
        &p50s,
        "Latency P50",
        BLUE,
        &format!("{}_P50.png", base_path),
        y_max_p50,
    )?;

    draw_chart(
        &seconds,
        &p90s,
        "Latency P90",
        GREEN,
        &format!("{}_P90.png", base_path),
        y_max_p90,
    )?;

    draw_chart(
        &seconds,
        &p99s,
        "Latency P99",
        RED,
        &format!("{}_P99.png", base_path),
        y_max_p99,
    )?;

    Ok(())
}

fn percentile(values: &[u64], percentile: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let count = values.len().saturating_sub(1) as u64;
    let index = percentile
        .saturating_mul(count)
        .saturating_add(50)
        .checked_div(100)
        .unwrap_or(0);
    let idx = usize::try_from(index).unwrap_or_else(|_| values.len().saturating_sub(1));
    *values.get(idx).unwrap_or(&0)
}

use std::collections::BTreeMap;

use plotters::prelude::*;

use crate::metrics::MetricRecord;

pub fn plot_latency_percentiles(
    metrics: &[MetricRecord],
    expected_status_code: u16,
    base_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if metrics.is_empty() {
        return Ok(());
    }

    let mut grouped: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    let mut grouped_ok: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for metric in metrics {
        let sec = metric.elapsed_ms / 1000;
        grouped.entry(sec).or_default().push(metric.latency_ms);
        if metric.status_code == expected_status_code
            && !metric.timed_out
            && !metric.transport_error
        {
            grouped_ok.entry(sec).or_default().push(metric.latency_ms);
        }
    }

    let mut seconds: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p50s: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p90s: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p99s: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p50s_ok: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p90s_ok: Vec<u64> = Vec::with_capacity(grouped.len());
    let mut p99s_ok: Vec<u64> = Vec::with_capacity(grouped.len());

    for (sec, mut times) in grouped {
        times.sort_unstable();
        p50s.push(percentile(&times, 50));
        p90s.push(percentile(&times, 90));
        p99s.push(percentile(&times, 99));
        let mut ok_times = grouped_ok.remove(&sec).unwrap_or_default();
        ok_times.sort_unstable();
        p50s_ok.push(percentile(&ok_times, 50));
        p90s_ok.push(percentile(&ok_times, 90));
        p99s_ok.push(percentile(&ok_times, 99));
        seconds.push(sec);
    }

    let y_max_p50 = p50s.iter().copied().max().unwrap_or(1).saturating_add(1);
    let y_max_p90 = p90s.iter().copied().max().unwrap_or(1).saturating_add(1);
    let y_max_p99 = p99s.iter().copied().max().unwrap_or(1).saturating_add(1);

    struct LatencySeries {
        title: &'static str,
        values: Vec<u64>,
        ok_values: Vec<u64>,
        color: RGBColor,
        file_path: String,
        y_max: u64,
    }

    fn draw_chart(
        seconds: &[u64],
        series: &LatencySeries,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = BitMapBackend::new(&series.file_path, (1600, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let x_min = *seconds.first().unwrap_or(&0);
        let x_max = seconds.last().copied().unwrap_or(0).saturating_add(1);

        let mut chart = ChartBuilder::on(&root)
            .caption(series.title, ("sans-serif", 30))
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(50)
            .build_cartesian_2d(x_min..x_max, 0u64..series.y_max)?;

        chart
            .configure_mesh()
            .x_desc("Elapsed Time (s)")
            .y_desc("Latency (ms)")
            .draw()?;

        let points: Vec<(u64, u64)> = seconds
            .iter()
            .copied()
            .zip(series.values.iter().copied())
            .collect();
        let ok_points: Vec<(u64, u64)> = seconds
            .iter()
            .copied()
            .zip(series.ok_values.iter().copied())
            .collect();
        chart
            .draw_series(LineSeries::new(points, series.color))?
            .label("All")
            .legend(|(x, y)| {
                PathElement::new(vec![(x, y), (x.saturating_add(20), y)], series.color)
            });
        chart
            .draw_series(LineSeries::new(ok_points, BLACK))?
            .label("OK")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x.saturating_add(20), y)], BLACK));

        chart
            .configure_series_labels()
            .border_style(BLACK)
            .background_style(WHITE.mix(0.8))
            .draw()?;

        root.present()?;
        Ok(())
    }

    let series = [
        LatencySeries {
            title: "Latency P50 (All vs OK)",
            values: p50s,
            ok_values: p50s_ok,
            color: BLUE,
            file_path: format!("{}_P50.png", base_path),
            y_max: y_max_p50,
        },
        LatencySeries {
            title: "Latency P90 (All vs OK)",
            values: p90s,
            ok_values: p90s_ok,
            color: GREEN,
            file_path: format!("{}_P90.png", base_path),
            y_max: y_max_p90,
        },
        LatencySeries {
            title: "Latency P99 (All vs OK)",
            values: p99s,
            ok_values: p99s_ok,
            color: RED,
            file_path: format!("{}_P99.png", base_path),
            y_max: y_max_p99,
        },
    ];

    for item in &series {
        draw_chart(&seconds, item)?;
    }

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

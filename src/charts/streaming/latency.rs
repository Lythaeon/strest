use plotters::prelude::*;

use crate::error::AppResult;

pub struct LatencyPercentilesSeries<'series> {
    pub buckets_ms: &'series [u64],
    pub bucket_ms: u64,
    pub p50: &'series [u64],
    pub p90: &'series [u64],
    pub p99: &'series [u64],
    pub p50_ok: &'series [u64],
    pub p90_ok: &'series [u64],
    pub p99_ok: &'series [u64],
}

pub fn plot_latency_percentiles_series(
    series: &LatencyPercentilesSeries<'_>,
    base_path: &str,
) -> AppResult<()> {
    if series.buckets_ms.is_empty() {
        return Ok(());
    }

    struct LatencySeries<'series> {
        title: &'static str,
        values: &'series [u64],
        color: RGBColor,
        file_path: String,
    }

    fn draw_chart(buckets_ms: &[u64], series: &LatencySeries<'_>, bucket_ms: u64) -> AppResult<()> {
        let root = BitMapBackend::new(&series.file_path, (1600, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut combined: Vec<(u64, u64)> = buckets_ms
            .iter()
            .copied()
            .zip(series.values.iter().copied())
            .collect();
        combined.sort_by_key(|(sec, _)| *sec);

        let x_min = combined.first().map(|(sec, _)| *sec).unwrap_or(0);
        let x_max = combined
            .last()
            .map(|(sec, _)| *sec)
            .unwrap_or(0)
            .saturating_add(1);
        let y_max = combined
            .iter()
            .map(|(_, value)| *value)
            .max()
            .unwrap_or(1)
            .saturating_add(1);

        let mut chart = ChartBuilder::on(&root)
            .caption(series.title, ("sans-serif", 30))
            .margin(10)
            .x_label_area_size(30)
            .y_label_area_size(50)
            .build_cartesian_2d(x_min..x_max, 0u64..y_max)?;

        chart
            .configure_mesh()
            .x_desc("Elapsed Time (ms)")
            .y_desc("Latency (ms)")
            .draw()?;

        let mut segments: Vec<Vec<(u64, u64)>> = Vec::new();
        let mut current: Vec<(u64, u64)> = Vec::new();
        let max_gap = bucket_ms.max(1);
        let mut last_x: Option<u64> = None;

        for (x, y) in combined.iter().copied() {
            if let Some(prev_x) = last_x
                && x.saturating_sub(prev_x) > max_gap
                && !current.is_empty()
            {
                segments.push(std::mem::take(&mut current));
            }
            current.push((x, y));
            last_x = Some(x);
        }
        if !current.is_empty() {
            segments.push(current);
        }

        if let Some((first, rest)) = segments.split_first() {
            chart
                .draw_series(LineSeries::new(first.clone(), series.color))?
                .label("Latency")
                .legend(|(x, y)| {
                    PathElement::new(vec![(x, y), (x.saturating_add(20), y)], series.color)
                });
            for segment in rest {
                chart.draw_series(LineSeries::new(segment.clone(), series.color))?;
            }
        }

        chart
            .configure_series_labels()
            .border_style(BLACK)
            .background_style(WHITE.mix(0.8))
            .draw()?;

        root.present()?;
        Ok(())
    }

    let chart_series = [
        LatencySeries {
            title: "Latency P50 (All)",
            values: series.p50,
            color: BLUE,
            file_path: format!("{}_P50_all.png", base_path),
        },
        LatencySeries {
            title: "Latency P50 (OK)",
            values: series.p50_ok,
            color: BLACK,
            file_path: format!("{}_P50_ok.png", base_path),
        },
        LatencySeries {
            title: "Latency P90 (All)",
            values: series.p90,
            color: GREEN,
            file_path: format!("{}_P90_all.png", base_path),
        },
        LatencySeries {
            title: "Latency P90 (OK)",
            values: series.p90_ok,
            color: BLACK,
            file_path: format!("{}_P90_ok.png", base_path),
        },
        LatencySeries {
            title: "Latency P99 (All)",
            values: series.p99,
            color: RED,
            file_path: format!("{}_P99_all.png", base_path),
        },
        LatencySeries {
            title: "Latency P99 (OK)",
            values: series.p99_ok,
            color: BLACK,
            file_path: format!("{}_P99_ok.png", base_path),
        },
    ];

    for item in &chart_series {
        draw_chart(series.buckets_ms, item, series.bucket_ms)?;
    }

    Ok(())
}

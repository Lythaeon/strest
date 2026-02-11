use std::collections::BTreeMap;

use plotters::prelude::*;

pub fn plot_average_response_time_from_buckets(
    buckets: &BTreeMap<u64, (u128, u64)>,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn plot_requests_per_second_from_counts(
    counts: &[u32],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn plot_timeouts_per_second_from_counts(
    counts: &[u32],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn plot_error_rate_breakdown_from_counts(
    timeouts: &[u32],
    transports: &[u32],
    non_expected: &[u32],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if timeouts.is_empty() && transports.is_empty() && non_expected.is_empty() {
        return Ok(());
    }

    let max_len = timeouts.len().max(transports.len()).max(non_expected.len());
    let max_sec = max_len.saturating_sub(1) as u32;
    let x_range = 0u32..max_sec.saturating_add(1);
    let y_max = (0..max_len)
        .map(|idx| {
            let t = *timeouts.get(idx).unwrap_or(&0);
            let tr = *transports.get(idx).unwrap_or(&0);
            let n = *non_expected.get(idx).unwrap_or(&0);
            t.saturating_add(tr).saturating_add(n)
        })
        .max()
        .unwrap_or(1);
    let y_range = 0u32..y_max.saturating_add(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Error Rate per Second (Breakdown)",
            ("sans-serif", 30).into_font(),
        )
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Errors per Second")
        .draw()?;

    let transport_color = RGBColor(255, 140, 0);
    let non_expected_color = RGBColor(128, 0, 128);

    chart
        .draw_series(LineSeries::new(
            timeouts
                .iter()
                .enumerate()
                .map(|(sec, &count)| (sec as u32, count)),
            RED,
        ))?
        .label("Timeouts")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x.saturating_add(20), y)], RED));

    chart
        .draw_series(LineSeries::new(
            transports
                .iter()
                .enumerate()
                .map(|(sec, &count)| (sec as u32, count)),
            transport_color,
        ))?
        .label("Transport Errors")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x.saturating_add(20), y)], transport_color)
        });

    chart
        .draw_series(LineSeries::new(
            non_expected
                .iter()
                .enumerate()
                .map(|(sec, &count)| (sec as u32, count)),
            non_expected_color,
        ))?
        .label("Non-Expected Status")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x.saturating_add(20), y)], non_expected_color)
        });

    chart
        .configure_series_labels()
        .border_style(BLACK)
        .background_style(WHITE.mix(0.8))
        .draw()?;

    root.present()?;
    Ok(())
}

pub fn plot_status_code_distribution_from_counts(
    counts_2xx: &[u32],
    counts_3xx: &[u32],
    counts_4xx: &[u32],
    counts_5xx: &[u32],
    counts_other: &[u32],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let len = counts_2xx
        .len()
        .max(counts_3xx.len())
        .max(counts_4xx.len())
        .max(counts_5xx.len())
        .max(counts_other.len());
    if len == 0 {
        return Ok(());
    }

    let mut c2 = vec![0u32; len];
    let mut c3 = vec![0u32; len];
    let mut c4 = vec![0u32; len];
    let mut c5 = vec![0u32; len];
    let mut co = vec![0u32; len];

    for i in 0..len {
        if let Some(slot) = c2.get_mut(i) {
            *slot = *counts_2xx.get(i).unwrap_or(&0);
        }
        if let Some(slot) = c3.get_mut(i) {
            *slot = *counts_3xx.get(i).unwrap_or(&0);
        }
        if let Some(slot) = c4.get_mut(i) {
            *slot = *counts_4xx.get(i).unwrap_or(&0);
        }
        if let Some(slot) = c5.get_mut(i) {
            *slot = *counts_5xx.get(i).unwrap_or(&0);
        }
        if let Some(slot) = co.get_mut(i) {
            *slot = *counts_other.get(i).unwrap_or(&0);
        }
    }

    let y_max = c2
        .iter()
        .zip(&c3)
        .zip(&c4)
        .zip(&c5)
        .zip(&co)
        .map(|((((c2, c3), c4), c5), other)| {
            c2.saturating_add(*c3)
                .saturating_add(*c4)
                .saturating_add(*c5)
                .saturating_add(*other)
        })
        .max()
        .unwrap_or(1);

    let root = BitMapBackend::new(path, (1600, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let x_max = u32::try_from(len.saturating_add(1)).unwrap_or(u32::MAX);
    let mut chart = ChartBuilder::on(&root)
        .caption(
            "HTTP Status Code Distribution",
            ("sans-serif", 30).into_font(),
        )
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(0u32..x_max, 0u32..y_max.saturating_add(1))?;

    chart
        .configure_mesh()
        .x_desc("Elapsed Time (seconds)")
        .y_desc("Requests per Second")
        .draw()?;

    let mut base_2xx = Vec::with_capacity(len);
    let mut base_3xx = Vec::with_capacity(len);
    let mut base_4xx = Vec::with_capacity(len);
    let mut base_5xx = Vec::with_capacity(len);
    let mut base_other = Vec::with_capacity(len);

    for (((c2v, c3v), c4v), c5v) in c2.iter().zip(&c3).zip(&c4).zip(&c5) {
        let base_3 = *c2v;
        let base_4 = c2v.saturating_add(*c3v);
        let base_5 = base_4.saturating_add(*c4v);
        let base_o = base_5.saturating_add(*c5v);

        base_2xx.push(0);
        base_3xx.push(base_3);
        base_4xx.push(base_4);
        base_5xx.push(base_5);
        base_other.push(base_o);
    }

    let colors = [
        (RGBColor(46, 204, 113), "2xx"),
        (RGBColor(52, 152, 219), "3xx"),
        (RGBColor(241, 196, 15), "4xx"),
        (RGBColor(231, 76, 60), "5xx"),
        (RGBColor(127, 140, 141), "Other"),
    ];

    let buckets = [
        (&c2, &base_2xx, colors[0]),
        (&c3, &base_3xx, colors[1]),
        (&c4, &base_4xx, colors[2]),
        (&c5, &base_5xx, colors[3]),
        (&co, &base_other, colors[4]),
    ];

    for (counts, base, (color, label)) in buckets {
        chart
            .draw_series(counts.iter().zip(base.iter()).enumerate().filter_map(
                |(sec, (&count, &base_value))| {
                    if count == 0 {
                        return None;
                    }
                    let sec_u32 = u32::try_from(sec).unwrap_or(u32::MAX);
                    let y0 = base_value;
                    let y1 = y0.saturating_add(count);
                    Some(Rectangle::new(
                        [(sec_u32, y0), (sec_u32.saturating_add(1), y1)],
                        color.filled(),
                    ))
                },
            ))?
            .label(label)
            .legend(move |(x, y)| {
                Rectangle::new(
                    [
                        (x, y.saturating_sub(5)),
                        (x.saturating_add(10), y.saturating_add(5)),
                    ],
                    color.filled(),
                )
            });
    }

    chart
        .configure_series_labels()
        .border_style(BLACK)
        .background_style(WHITE.mix(0.8))
        .draw()?;

    root.present()?;
    Ok(())
}

pub fn plot_inflight_requests_from_counts(
    inflight: &[u32],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
    if series.buckets_ms.is_empty() {
        return Ok(());
    }

    struct LatencySeries<'series> {
        title: &'static str,
        values: &'series [u64],
        color: RGBColor,
        file_path: String,
    }

    fn draw_chart(
        buckets_ms: &[u64],
        series: &LatencySeries<'_>,
        bucket_ms: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

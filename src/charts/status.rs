use plotters::prelude::*;

use crate::metrics::MetricRecord;

pub fn plot_status_code_distribution(
    metrics: &[MetricRecord],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if metrics.is_empty() {
        return Ok(());
    }

    let max_sec = metrics
        .iter()
        .map(|metric| metric.elapsed_ms / 1000)
        .max()
        .unwrap_or(0);

    let len = max_sec.saturating_add(1) as usize;
    let mut counts_2xx = vec![0u32; len];
    let mut counts_3xx = vec![0u32; len];
    let mut counts_4xx = vec![0u32; len];
    let mut counts_5xx = vec![0u32; len];
    let mut counts_other = vec![0u32; len];

    for metric in metrics {
        let sec = (metric.elapsed_ms / 1000) as usize;
        if sec >= len {
            continue;
        }
        let code = metric.status_code;
        let bucket = match code {
            200..=299 => &mut counts_2xx,
            300..=399 => &mut counts_3xx,
            400..=499 => &mut counts_4xx,
            500..=599 => &mut counts_5xx,
            _ => &mut counts_other,
        };
        if let Some(slot) = bucket.get_mut(sec) {
            *slot = slot.saturating_add(1);
        }
    }

    let y_max = counts_2xx
        .iter()
        .zip(&counts_3xx)
        .zip(&counts_4xx)
        .zip(&counts_5xx)
        .zip(&counts_other)
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

    let x_max = u32::try_from(max_sec.saturating_add(1)).unwrap_or(u32::MAX);
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

    for (((c2, c3), c4), c5) in counts_2xx
        .iter()
        .zip(&counts_3xx)
        .zip(&counts_4xx)
        .zip(&counts_5xx)
    {
        let base_3 = *c2;
        let base_4 = c2.saturating_add(*c3);
        let base_5 = base_4.saturating_add(*c4);
        let base_o = base_5.saturating_add(*c5);

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
        (&counts_2xx, &base_2xx, colors[0]),
        (&counts_3xx, &base_3xx, colors[1]),
        (&counts_4xx, &base_4xx, colors[2]),
        (&counts_5xx, &base_5xx, colors[3]),
        (&counts_other, &base_other, colors[4]),
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

use plotters::prelude::*;

use crate::metrics::MetricRecord;

pub fn plot_error_rate_breakdown(
    metrics: &[MetricRecord],
    expected_status_code: u16,
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

    let mut timeouts = vec![0u32; max_sec.saturating_add(1) as usize];
    let mut transports = vec![0u32; max_sec.saturating_add(1) as usize];
    let mut non_expected = vec![0u32; max_sec.saturating_add(1) as usize];

    for metric in metrics {
        let sec = (metric.elapsed_ms / 1000) as usize;
        let Some(timeout_slot) = timeouts.get_mut(sec) else {
            continue;
        };

        if metric.timed_out {
            *timeout_slot = timeout_slot.saturating_add(1);
            continue;
        }

        if metric.transport_error {
            if let Some(slot) = transports.get_mut(sec) {
                *slot = slot.saturating_add(1);
            }
            continue;
        }

        if metric.status_code != expected_status_code
            && let Some(slot) = non_expected.get_mut(sec)
        {
            *slot = slot.saturating_add(1);
        }
    }

    let max_sec_u32 = u32::try_from(max_sec).unwrap_or(u32::MAX);
    let x_range = 0u32..max_sec_u32.saturating_add(1);
    let y_max = timeouts
        .iter()
        .zip(transports.iter())
        .zip(non_expected.iter())
        .map(|((t, tr), n)| t.saturating_add(*tr).saturating_add(*n))
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

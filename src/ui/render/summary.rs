use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Backend, Frame},
};

use crate::ui::model::UiRenderData;

use super::summary_panels_metrics::{CompareTotals, reliability_panel, throughput_panel};
use super::summary_panels_quality::{flow_panel, latency_panel};
use super::summary_run::render_run_panel;
use super::theme::{
    SUMMARY_COL_FLOW, SUMMARY_COL_LATENCY, SUMMARY_COL_PROGRESS, SUMMARY_COL_RELIABILITY,
    SUMMARY_COL_THROUGHPUT,
};

pub fn render_summary<B: Backend>(f: &mut Frame<'_, B>, data: &UiRenderData, summary_chunk: Rect) {
    let summary_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(SUMMARY_COL_PROGRESS),
            Constraint::Percentage(SUMMARY_COL_THROUGHPUT),
            Constraint::Percentage(SUMMARY_COL_RELIABILITY),
            Constraint::Percentage(SUMMARY_COL_LATENCY),
            Constraint::Percentage(SUMMARY_COL_FLOW),
        ])
        .split(summary_chunk);

    let (run_chunk, throughput_chunk, reliability_chunk, latency_kpi_chunk, flow_chunk) =
        match summary_chunks.as_ref() {
            [a, b, c, d, e] => (a, b, c, d, e),
            _ => return,
        };

    let total_requests = data.current_request;
    let success_requests = data.successful_requests;
    let error_requests = total_requests.saturating_sub(success_requests);
    let compare_totals: Option<CompareTotals> = data.compare.as_ref().map(|value| {
        let total = value.current_requests;
        let success = value.successful_requests;
        let error = total.saturating_sub(success);
        let success_rate = rate_x100(success, total);
        let error_rate = rate_x100(error, total);
        (total, success, error, success_rate, error_rate)
    });

    let throughput_text = throughput_panel(data, total_requests);
    let reliability_text = reliability_panel(data, error_requests, compare_totals);
    let latency_text = latency_panel(data);
    let flow_text = flow_panel(data);

    render_run_panel(f, data, *run_chunk);
    f.render_widget(throughput_text, *throughput_chunk);
    f.render_widget(reliability_text, *reliability_chunk);
    f.render_widget(latency_text, *latency_kpi_chunk);
    f.render_widget(flow_text, *flow_chunk);
}

fn rate_x100(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let scaled = u128::from(numerator)
        .saturating_mul(10_000)
        .checked_div(u128::from(denominator))
        .unwrap_or(0);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

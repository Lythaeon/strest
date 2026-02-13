mod charts;
mod charts_status_data;
mod charts_window;
mod dashboard;
mod formatting;
mod frame;
mod lifecycle;
mod progress;
mod summary;
mod summary_panels_metrics;
mod summary_panels_quality;
mod summary_run;
mod theme;

#[cfg(test)]
pub use dashboard::{Ui, UiActions};
pub use lifecycle::{run_splash_screen, setup_render_ui};

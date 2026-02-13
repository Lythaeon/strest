mod bounds;
mod records;
mod runner;
mod snapshots;
pub(crate) mod state;
pub(crate) mod summary;
pub(crate) mod ui;

#[cfg(test)]
mod tests;

pub(crate) use records::read_records_from_path;
pub(crate) use runner::run_replay;
use runner::window_slice;
pub(crate) use state::{ReplayWindow, SnapshotMarkers};
pub(crate) use ui::build_ui_data_with_config;

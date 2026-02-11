#[derive(Clone, Copy)]
pub(super) struct ReplayWindow {
    pub(super) start_ms: u64,
    pub(super) cursor_ms: u64,
    pub(super) end_ms: u64,
    pub(super) playing: bool,
}

#[derive(Default)]
pub(super) struct SnapshotMarkers {
    pub(super) start: Option<u64>,
    pub(super) end: Option<u64>,
}

impl SnapshotMarkers {
    pub(super) const fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }
}

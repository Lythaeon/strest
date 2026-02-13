#[derive(Clone, Copy)]
pub(crate) struct ReplayWindow {
    pub(crate) start_ms: u64,
    pub(crate) cursor_ms: u64,
    pub(crate) end_ms: u64,
    pub(crate) playing: bool,
}

#[derive(Default)]
pub(crate) struct SnapshotMarkers {
    pub(crate) start: Option<u64>,
    pub(crate) end: Option<u64>,
}

impl SnapshotMarkers {
    pub(crate) const fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }
}

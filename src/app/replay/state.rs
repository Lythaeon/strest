pub(crate) type ReplayWindow = crate::application::replay_compare::PlaybackState;

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

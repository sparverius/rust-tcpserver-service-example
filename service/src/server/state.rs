use crate::stats::Stats;
use zerocopy::AsBytes;

/// Contains state information about the running service
#[derive(Default, Debug, PartialEq)]
pub struct State {
    stats: Stats,
    total: usize,      // Total bytes received from compression requests
    compressed: usize, // Total bytes sent after compressing valid compress requests
    internal_error: u16,
}

impl State {
    pub fn new() -> State {
        Default::default()
    }

    pub fn stats_as_bytes(&self) -> &[u8] {
        self.stats.as_bytes()
    }

    pub fn internal_error(&self) -> u16 {
        self.internal_error
    }

    pub fn update_read(&mut self, size: usize) {
        self.stats.update_read(size)
    }

    pub fn update_sent(&mut self, size: usize) {
        self.stats.update_sent(size)
    }

    pub fn update_ratio(&mut self, total: usize, compressed: usize) {
        self.total += total;
        self.compressed += compressed;
        self.stats.set_ratio(self.compressed, self.total);
    }

    pub fn reset(&mut self) {
        self.stats.reset();
        self.total = 0;
        self.compressed = 0;
    }

    // used in testing
    pub fn new_with(stats: Stats, total: usize, compressed: usize, internal_error: u16) -> State {
        State {
            stats,
            total,
            compressed,
            internal_error,
        }
    }
}

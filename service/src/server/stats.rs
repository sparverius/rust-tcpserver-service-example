use byteorder::NetworkEndian;
use zerocopy::{byteorder::U32, AsBytes, ByteSlice, FromBytes, LayoutVerified, Unaligned};

/// Useful for keeping track of client server communication
/// Count of all bytes received by the service, including headers
/// sent: Count of all bytes sent by the service, including headers
/// ratio: From 0-100 representing the performance of the compression service
#[derive(Default, Debug, PartialEq, AsBytes, FromBytes, Unaligned)]
#[repr(packed)]
pub struct Stats {
    read: U32<NetworkEndian>,
    sent: U32<NetworkEndian>,
    ratio: u8,
}

impl Stats {
    pub fn new() -> Stats {
        Default::default()
    }

    pub fn new_with(read: u32, sent: u32, ratio: u8) -> Stats {
        Stats {
            read: U32::new(read),
            sent: U32::new(sent),
            ratio,
        }
    }

    pub fn read(&self) -> u32 {
        self.read.get()
    }

    pub fn sent(&self) -> u32 {
        self.sent.get()
    }

    pub fn ratio(&self) -> u8 {
        self.ratio
    }

    pub fn update_read(&mut self, len: usize) {
        self.read.set(self.read.get() + len as u32);
    }

    pub fn update_sent(&mut self, len: usize) {
        self.sent.set(self.sent.get() + len as u32);
    }

    pub fn set_ratio(&mut self, compressed: usize, msg_total: usize) {
        if msg_total > 0 && compressed > 0 {
            let new_ratio = compressed as f64 / msg_total as f64;
            let ratio = (1f64 - new_ratio) * 100f64;
            self.ratio = ratio as u8;
        }
    }

    pub fn reset(&mut self) {
        self.read.set(0);
        self.sent.set(0);
        self.ratio = 0;
    }
}

// used in test-client package
impl Stats {
    pub fn parse<B: ByteSlice>(bytes: B) -> Option<LayoutVerified<B, Stats>> {
        let stats = LayoutVerified::new(bytes)?;
        Some(stats)
    }
}

#[cfg(test)]
mod tests {
    use zerocopy::AsBytes;

    #[test]
    fn test_parse() {
        let msg = [0, 0, 0, 22, 0, 0, 0, 22, 10];
        let stats = super::Stats::parse(&msg[..]);
        assert!(!stats.is_none())
    }

    #[test]
    fn test_as_bytes() {
        let stats = super::Stats::new_with(22, 22, 10);
        assert_eq!(stats.as_bytes(), [0, 0, 0, 22, 0, 0, 0, 22, 10]);
    }
}

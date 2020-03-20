use super::compress::compress_message;
use super::state::State;
use crate::message;
use crate::message::*;

use zerocopy::{ByteSlice, ByteSliceMut};

/// A facade of the underlying receive and transmit slices in the form of
/// `Message`s
///
/// Connection has associated functions for generating a response (into tx)
/// from the request (in rx)
///
/// # Example
/// ```
/// use service::Connection;
/// use service::Message;
/// let mut rx = [83u8, 84, 82, 89, 0, 4, 0, 4, 115, 116, 114, 121];
/// let mut tx = [0u8; 12];
/// let len = 12;
/// {
///     let mut conn = Connection::parse_slices(&mut rx[..], &mut tx[..], len);
///     assert_eq!(conn.rx.header.size(), 4);
///     conn.rx.set_size(0);
/// }
/// let conn = Connection::parse_slices(&mut rx[..], &mut tx[..], len);
/// assert_eq!(conn.rx.header.size(), 0);
/// ```
#[derive(Debug)]
pub struct Connection<Rx: ByteSlice, Tx: ByteSliceMut> {
    pub rx: Message<Rx>,
    pub tx: Message<Tx>,
    pub message_len: usize,
}

impl<Rx, Tx> Connection<Rx, Tx>
where
    Rx: ByteSlice,
    Tx: ByteSliceMut,
{
    pub fn new_with(rx: Rx, tx: Tx, message_len: usize) -> Connection<Rx, Tx> {
        let rx = Message::parse(rx).unwrap();
        let tx = Message::parse_mut(tx).unwrap();
        Connection {
            rx,
            tx,
            message_len,
        }
    }

    pub fn read_payload_len(&self) -> usize {
        message::payload_len(self.message_len) // self.message_len - HEADER_SIZE
    }

    /// Handles the client's query (rx) and constructs response (tx)
    pub fn create_response(&mut self, state: &mut State) -> usize {
        let response_code = self.rx.validate(self.message_len);
        let tx_body_len = match response_code {
            Response::Ok => self.process_response(state),
            _ => 0,
        };
        self.tx
            .set_header(message::MAGIC, tx_body_len, response_code as u16);
        message::total_response_len(tx_body_len as usize) // HEADER_SIZE + tx_body_len
    }

    fn process_response(&mut self, state: &mut State) -> u16 {
        match Request::from_u16(self.rx.header.code()).unwrap() {
            Request::Ping => self.process_ping(state),
            Request::GetStats => self.process_getstats(state),
            Request::ResetStats => self.process_resetstats(state),
            Request::Compress => self.process_compress(state),
        }
    }

    fn process_ping(&mut self, state: &mut State) -> u16 {
        self.tx.set_code(state.internal_error()); // report errors?
        0
    }

    fn process_getstats(&mut self, state: &mut State) -> u16 {
        let stats_bytes = state.stats_as_bytes();
        self.tx.set_payload(stats_bytes).unwrap();
        stats_bytes.len() as u16
    }

    fn process_resetstats(&mut self, state: &mut State) -> u16 {
        state.reset();
        0
    }

    fn process_compress(&mut self, state: &mut State) -> u16 {
        // stats are not updated if the message is invalid
        let payload_len = self.read_payload_len();
        let the_rx = &self.rx.payload[..payload_len];
        let the_tx = &mut self.tx.payload;
        match compress_message(the_rx, the_tx) {
            None => 0,
            Some(compressed_len) => {
                state.update_ratio(payload_len, compressed_len);
                compressed_len as u16
            }
        }
    }
}

impl<Rx: ByteSlice, Tx: ByteSliceMut> Connection<Rx, Tx> {
    #[allow(dead_code)]
    // Used in illustration example above
    pub fn parse_slices(rx: Rx, tx: Tx, len: usize) -> Connection<Rx, Tx> {
        let rx = Message::parse(rx).unwrap();
        let tx = Message::parse_mut(tx).unwrap();
        let message_len = len;
        Connection {
            rx,
            tx,
            message_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Connection, Request, Response, State};
    use crate::stats::Stats;

    fn test_response(bytes_read: usize, rx: &mut [u8], tx: &mut [u8]) -> usize {
        let mut state: State = Default::default();
        Connection::new_with(rx, tx, bytes_read).create_response(&mut state)
    }

    #[test]
    fn test_compress_invalid_characters() {
        let mut rx = [83u8, 84, 82, 89, 0, 1, 0, 4, 65];
        let mut tx = [0u8; 9];
        let bytes_read = rx.len();
        let response_size = test_response(bytes_read, &mut rx, &mut tx);
        let n = Response::MessagePayloadContainsInvalidCharacters as u8;
        let result = [83u8, 84, 82, 89, 0, 0, 0, n];
        assert_eq!(tx[..response_size], result);
    }

    #[test]
    fn test_compress() {
        let request = Request::Compress as u8;
        let rx = [83u8, 84, 82, 89, 0, 3, 0, request, 97, 97, 97];
        let mut tx = [0u8; 11];
        let mut state = State::new();
        state.update_read(11);
        let size = Connection::new_with(&rx[..], &mut tx[..], 11).create_response(&mut state);

        assert_eq!(size, 10);
        assert_eq!(&tx[..size], &[83u8, 84, 82, 89, 0, 2, 0, 0, 51, 97]);

        // State {
        //     stats: Stats {
        //         read: U32::new(0),
        //         sent: U32::new(0),
        //         ratio: 33,
        //     },
        //     total: 3,
        //     compressed: 2,
        //     internal_error: 0,
        // };
        let stats = Stats::new_with(11, 0, 33);
        let expected_state = State::new_with(stats, 3, 2, 0);
        assert_eq!(state, expected_state);
    }

    #[test]
    fn test_ping() {
        let rx = [83u8, 84, 82, 89, 0, 0, 0, Request::Ping as u8];
        let mut tx = [0u8; 8];
        let bytes_read = rx.len();
        let mut state = State::new();
        state.update_read(bytes_read);
        let size =
            Connection::new_with(&rx[..], &mut tx[..], bytes_read).create_response(&mut state);

        assert_eq!(size, 8);
        assert_eq!(&tx[..size], &[83u8, 84, 82, 89, 0, 0, 0, 0]);

        // State {
        //     stats: Stats {
        //         read: U32::new(bytes_read as u32),
        //         sent: U32::new(0),
        //         ratio: 0,
        //     },
        //     total: 0,
        //     compressed: 0,
        //     internal_error: 0,
        // };
        let stats = Stats::new_with(bytes_read as u32, 0, 0);
        let expected_state = State::new_with(stats, 0, 0, 0);
        assert_eq!(state, expected_state);
    }

    #[test]
    fn test_get_stats() {
        let request = Request::Compress as u8;
        let rx = [83u8, 84, 82, 89, 0, 3, 0, request, 97, 97, 97];
        let mut tx = [0u8; 11];
        let mut state = State::new();
        state.update_read(11);
        let size = Connection::new_with(&rx[..], &mut tx[..], 11).create_response(&mut state);
        state.update_sent(size);

        let rx = [83u8, 84, 82, 89, 0, 0, 0, Request::GetStats as u8];
        let mut tx = [0u8; 17];
        let bytes_read = rx.len();

        let size =
            Connection::new_with(&rx[..], &mut tx[..], bytes_read).create_response(&mut state);

        assert_eq!(size, 17);
        assert_eq!(
            &tx[..size],
            &[
                83u8, 84, 82, 89, 0, 9, 0, 0, //
                0, 0, 0, 11, 0, 0, 0, 10, 33
            ]
        );
    }

    #[test]
    fn test_reset_stats() {
        let mut tx = [0u8; 20];
        let mut state = State::new();

        let request = Request::Compress as u8;
        let rx = [83u8, 84, 82, 89, 0, 3, 0, request, 97, 97, 97];
        state.update_read(rx.len());
        let size = Connection::new_with(&rx[..], &mut tx[..], rx.len()).create_response(&mut state);
        state.update_sent(size);

        let request = Request::GetStats as u8;
        let rx = [83u8, 84, 82, 89, 0, 0, 0, request];
        let size = Connection::new_with(&rx[..], &mut tx[..], rx.len()).create_response(&mut state);
        assert_eq!(size, 17);
        assert_eq!(
            &tx[..size],
            &[83u8, 84, 82, 89, 0, 9, 0, 0, 0_u8, 0, 0, 11, 0, 0, 0, 10, 33]
        );

        let request = Request::ResetStats as u8;
        let rx = [83u8, 84, 82, 89, 0, 0, 0, request];
        let size = Connection::new_with(&rx[..], &mut tx[..], rx.len()).create_response(&mut state);
        assert_eq!(size, 8);
        assert_eq!(&tx[..size], &[83u8, 84, 82, 89, 0, 0, 0, 0]);
    }
}

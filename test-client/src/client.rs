use message::{Header, Message, Request, Response};
use service::{message, State};

use bytes::{Bytes, BytesMut};
use futures::SinkExt;
use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
};
use tokio::{net::TcpStream, stream::StreamExt};
use tokio_util::codec::{BytesCodec, Framed};
use zerocopy::AsBytes;

type Result<T> = std::result::Result<T, std::io::Error>;
type BytesFramed = Framed<TcpStream, BytesCodec>;

/// For conducting dynamic testing of the service
pub struct Client {
    url: String,
    state: State,
    results: TestResults,
}

#[derive(Debug, Clone)]
pub enum TestKind {
    Valid,
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Test {
    pub query_kind: Request,
    pub query: Vec<u8>,
    pub expected: Vec<u8>,
    pub validity: TestKind,
}

#[derive(Debug, Default)]
pub struct TestResults {
    count: usize,
    failed: usize,
    passed: usize,
}

impl TestResults {
    pub fn inc_failed(&mut self) {
        self.failed += 1;
    }
    pub fn inc_passed(&mut self) {
        self.passed += 1;
    }
    pub fn inc_count(&mut self) {
        self.count += 1;
    }
}

impl Client {
    pub async fn new_with_url(url: String) -> Result<Client> {
        let state: State = Default::default();
        let results: TestResults = Default::default();
        Ok(Client {
            url,
            state,
            results,
        })
    }

    pub async fn run_with(&mut self, i: usize, cases: Vec<Test>) -> Result<()> {
        match TcpStream::connect(&self.url).await {
            Ok(stream) => {
                // println!("Client({}) @ {}", i, stream.local_addr()?);
                if let Err(e) = self.process(i, stream, cases).await {
                    eprintln!("{}", e)
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("{}", e);
                Err(e)
            }
        }
    }

    fn update_ratio(state: &mut State, test: &Test) {
        let message = Message::parse(&test.query[..]).unwrap();
        if let Request::Compress = Request::from_u16(message.header.code()).unwrap() {
            let compressed = Message::parse(&test.expected[..]).unwrap();
            let total_len = message.payload.len();
            let compressed_len = compressed.payload.len();
            state.update_ratio(total_len, compressed_len);
        }
    }

    fn show_overview(&self, i: usize, addr: SocketAddr) {
        println!("Client({}) @ {:?} : {:?}", i, addr, self.results);
        // for displaying client's state also
        // println!("Client({}) @ {:?} : {:?}\n{:?}", i, addr, self.results, self.state);
    }

    async fn process(&mut self, i: usize, stream: TcpStream, cases: Vec<Test>) -> Result<()> {
        let client_addr = stream.local_addr()?;
        let mut frames = Framed::new(stream, BytesCodec::new());
        for test in cases.iter() {
            println!("({}) count({:?})", i, self.results.count);
            if let Err(e) = self.process_test_case(&mut frames, test).await {
                // return error here to propogate forward otherwise just display test failure
                eprintln!("{:?}", e);
            }
        }
        self.show_overview(i, client_addr);
        Ok(())
    }

    async fn process_test_case(&mut self, frames: &mut BytesFramed, test: &Test) -> Result<()> {
        if let TestKind::Valid = test.validity {
            if test.query.len() >= message::HEADER_SIZE {
                Client::update_ratio(&mut self.state, test);
            }
        }
        match frames.send(Bytes::copy_from_slice(&test.query[..])).await {
            Ok(()) => {
                self.state.update_read(test.query.len());
                // // read next incomming message from socket
                match frames.next().await {
                    Some(Ok(frame)) if frame.is_empty() => Ok(()), // disconnected
                    Some(Ok(frame)) => self.handle_server_response(frame, test),
                    _ => Err(Error::new(ErrorKind::Other, "Server Disconnected")),
                }
            }
            Err(e) => Err(e),
        }
    }

    fn handle_server_response(&mut self, response: BytesMut, test: &Test) -> Result<()> {
        let bytes_read = response.len();
        match test.query_kind {
            Request::GetStats => self.handle_get_stats(response, test),
            Request::ResetStats => self.handle_reset_stats(response, test),
            _ => self.handle_other_requests(response, test),
        }
        self.state.update_sent(bytes_read);
        self.results.inc_count();
        Ok(())
    }

    // no need to propogate errors forward as these are non critical test errors
    fn handle_get_stats(&mut self, response: BytesMut, test: &Test) {
        let stats = self.state.stats_as_bytes();
        match Client::validate_getstats(&test.query[..], &response[..], stats) {
            Ok(()) => self.results.inc_passed(),
            Err(e) => {
                eprintln!("{}", e);
                self.results.inc_failed();
            }
        }
    }

    fn handle_reset_stats(&mut self, response: BytesMut, test: &Test) {
        self.state.reset();
        self.handle_other_requests(response, test)
    }

    fn handle_other_requests(&mut self, response: BytesMut, test: &Test) {
        match Client::validate_messages(&response[..], &test.expected[..]) {
            Ok(()) => self.results.inc_passed(),
            Err(e) => {
                eprintln!("{}", e);
                self.results.inc_failed();
            }
        }
    }

    fn validate_getstats(query: &[u8], response: &[u8], stats: &[u8]) -> Result<()> {
        let query = Message::parse(&query[..]).unwrap();
        let response = Message::parse(&response[..]).unwrap();
        // println!("{:?}", response);
        if Request::from_u16(query.header.code()).unwrap() != Request::GetStats {
            return Err(Error::new(
                ErrorKind::Other,
                "Client Error: Request is not GetStats",
            ));
        }
        if response.payload != stats {
            let msg: String = format!(
                "Error: Validating GetStats Request:\nreceived {:?}\nexpected {:?}\n",
                response.payload, stats
            );
            return Err(Error::new(ErrorKind::Other, msg));
        }
        Ok(())
    }

    fn validate_messages(pack: &[u8], test: &[u8]) -> Result<()> {
        let pack_message = Message::parse(&pack[..]).unwrap();
        let test_message = Message::parse(&test[..]).unwrap();
        if pack_message.header.as_bytes() != test_message.header.as_bytes() {
            let msg: String = format!(
                "Error: Headers not equal\nreceived: {:?}\nexpected: {:?}\n",
                pack_message.header.as_bytes(),
                test_message.header.as_bytes()
            );
            return Err(Error::new(ErrorKind::Other, msg));
        }
        if pack[..] != test[..] {
            let msg: String = format!(
                "Error: Payloads not equal\nreceived: {:?}\nexpected: {:?}",
                pack, test
            );
            return Err(Error::new(ErrorKind::Other, msg));
        }
        Ok(())
    }
}

impl Test {
    // arbitrarily large to allow testing total message size larger than MAX_MESSAGE
    const FULL_BUFF: usize = (message::MAX_MESSAGE_PADDED * 2) + 12;

    pub fn message_bytes(sign: u32, size: u16, code: u16, msg: &[u8]) -> Result<Vec<u8>> {
        let mut buf = [0u8; Test::FULL_BUFF];
        match msg.len() {
            n if n > Test::FULL_BUFF => Err(Error::new(ErrorKind::Other, "payload is too large")),
            n => {
                Message::parse_mut(&mut buf[..])
                    .unwrap()
                    .set_all(sign, size, code, msg);
                Ok(buf[..message::total_response_len(n)].to_vec())
            }
        }
    }

    pub fn message_default(code: u16, bytes: &[u8]) -> Vec<u8> {
        Test::message_bytes(message::MAGIC, bytes.len() as u16, code, bytes).unwrap()
    }

    pub fn header_bytes(sign: u32, size: u16, code: u16) -> Vec<u8> {
        Header::new_with(sign, size, code).as_bytes().to_vec()
    }

    pub fn header_default(code: u16) -> Vec<u8> {
        Test::header_bytes(message::MAGIC, 0, code)
    }

    pub fn response_fail(response: Response) -> Vec<u8> {
        Test::header_default(response as u16)
    }

    pub fn request_ping() -> Vec<u8> {
        Test::header_default(Request::Ping as u16)
    }

    pub fn response_ping() -> Vec<u8> {
        Test::header_default(Response::Ok as u16)
    }

    pub fn request_reset_stats() -> Vec<u8> {
        Test::header_default(Request::ResetStats as u16)
    }

    pub fn response_reset_stats() -> Vec<u8> {
        Test::header_default(Response::Ok as u16)
    }

    pub fn request_get_stats() -> Vec<u8> {
        Test::header_default(Request::GetStats as u16)
    }

    #[allow(unused)]
    pub fn response_get_stats(stats: &[u8]) -> Vec<u8> {
        Test::message_default(Response::Ok as u16, stats)
    }

    pub fn request_compress(payload: &[u8]) -> Vec<u8> {
        Test::message_default(Request::Compress as u16, payload)
    }

    pub fn response_compress(bytes: &[u8]) -> Vec<u8> {
        Test::message_default(Response::Ok as u16, bytes)
    }
}

// a => a
// aa => aa
// aaa => 3a
// aaaaabbb => 5a3b
// aaaaabbbbbbaaabb => 5a6b3abb
// abcdefg => abcdefg
// aaaccddddhhhhi => 3acc4d4hi
pub fn test_compress_ok(request: &[u8], response: &[u8]) -> Test {
    Test {
        query_kind: Request::Compress,
        query: Test::request_compress(request),
        expected: Test::response_compress(response),
        validity: TestKind::Valid,
    }
}

/// TODO:
/// add response argument if other unique character types error messages added
/// i.e. Resopnse::MessageContainsNumbers,
/// or, Resopnse::MessageContainsUppercaseCharacters
pub fn test_compress_fail(request: &[u8], response: Response) -> Test {
    Test {
        query_kind: Request::Compress,
        query: Test::request_compress(request),
        expected: Test::response_fail(response),
        validity: TestKind::Invalid,
    }
}

pub fn test_compress_fail_default(request: &[u8]) -> Test {
    Test {
        query_kind: Request::Compress,
        query: Test::request_compress(request),
        expected: Test::response_fail(Response::MessagePayloadContainsInvalidCharacters),
        validity: TestKind::Invalid,
    }
}

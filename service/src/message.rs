use byteorder::NetworkEndian;
use std::{cmp, error::Error, fmt, mem};
use zerocopy::{
    byteorder::{U16, U32},
    AsBytes, ByteSlice, ByteSliceMut, FromBytes, LayoutVerified,
};

pub const MAGIC: u32 = 0x5354_5259_u32;
pub const HEADER_SIZE: usize = mem::size_of::<Header>();
pub const MAX_PAYLOAD: u16 = 1 << 13;
pub const MAX_MESSAGE: usize = HEADER_SIZE + MAX_PAYLOAD as usize;
pub const MAX_MESSAGE_PADDED: usize = MAX_MESSAGE + 8;

/// The request code found within the header of received messages from the client
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Request {
    Ping = 1,
    GetStats = 2,
    ResetStats = 3,
    Compress = 4,
}

impl Request {
    pub fn from_u16(value: u16) -> Option<Request> {
        match value {
            1 => Some(Request::Ping),
            2 => Some(Request::GetStats),
            3 => Some(Request::ResetStats),
            4 => Some(Request::Compress),
            _ => None,
        }
    }
}

/// The response code found within the header of sent messages from the server
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Response {
    Ok = 0,
    UnknownError = 1,
    /// The received message is larger than MAX_MESSAGE
    MessageTooLarge = 2,
    UnsupportedRequestType = 3,
    // Implementer Defined
    /// The received message is less than the size of a header
    MessageTooSmall = 34,
    /// The magic signature of the header is not equal to 0x53545259
    MessageHeaderHasBadMagic = 35,
    /// The header size field does not match the length of the payload
    MessageHeaderSizeMismatch = 36,
    /// The associated request requires the size field of the header to be zero
    RequestKindRequiresZeroLength = 37,
    /// Compression request requires a header with a non-zero length field
    CompressionRequestRequiresNonZeroLength = 38,
    /// Compression request payload includes non lowercase ascii characters
    MessagePayloadContainsInvalidCharacters = 39,
}

/// A Message's header field
/// A zerocopy-able representation of incoming and outgoing packet headers
/// sign: The magic signature
/// size: The size of the payload
/// code: Request or Response code
#[derive(Debug, Eq, PartialEq, FromBytes, AsBytes)]
#[repr(C)]
pub struct Header {
    sign: U32<NetworkEndian>,
    size: U16<NetworkEndian>,
    code: U16<NetworkEndian>,
}

impl Header {
    // used only for Client Tests
    pub fn new_with(sign: u32, size: u16, code: u16) -> Header {
        Header {
            sign: U32::new(sign),
            size: U16::new(size),
            code: U16::new(code),
        }
    }

    pub fn sign(&self) -> u32 {
        self.sign.get()
    }

    pub fn size(&self) -> u16 {
        self.size.get()
    }

    pub fn code(&self) -> u16 {
        self.code.get()
    }

    pub fn set_sign(&mut self, sign: u32) {
        self.sign.set(sign);
    }

    pub fn set_size(&mut self, size: u16) {
        self.size.set(size);
    }

    pub fn set_code(&mut self, code: u16) {
        self.code.set(code);
    }

    /// Validates the header of a client's request message
    /// returns a `Response` relative to the `Request`
    pub fn validate_header(&self) -> Response {
        let request = Request::from_u16(self.code.get());
        if self.sign.get() != MAGIC {
            return Response::MessageHeaderHasBadMagic;
        }
        if request.is_none() {
            return Response::UnsupportedRequestType;
        }
        match (request.unwrap(), self.size.get()) {
            (Request::Compress, n) => match n {
                0 => Response::CompressionRequestRequiresNonZeroLength,
                n if n > MAX_PAYLOAD => Response::MessageTooLarge,
                _ => Response::Ok,
            },
            (_, 0) => Response::Ok,
            (_, _) => Response::RequestKindRequiresZeroLength,
        }
    }
}

//

/// The representation of messages sent/received within the service
///
/// Constructs a reference to a underlying slice
/// More precisely, Serialize a `ByteSlice` (&[u8]) or `ByteSliceMut` (&mut [u8])
/// into a zerocopy representation of an incoming or outgoing message
///
/// Wraps buffer as a `Message` this allows zerocopy accessor / mutators
/// into the underlying `Header` and payload parts of the Message
pub struct Message<B: ByteSlice> {
    pub header: LayoutVerified<B, Header>,
    pub payload: B,
}

impl<B: ByteSlice> Message<B> {
    /// Creates a statically checked reference to a ByteSlice as a Message
    ///
    /// # Example
    /// ```
    /// use service::Message;
    /// let magic: u32 = 0x5354_5259_u32;
    /// let buf = [83u8, 84, 82, 89, 0, 3, 0, 4, 97, 97, 97];
    /// let message = Message::parse(&buf[..]).unwrap();
    /// assert_eq!(message.header.sign(), magic);
    /// ```
    pub fn parse(bytes: B) -> Option<Message<B>> {
        let (header, payload) = LayoutVerified::new_from_prefix(bytes)?;
        Some(Message { header, payload })
    }
}

impl<B: ByteSliceMut> Message<B> {
    /// Creates a statically checked reference to a ByteSliceMut as a Message
    ///
    /// # Example
    /// ```
    /// use service::Message;
    /// let magic: u32 = 0x5354_5259_u32;
    /// let mut buf = [83u8, 84, 82, 89, 0, 3, 0, 4, 97, 97, 97];
    /// let message = Message::parse_mut(&mut buf[..]).unwrap();
    /// assert_eq!(message.header.sign(), magic);
    /// ```
    pub fn parse_mut(bytes: B) -> Option<Message<B>> {
        let (header, payload) = LayoutVerified::new_from_prefix(bytes)?;
        Some(Message { header, payload })
    }

    pub fn set_sign(&mut self, new_sign: u32) {
        self.header.set_sign(new_sign)
    }

    pub fn set_size(&mut self, new_size: u16) {
        self.header.set_size(new_size)
    }

    pub fn set_code(&mut self, new_code: u16) {
        self.header.set_code(new_code)
    }

    pub fn set_header(&mut self, sign: u32, size: u16, code: u16) {
        self.set_sign(sign);
        self.set_size(size);
        self.set_code(code);
    }

    pub fn set_header_with_default_magic(&mut self, size: u16, code: u16) {
        self.set_header(MAGIC, size, code);
    }

    /// Sets the body of the payload from a given byte-slice
    /// returns error if the length of the input slice is larger than the message's payload length
    pub fn set_payload(&mut self, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        if bytes.len() > self.payload.len() as usize {
            return Err("length of input exceeds payload size".into());
        }
        self.payload[..bytes.len()].clone_from_slice(bytes);
        Ok(())
    }

    pub fn set_all(&mut self, sign: u32, size: u16, code: u16, bytes: &[u8]) {
        self.set_header(sign, size, code);
        let _ = self.set_payload(bytes);
    }
}

impl<B> Message<B>
where
    B: ByteSlice,
{
    pub fn validate(&self, bytes_read: usize) -> Response {
        if bytes_read < HEADER_SIZE {
            return Response::MessageTooSmall;
        }
        if bytes_read > MAX_MESSAGE as usize {
            return Response::MessageTooLarge;
        }
        if self.header.size() != payload_len(bytes_read) as u16 {
            return Response::MessageHeaderSizeMismatch;
        }

        let response = self.header.validate_header();
        let request = Request::from_u16(self.header.code());
        match (response, request) {
            (Response::Ok, Some(Request::Compress)) => self.validate_payload(bytes_read),
            (response_code, _) => response_code,
        }
    }

    pub fn validate_payload(&self, bytes_read: usize) -> Response {
        if self.is_payload_valid(bytes_read) {
            Response::Ok
        } else {
            Response::MessagePayloadContainsInvalidCharacters
        }
    }

    // could instead filter out valid characters (ascii_lowercase)
    // then, check the unique character types found and relay a more precise message
    // i.e.
    // Resopnse::MessageContainsNumbers,
    // Resopnse::MessageContainsUppercaseCharacters
    /// Validates the payload part of a message
    /// Currently, a payload is only valid if it exclusively contains lowercase ascii characters
    pub fn is_payload_valid(&self, _bytes_read: usize) -> bool {
        // There is a trade-off between validating before vs while compressing
        self.payload[..self.header.size() as usize]
            .iter()
            .all(|x: &u8| (*x as char).is_ascii_lowercase())
    }
}

impl<B: ByteSlice> fmt::Display for Message<B> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let max_size = cmp::min(self.header.size(), MAX_PAYLOAD) as usize;
        fmt.debug_struct("Message")
            .field("header", &*self.header)
            .field("payload", &&self.payload[..max_size])
            .finish()
    }
}

impl<B: ByteSlice> fmt::Debug for Message<B> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self)
    }
}

/// Given the number of bytes read, computes the payload length of the message
pub fn payload_len(bytes_read: usize) -> usize {
    bytes_read - HEADER_SIZE
}

/// Given the payload length, computes the total message size
pub fn total_response_len(payload_len: usize) -> usize {
    HEADER_SIZE + payload_len
}

/// Determine if a slice can be parsed/serialized into a `Message`
pub fn can_parse(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER_SIZE
}

#[cfg(test)]
mod tests {
    #[allow(unused)]
    use super::{Message, Request, Response, HEADER_SIZE, MAX_MESSAGE, MAX_PAYLOAD};
    const MAGIC: u32 = 0x5354_5259_u32;

    #[test]
    fn test_payload() {
        let mut buf = [83u8, 84, 82, 89, 0, 3, 0, 4, 97, 97, 97];
        let payload = Message::parse_mut(&mut buf[..]).unwrap();
        assert_eq!(payload.header.sign(), MAGIC);
        assert_eq!(payload.header.size(), 3);
        assert_eq!(payload.header.code(), 4);
        assert_eq!(
            Request::from_u16(payload.header.code()),
            Some(Request::Compress)
        );
    }

    #[test]
    fn test_message_too_large() {
        let mut rx = [0u8; MAX_MESSAGE + 8];
        let bytes_read = MAX_MESSAGE + 8;
        let mut message = Message::parse_mut(&mut rx[..]).unwrap();
        message.set_size(bytes_read as u16);
        message.set_code(4);
        assert_eq!(message.validate(bytes_read), Response::MessageTooLarge);
    }

    #[test]
    fn test_message_too_small() {
        let mut rx = [83u8, 84, 82, 89, 0, 0, 0, 0];
        let bytes_read = 7;
        assert_eq!(
            Message::parse_mut(&mut rx[..])
                .unwrap()
                .validate(bytes_read),
            Response::MessageTooSmall
        );
    }

    #[test]
    fn test_message_header_size_mismatch() {
        let mut rx = [83u8, 84, 82, 89, 0, 0, 0, 0, 97];
        let bytes_read = rx.len();
        assert!(Message::parse_mut(&mut rx[..])
            .unwrap()
            .validate(bytes_read)
            .eq(&Response::MessageHeaderSizeMismatch));

        let mut rx = [83u8, 84, 82, 89, 0, 3, 0, 4];
        let bytes_read = rx.len();
        assert!(Message::parse_mut(&mut rx[..])
            .unwrap()
            .validate(bytes_read)
            .eq(&Response::MessageHeaderSizeMismatch));

        // header.size = 1, payload.len = 2
        let mut rx = [83u8, 84, 82, 89, 0, 1, 0, 4, 97, 97];
        let bytes_read = rx.len();
        assert!(Message::parse_mut(&mut rx[..])
            .unwrap()
            .validate(bytes_read)
            .eq(&Response::MessageHeaderSizeMismatch));
    }

    #[test]
    fn test_message_request_requires_zero_length() {
        let mut rx = [83u8, 84, 82, 89, 0, 1, 0, 0, 97];
        let bytes_read = rx.len();
        let mut message = Message::parse_mut(&mut rx[..]).unwrap();
        {
            message.set_code(Request::Ping as u16);
            assert_eq!(
                message.validate(bytes_read),
                Response::RequestKindRequiresZeroLength
            );
        }
        {
            message.set_code(Request::GetStats as u16);
            assert_eq!(
                message.validate(bytes_read),
                Response::RequestKindRequiresZeroLength
            );
        }
        {
            message.set_code(Request::Compress as u16);
            assert_eq!(message.validate(bytes_read), Response::Ok);
        }
    }

    #[test]
    fn test_compression_request_requires_non_zero() {
        let mut rx = [83u8, 84, 82, 89, 0, 0, 0, 4];
        let bytes_read = rx.len();
        // let response =
        assert!(Message::parse_mut(&mut rx[..])
            .unwrap()
            .validate(bytes_read)
            .eq(&Response::CompressionRequestRequiresNonZeroLength));
    }
}

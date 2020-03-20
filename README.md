# Compression-Service

Rust + Aync + TCP sockets = Fun!

## Target Platform
+ OS: Fedora 30
+ Language:
  + rust 2018
  + cargo 1.42.0

## Usage

`cargo run --bin compression_service`

#### Note
+ unit tests provided
  + run via
	+ `cargo test`
    + or, `sh test.sh unit`
+ a "test" client is available through the provided test-client crate.
  + run in a separate terminals
	+ `sh run.sh`
	+ `sh test.sh client`



## Description

The compression-service's TCP server (found within the `service` crate) reads in
requests and constructs responses in an asynchronous fashion. Requests are
serialized into a Message (with the use of the zerocopy crate), the message's
header and payload are validated then, the appropriate response is constructed
and sent to the client.

+ Important Values
  + MAX_PAYLAOD
	+ value : 8192
  + MAX_MESSAGE
	+ value : `MAX_PAYLOAD` + `HEADER_SIZE` (= 8200)

+ crate documentation also available after running `docs.sh`

### Third-Party Libraries
+ service:
  + tokio
	+ version  : 0.2
	+ features : full
	+ Chosen for async io, concurrency and a simple interface for creating a tcp server
  + zerocopy
	+ version 0.3.0
	+ Chosen for simple and efficient zerocopy (ser|de)ializing byte-slices
	+ For statically checked trait derevations AsBytes and FromBytes
	+ Intuitive conversion of network-endian with zerocopy::{U32, U16}
  + byteorder
	+ version 1.3.4
	+ For endian utilities
+ test-client:
  + tokio-util
	+ version  : 0.3.1
	+ features : codec
	+ For Framed codecs (BytesCodec)
	+ a simplified flow for handling reading/writing from/to socket
  + futures = "0.3.0"
	+ For futures support on streams
  + bytes = "0.5"
	+ For reading/writing the frames into

### Assumptions
+ Following "In all cases the status field of the header should be filled in
  appropriately",
+ The server will receive requests from clients and reply to them in a loop
+ There can be multiple simultaneous client connections/requests


## The System
This service will consume data in ASCII format over a TCP socket and return a
compressed version of that data.

### Messaging Format
All messages that flow over the socket share a common header that consists of
three fixed width integer fields in the following order:
+ A 32 bit wide magic value which is always equal to MAGIC.
+ A 16 bit payload length
+ A 16 bit request code / status code
Note: MAGIC is the signature and can be changed

The header may or may not be followed by a payload depending on the message
type. Lastly, all fields are in ***network byte order***.

### Requests
The compression service supports the following request types (request
code noted in parenthesis):
• “Ping” (RC: 1)
+ Serves as a means to check that the service is operating normally.
+ + “Get Stats” (RC: 2)
+ Retrieves various internal statistics of the service. Note that these
statistics do not have to be preserved across service instances.
+ “Reset Stats” (RC: 3)
+ + Resets the internal statistics to their appropriate default values.
+ “Compress” (RC: 4)
+ + Requests that some data be compressed using a particular compression
scheme.
All other request codes should be considered invalid.

### Request Formats
Ping / Get Stats / Reset Stats Requests
All three of these requests consist of only a header with the payload length set
to zero and the request code set appropriately (e.g. 3 in the case of a “Reset
Stats” request).

### Compress Request
The “Compress” request consists of a header followed by the ASCII payload to be
compressed. Note that your server should have an . Any request that is larger
than the MAXPAYLOADSIZE (of at least 4KiB but less than 32KiB) should result in
an appropriate error.

### Compression Algorithm
The compression algorithm is a simplified prefix encoding compression scheme.
(all consecutively repeated characters in the given string are replaced by a prefix denoting the number of characters replaced followed by the character itself. Some examples:
+ a => a
+ aa => aa
+ aaa => 3a
+ aaaaabbb => 5a3b
+ aaaaabbbbbbaaabb => 5a6b3abb
+ abcdefg => abcdefg
+ aaaccddddhhhhi => 3acc4d4hi
+ 123 => <invalid: contains numbers>
+ abCD => <invalid: contains uppercase characters>

### Responses
Each of the above requests has a matching response as defined below. In all
cases the status field of the header should be filled in appropriately from the
following table:
Status Code Meaning
+ 0 - Ok
+ 1 - Unkown Error
+ 2 - Message Too Large
+ 3 - Unsupported Request Type
+ 4-32 - Reserved Status Code Range
+ 33-128 - Implementer Defined
  + 34 - MessageTooSmall,
	+ The received message is less than the size of a header
  + 35 - MessageHeaderHasBadMagic
	+ The magic signature of the header is not equal to MAGIC
  + 36 - MessageHeaderSizeMismatch = 36,
	+ The header size field does not match the length of the payload
  + 37 - RequestKindRequiresZeroLength = 37,
	+ The associated request requires the size field of the header to be zero
  + 38 - CompressionRequestRequiresNonZeroLength = 38,
	+ Compression request requires a header with a non-zero length field
  + 39 - MessagePayloadContainsInvalidCharacters = 39,
	+ Compression request payload includes non lowercase ascii characters


### Ping Response
Consists of just a header with the payload length set to zero and
the status code set to OK (0) if the service is operating normally or one of the
2error status codes if some error condition has occurred that is not request
specific (e.g. service initialization failed, low memory condition, etc).

### Get Stats Response
The “Get Stats” response consists of a header and the payload consists of two 32
bit unsigned integers followed by one unsigned byte:
+ **Total Bytes Received**: A count of all bytes received by the service, including
headers
+ **Total Bytes Sent**: A count of all bytes sent by the service, including
headers
+ **Compression Ratio**: A number from 0 - 100 representing the performance
of the service. (i.e. if the service was asked to compress 298398921 bytes of
data and was able to compress those bytes down to 129372810 bytes, the
service’s compression ratio would be 43).
Note: the size field of the header is always equal to `(sizeof(u32) * 2) + sizeof(u8))`


### Reset Stats Response
Consists of just a header with a payload length of zero and an appropriate status code.

### Compress Response
Consists of a header with payload size set appropriately
followed by compressed ASCII data. If an error occurs, the response is just a
header with payload size set to zero and an appropriately set status code.


  + Therefore, the response to a GetStats request will also be prefixed with a
    header
	+ the header will have a size field equal to 7
	+ the message payload will be 7 bytes long (relative to the size of each
      field of `Stats`)
	+ the status field will be set appropriately


## Improvements
+ Generalizing this style of client-server communication with custom traits
  + Namely, generic traits specifying the relationship between,
	+ a Message, Client, Server, and the rules of engagement through and between
      each
+ Limiting the number of clients based on resources available
+ Managed circular-buffer for efficiently reading Messages
+ Handling tokio runtime shutdown
+ More analysis of the trade-offs of some other design decisions,
	+ Utilizing bytes::{Bytes, BytesMut} & tokio-util::codec::BytesCodec (as seen in `client`)
	  + instead of stack allocated arrays (as seen in `service`)
	+ Reading the first 8 bytes to get the header of the Message
		+ Then, reading based on the size field of the header
			- expensive IO for handling many concurrent connections?
+ Better mechanism to overcome a client flooding the server.
  + Max size of the read buffer is greater than MAX_MESSAGE to identify messages
    that overflow
  + Currently dealt with by dropping the client when the following read is also
    greater than MAX_PACKET
	+ if the first read is exhausted (i.e. bytes_read > MAX_PAYLOAD), read again
      to drain the remaining bytes and if the number of bytes read is agian
      exausted (i.e. bytes_read >= MAX_PAYLOAD) drop the client

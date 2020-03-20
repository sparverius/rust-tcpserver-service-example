use crate::message;
pub use compress::compress_message;
pub use connection::Connection;
pub use state::State;
pub use stats::Stats;

mod compress;
mod connection;
mod state;
pub mod stats;

use std::{
    io::{Error, ErrorKind},
    sync::Arc,
};
use tokio::{
    net::{TcpListener, TcpStream},
    prelude::*,
    sync::Mutex,
};

type Result<T> = std::result::Result<T, std::io::Error>;

// `State`, `Message`, `Connection` could be generalized

/// The compression Server
pub struct Server {
    pub listener: TcpListener,
    the_state: Arc<Mutex<State>>,
}

impl Server {
    /// # Examples
    ///
    /// ```ignore
    /// use Server::{Result, Server};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), std::io::Error> {
    ///    Server::new_with_url("127.0.0.1:4000").await?.serve().await
    /// }
    /// ```
    pub async fn new_with_url(url: &str) -> Result<Server> {
        let listener = TcpListener::bind(url).await?;
        let the_state = Arc::new(Mutex::new(State::new()));
        Ok(Server {
            listener,
            the_state,
        })
    }

    /// Asynchronous accept loop for a TcpListener listening at a given url
    /// Multiple threads are spawned for processing connections in parallel
    pub async fn serve(&mut self) -> Result<()> {
        println!(
            "Starting Compression Service @ {}",
            self.listener.local_addr().unwrap()
        );
        loop {
            match self.listener.accept().await {
                Ok((stream, _)) => {
                    let peer_addr = stream.peer_addr()?;
                    let state = Arc::clone(&self.the_state);
                    tokio::spawn(async move {
                        // println!("Client @ {:?}", peer_addr);

                        if let Err(e) = Server::process(stream, state).await {
                            eprintln!("{}", e)
                        }

                        println!("Client @ {:?} Complete", peer_addr);
                    });
                }
                Err(e) => eprintln!("{:?}", e),
            }
        }
    }

    /// Process communication from a given client connection, consumes client
    /// messages, process the request, and sends appropriate response to client
    /// TODO:
    /// Potentially replace the tx and rx buffers with a managed circular buffer
    /// OR use tokio_util::codec::BytesCodec (as seen in test-client package).
    /// However with that approach, there is a tradeof between excessive copying
    /// with the use of bytes::Bytes and a Framed codec
    /// and wasted stack space
    ///
    /// TODO:
    /// Find alternative to dropping the client for flooding the server with
    /// excessively large messages perhaps, rate limiting or a warning response?
    pub async fn process(mut stream: TcpStream, state: Arc<Mutex<State>>) -> Result<()> {
        let mut rx = [0u8; message::MAX_MESSAGE_PADDED];
        let mut tx = [0u8; message::MAX_MESSAGE_PADDED];
        loop {
            let mut state = state.lock().await;
            let bytes_read = stream.read(&mut rx).await?;
            if bytes_read == 0 {
                return Ok(()); // connection closed
            }

            // MessageTooLarge so, drop the rest so that we can create error response
            // and free up the stream to read in subsequent messages
            if bytes_read > message::MAX_MESSAGE {
                let mut bytes = [0u8; message::MAX_MESSAGE_PADDED];
                let num_bytes = stream.read(&mut bytes).await?;
                state.update_read(num_bytes);
                if num_bytes >= message::MAX_MESSAGE {
                    return Err(Error::new(ErrorKind::Other, "Dropping client"));
                }
            }
            state.update_read(bytes_read);

            // the request buffer (rx) must be atleast the size of the header
            // otherwise parsing the buffer into a Message will return None
            let sz = std::cmp::max(message::HEADER_SIZE, bytes_read);

            let size = Connection::new_with(&rx[..sz], &mut tx[..], bytes_read)
                .create_response(&mut state);

            stream.write_all(&tx[..size]).await?;
            state.update_sent(size);

            // Not strictly needed however, zero out buffers for data integrity
            // Server::unset(&mut rx[..bytes_read]);
            // Server::unset(&mut tx[..size]);
        }
    }

    #[allow(dead_code)]
    fn unset(buf: &mut [u8]) {
        buf.iter_mut().for_each(|x: &mut u8| *x = 0);
    }
}

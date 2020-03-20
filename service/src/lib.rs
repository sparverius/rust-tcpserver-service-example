//! service
//!
//! Provides the building blocks for a simple compression service.
//!
//! This service will consume data in ASCII format over a TCP socket
//! and return a compressed version of that data
//!
//! The service is also able to respond to several other types of `Request`s
//!
//! The unit of communcation is done through a `Message`
pub mod message;
pub use message::*;
pub mod server;
pub use server::*;

//! The adapter side of EtherNet/IP, built on `eipscanne_rs`: an explicit messaging server.
//! A device implements [`handler::Handler`]; [`server::serve`] does everything else.
//!
//! ```text
//! server   Accepts connections, registers sessions, hands Get and Set requests to the handler
//! handler  The trait a device implements, and `decode` for the data of a Set
//! reply    The Message Router replies: a success with data, or a refusal
//! packet   Reads and writes one encapsulation packet on a stream, finds the request it carries
//! error    The one error type of every fallible call
//! ```

pub mod error;
pub mod handler;
pub mod packet;
pub mod reply;
pub mod server;

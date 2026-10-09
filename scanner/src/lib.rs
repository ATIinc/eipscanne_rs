//! The scanner side of EtherNet/IP, built on `eipscanne_rs`, written to be read top to bottom.
//! The library only (de)serializes packets; sockets, timers and state live here.
//!
//! ```text
//! session             The encapsulation session over TCP 44818, shared by both kinds of messaging
//! explicit            Unconnected messaging: one request, one reply
//!   connected         A class 3 connection: requests over Send Unit Data
//! connection_manager  Forward_Open and Forward_Close, shared by both kinds of connection
//! implicit            A class 1 I/O connection: cyclic I/O over UDP 2222
//! error               The one error type of every fallible call
//! ```

pub mod connection_manager;
pub mod error;
pub mod explicit;
pub mod implicit;
pub mod session;

//! The scanner side of EtherNet/IP, built on `eipscanne_rs`, written to be read top to bottom.
//! The library only (de)serializes packets; sockets, timers and state live here.
//!
//! ```text
//! session     The encapsulation session over TCP 44818, shared by both kinds of messaging
//! explicit    Unconnected messaging: one request, one reply
//! implicit    A class 1 I/O connection: Forward_Open, cyclic I/O over UDP 2222, Forward_Close
//! error       The one error type of every fallible call
//! ```

pub mod error;
pub mod explicit;
pub mod implicit;
pub mod session;

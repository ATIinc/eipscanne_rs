//! The scanner side of EtherNet/IP, built on `eipscanne_rs`: how to talk to an adapter, in code
//! a person can read top to bottom. The library stays packet (de)serialization only; everything
//! with a socket, a timer or state lives here, so production code can use it as a reference and
//! reuse the parts it needs.
//!
//! ```text
//! session     The encapsulation session over TCP 44818 (RegisterSession ... UnregisterSession),
//!             shared by both kinds of messaging
//! explicit    Explicit (unconnected) messaging: one request, one reply, e.g. reading the
//!             Identity object or an assembly
//! implicit    Implicit messaging: a class 1 I/O connection (Forward_Open, cyclic I/O over
//!             UDP 2222, Forward_Close): the connection, then one submodule per direction
//! error       The one error type of every fallible call
//! ```
//!
//! The `read-identity` and `write-clearlink-io` examples use `session` and `explicit`; the
//! `implicit-io` example uses `session` and `implicit`.

pub mod error;
pub mod explicit;
pub mod implicit;
pub mod session;

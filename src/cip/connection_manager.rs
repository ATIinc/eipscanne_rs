//! Connection Manager object services: opening and closing connections.
//!
//! Terminology used throughout this module, as the protocol and Wireshark use it:
//!
//! * **originator**: the device that opens the connection; in this library that is always the
//!   scanner.
//! * **target**: the device the connection is opened to; the adapter (the I/O device).
//!
//! * **O->T** ("originator to target"): data and parameters for the direction scanner -> adapter,
//!   i.e. the outputs we send. Field names use the `o2t_` prefix.
//! * **T->O** ("target to originator"): the direction adapter -> scanner, i.e. the inputs we
//!   receive. Field names use the `t2o_` prefix.

pub mod forward_close;
pub mod forward_open;
pub mod parameters;
pub mod response;
pub mod shared;

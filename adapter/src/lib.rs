//! The adapter side: a device implements [`message_router::MessageRouter`], [`server::serve`] does
//! the rest.
//!
//! A reply is an encapsulation packet; a response is a CIP Message Router response.

pub mod error;
mod explicit;
pub mod message_router;
pub mod packet;
pub mod server;

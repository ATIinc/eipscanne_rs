//! The adapter side: a device implements [`message_router::MessageRouter`] (and
//! [`implicit::ImplicitDevice`] for I/O, wrapped in an
//! [`implicit::message_router::ImplicitMessageRouter`]), [`server::serve`] does the rest.
//!
//! A reply is an encapsulation packet; a response is a CIP Message Router response.

pub mod error;
pub mod explicit;
pub mod implicit;
pub mod message_router;
pub mod packet;
pub mod server;

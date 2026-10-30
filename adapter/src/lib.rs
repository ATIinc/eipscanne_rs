//! The adapter side: a device implements [`message_router::MessageRouter`], [`server::serve`]
//! does the rest. An [`implicit::message_router::ImplicitMessageRouter`] adds class 1
//! connections to an [`implicit::ImplicitDevice`], a
//! [`explicit::connected::message_router::ConnectedMessageRouter`] class 3 ones.
//!
//! A reply is an encapsulation packet; a response is a CIP Message Router response.

mod connection_helpers;
pub mod error;
pub mod explicit;
pub mod implicit;
pub mod message_router;
pub mod packet;
pub mod server;

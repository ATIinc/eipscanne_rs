//! The adapter side: a device implements [`message_router::MessageRouter`] (and
//! [`implicit::ImplicitDevice`] for I/O), [`server::serve`] (or [`server::serve_implicit`]) does
//! the rest.

pub mod error;
pub mod explicit;
pub mod implicit;
pub mod message_router;
pub mod packet;
pub mod server;

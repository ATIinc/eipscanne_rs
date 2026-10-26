//! An explicit messaging server: a device implements [`handler::Handler`], [`server::serve`] does the rest.

pub mod error;
pub mod handler;
pub mod packet;
pub mod reply;
pub mod server;

//! An explicit messaging server: a device implements [`device::Device`], [`server::serve`] does the rest.

pub mod device;
pub mod error;
pub mod explicit;
pub mod packet;
pub mod server;

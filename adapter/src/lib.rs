//! The adapter side: a device implements [`device::Device`], [`server::serve`] does the rest.

pub mod device;
pub mod error;
pub mod explicit;
pub mod implicit;
pub mod packet;
pub mod server;

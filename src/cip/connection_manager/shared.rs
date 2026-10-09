//! Blocks that the Connection Manager services carry in the same layout.

use binrw::binrw;

use crate::cip::types::{CipUdint, CipUint};

/// The three values that identify a connection. Every Connection Manager service carries them,
/// and a Forward_Close is matched against the ones of the Forward_Open that opened the connection.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct ConnectionTriad {
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,
}

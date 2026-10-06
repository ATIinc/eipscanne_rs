//! Blocks that the Connection Manager services carry in the same layout.

use binrw::binrw;

use crate::cip::types::{CipUdint, CipUint, CipUsint};

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

/// Reply data of any Connection Manager service when the general status is not success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct UnsuccessfulResponse {
    /// The triad of the refused request when the Connection Manager itself refused it. A request
    /// the Message Router refuses before it reaches the Connection Manager (a path segment error,
    /// for one) comes back with no reply data at all.
    #[br(try)]
    pub connection_triad: Option<ConnectionTriad>,

    /// Remaining Path Size in 16-bit words and its reserved byte: how much of the connection path
    /// was left when a router rejected the request. Only present for routing errors, so both are
    /// read when the bytes are there and left out otherwise.
    #[br(try)]
    pub remaining_path_size: Option<CipUsint>,
    #[br(try)]
    pub reserved: Option<CipUsint>,
}

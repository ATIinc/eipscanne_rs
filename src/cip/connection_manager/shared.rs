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

/// Tail of every successful reply: data the target application adds to the reply
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ApplicationReply {
    /// Application Reply Size in 16-bit words, followed by a reserved byte
    #[brw(pad_after = 1)]
    pub application_reply_size: CipUsint,

    #[br(count = usize::from(application_reply_size) * 2)]
    pub application_reply: Vec<CipUsint>,
}

/// Remaining Path Size in 16-bit words and its reserved byte: how much of the connection path
/// was left when a routing node rejected the request
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct RemainingPath {
    pub remaining_path_size: CipUsint,
    pub reserved: CipUsint,
}

/// Reply data of any Connection Manager service when the general status is not success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct UnsuccessfulResponse {
    pub connection_triad: ConnectionTriad,

    /// Only present when a routing node rejected the request
    #[br(try)]
    pub remaining_path: Option<RemainingPath>,
}

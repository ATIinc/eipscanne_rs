//! Forward_Close: closing a connection through the Connection Manager.

use binrw::binrw;

use crate::cip::connection_manager::parameters::PriorityTimeTick;
use crate::cip::connection_manager::shared::ConnectionTriad;
use crate::cip::path::CipPath;
use crate::cip::types::CipUsint;

/// Forward_Close request data (everything after the request path)
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardCloseRequest {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    /// The triad of the Forward_Open that opened the connection
    pub connection_triad: ConnectionTriad,

    // Connection Path Size in 16-bit words, derived from the path on write, then a reserved byte
    #[br(temp)]
    #[bw(calc = connection_path.word_len() as CipUsint)]
    #[brw(pad_after = 1)]
    connection_path_size: CipUsint,

    #[br(args(connection_path_size))]
    pub connection_path: CipPath,
}

/// Forward_Close reply data when the general status is success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardCloseResponse {
    pub connection_triad: ConnectionTriad,

    /// Application Reply Size in 16-bit words, followed by a reserved byte
    #[brw(pad_after = 1)]
    pub application_reply_size: CipUsint,

    /// Data the target application adds to the reply
    #[br(count = usize::from(application_reply_size) * 2)]
    pub application_reply: Vec<CipUsint>,
}

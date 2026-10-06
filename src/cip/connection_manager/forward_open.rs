//! Forward_Open and Large_Forward_Open: opening a connection through the Connection Manager.

use binrw::binrw;

use crate::cip::connection_manager::parameters::{
    ConnectionTimeoutMultiplier, NetworkConnectionParameters, PriorityTimeTick,
    TransportTypeTrigger,
};
use crate::cip::connection_manager::shared::ConnectionTriad;
use crate::cip::message::shared::ServiceCode;
use crate::cip::path::CipPath;
use crate::cip::types::{CipUdint, CipUsint};

/// Forward_Open / Large_Forward_Open request data (everything after the request path).
///
/// The width of the Network Connection Parameters decides the service: 16 bits for
/// Forward_Open, 32 bits for Large_Forward_Open, so both directions use the same case of
/// [`NetworkConnectionParameters`]. Reading needs to be told which one to expect.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(large: bool))]
pub struct ForwardOpenRequest {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    /// O->T (originator to target) Network Connection ID.
    /// 0 for a point-to-point connection: the target chooses it and returns it in the reply
    pub o2t_network_connection_id: CipUdint,
    /// T->O (target to originator) Network Connection ID, chosen by the originator
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,

    // Followed by three reserved bytes
    #[brw(pad_after = 3)]
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,

    /// O->T RPI (originator to target requested packet interval), in microseconds
    pub o2t_requested_packet_interval: CipUdint,
    /// O->T (originator to target) Network Connection Parameters
    #[br(args(large))]
    pub o2t_network_connection_parameters: NetworkConnectionParameters,

    /// T->O RPI (target to originator requested packet interval), in microseconds
    pub t2o_requested_packet_interval: CipUdint,
    /// T->O (target to originator) Network Connection Parameters
    #[br(args(large))]
    pub t2o_network_connection_parameters: NetworkConnectionParameters,

    pub transport_type_trigger: TransportTypeTrigger,

    // Connection Path Size in 16-bit words, derived from the path on write
    #[br(temp)]
    #[bw(calc = connection_path.word_len() as CipUsint)]
    connection_path_size: CipUsint,

    #[br(args(connection_path_size))]
    pub connection_path: CipPath,
}

// ======= Start of ForwardOpenRequest impl ========

impl ForwardOpenRequest {
    /// The service this request is sent with, decided by the width of its connection parameters.
    /// The originator to target word is looked at; the target to originator word must have the
    /// same width.
    pub fn service_code(&self) -> ServiceCode {
        match self.o2t_network_connection_parameters {
            NetworkConnectionParameters::Standard(_) => ServiceCode::ForwardOpen,
            NetworkConnectionParameters::Large(_) => ServiceCode::LargeForwardOpen,
        }
    }
}

// ^^^^^^^^ End of ForwardOpenRequest impl ^^^^^^^^

/// Forward_Open / Large_Forward_Open reply data when the general status is success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardOpenResponse {
    /// O->T (originator to target) Network Connection ID, chosen by the target
    pub o2t_network_connection_id: CipUdint,
    /// T->O (target to originator) Network Connection ID, echoed from the request
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,
    /// O->T API (originator to target actual packet interval), in microseconds
    pub o2t_actual_packet_interval: CipUdint,
    /// T->O API (target to originator actual packet interval), in microseconds
    pub t2o_actual_packet_interval: CipUdint,

    /// Application Reply Size in 16-bit words, followed by a reserved byte
    #[brw(pad_after = 1)]
    pub application_reply_size: CipUsint,

    /// Data the target application adds to the reply
    #[br(count = usize::from(application_reply_size) * 2)]
    pub application_reply: Vec<CipUsint>,
}

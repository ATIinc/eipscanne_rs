//! Forward_Open and Large_Forward_Open: opening a connection through the Connection Manager.

use binrw::binrw;

use crate::cip::connection_manager::parameters::{
    ConnectionDirection, ConnectionSizeError, ConnectionTimeoutMultiplier,
    NetworkConnectionParameters, PriorityTimeTick, TransportTypeTrigger,
};
use crate::cip::connection_manager::shared::{ApplicationReply, ConnectionTriad};
use crate::cip::message::shared::ServiceCode;
use crate::cip::path::CipPath;
use crate::cip::types::{CipUdint, CipUsint};

/// Forward_Open / Large_Forward_Open request data (everything after the request path).
///
/// The width of the Network Connection Parameters decides the service: 16 bits for
/// Forward_Open, 32 bits for Large_Forward_Open. Reading needs to be told which one to expect.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(large: bool))]
pub struct ForwardOpenRequest {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    pub o2t_network_connection_id: CipUdint,
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,

    // Followed by three reserved bytes
    #[brw(pad_after = 3)]
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,

    /// Requested packet interval, originator to target, in microseconds
    pub o2t_rpi: CipUdint,
    #[br(args(large))]
    pub o2t_network_connection_parameters: NetworkConnectionParameters,

    /// Requested packet interval, target to originator, in microseconds
    pub t2o_rpi: CipUdint,
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

/// Everything needed to build a Forward_Open request
#[derive(Debug, PartialEq, Clone, Default)]
pub struct ConnectionParameters {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    /// Left at 0 for a point-to-point connection: the target chooses it and returns it in the reply
    pub o2t_network_connection_id: CipUdint,
    /// Chosen by the originator
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,
    /// Requested packet interval, originator to target, in microseconds
    pub o2t_rpi: CipUdint,
    /// Requested packet interval, target to originator, in microseconds
    pub t2o_rpi: CipUdint,
    pub o2t: ConnectionDirection,
    pub t2o: ConnectionDirection,
    pub transport_type_trigger: TransportTypeTrigger,
    pub connection_path: CipPath,
    /// Send a Large_Forward_Open (32-bit connection parameters) instead of a Forward_Open
    pub large: bool,
}

// ======= Start of ForwardOpenRequest impl ========

impl ForwardOpenRequest {
    /// Builds the request data, computing the connection size of each direction
    pub fn new(
        parameters: &ConnectionParameters,
    ) -> Result<ForwardOpenRequest, ConnectionSizeError> {
        let transport_class = parameters.transport_type_trigger.transport_class();

        Ok(ForwardOpenRequest {
            priority_time_tick: parameters.priority_time_tick,
            timeout_ticks: parameters.timeout_ticks,
            o2t_network_connection_id: parameters.o2t_network_connection_id,
            t2o_network_connection_id: parameters.t2o_network_connection_id,
            connection_triad: parameters.connection_triad,
            connection_timeout_multiplier: parameters.connection_timeout_multiplier,
            o2t_rpi: parameters.o2t_rpi,
            o2t_network_connection_parameters: NetworkConnectionParameters::new(
                &parameters.o2t,
                transport_class,
                parameters.large,
            )?,
            t2o_rpi: parameters.t2o_rpi,
            t2o_network_connection_parameters: NetworkConnectionParameters::new(
                &parameters.t2o,
                transport_class,
                parameters.large,
            )?,
            transport_type_trigger: parameters.transport_type_trigger,
            connection_path: parameters.connection_path.clone(),
        })
    }

    /// The service this request is sent with, decided by the width of its connection parameters
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
    pub o2t_network_connection_id: CipUdint,
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,
    /// Actual packet interval, originator to target, in microseconds
    pub o2t_api: CipUdint,
    /// Actual packet interval, target to originator, in microseconds
    pub t2o_api: CipUdint,
    pub application_reply: ApplicationReply,
}

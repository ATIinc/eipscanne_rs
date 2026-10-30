//! The Message Router a device implements and its explicit messaging options: the services a
//! scanner's requests reach, and the paths, connection parameters and responses of those services.

use std::io::Cursor;
use std::net::IpAddr;
use std::time::Duration;

use binrw::BinWrite;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, ConnectionType, NetworkConnectionParameters,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, Rejection, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::error::{Error, Result};

/// What a scanner's requests reach behind [`crate::server::serve`]; calls can overlap, and a
/// service it leaves out is not supported
pub trait MessageRouter: Send + Sync + 'static {
    /// Get_Attributes_All
    fn handle_get_attributes_all(
        &self,
        _class: u16,
        _instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Get_Attribute_Single
    fn handle_get_attribute_single(
        &self,
        _class: u16,
        _instance: u16,
        _attribute: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Set_Attributes_All
    fn handle_set_attributes_all(
        &self,
        _class: u16,
        _instance: u16,
        _data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Set_Attribute_Single
    fn handle_set_attribute_single(
        &self,
        _class: u16,
        _instance: u16,
        _attribute: u16,
        _data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Connections only: Forward_Open and Large_Forward_Open, and the reply's Socket Address Info
    /// items
    fn handle_forward_open(
        &self,
        forward_open_request: ForwardOpenRequest,
        _originator_ip: IpAddr,
        _t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> Result<(ForwardOpenResponse, SocketAddrInfoItems), Rejection> {
        Err(Rejection {
            service: forward_open_request.service_code(),
            general_status: ResponseStatusCode::ServiceNotSupported,
            additional_status: vec![],
        })
    }

    /// Connections only: Forward_Close
    fn handle_forward_close(
        &self,
        _forward_close_request: ForwardCloseRequest,
        _originator_ip: IpAddr,
    ) -> Result<ForwardCloseResponse, Rejection> {
        Err(Rejection {
            service: ServiceCode::ForwardClose,
            general_status: ResponseStatusCode::ServiceNotSupported,
            additional_status: vec![],
        })
    }

    /// Connection Manager: accepts a request on the class 3 connection `connection_id` from
    /// `originator_ip`; the T->O connection ID and size of its reply, or
    /// `Error::UnexpectedPacket` when the request is discarded
    fn accept_connected_request(
        &self,
        connection_id: CipUdint,
        _cip_sequence_count: CipUint,
        _originator_ip: IpAddr,
    ) -> Result<(CipUdint, u16)> {
        Err(Error::UnexpectedPacket(format!(
            "request for no class 3 connection ({connection_id:#010x})"
        )))
    }
}

/// The device's explicit messaging, fixed for the server's life
#[derive(Clone, Copy)]
pub struct ExplicitOptions {
    /// The TCP/IP Interface object's Encapsulation Inactivity Timeout: a session without
    /// encapsulation activity for this long closes, unless `None`. A Set of the attribute cannot
    /// change it
    pub encapsulation_inactivity_timeout: Option<Duration>,
}

impl Default for ExplicitOptions {
    fn default() -> Self {
        ExplicitOptions {
            // The attribute's default
            encapsulation_inactivity_timeout: Some(Duration::from_secs(120)),
        }
    }
}

/// The response to a `service` request
pub(crate) fn message_router_response(
    service: ServiceCode,
    status: ResponseStatusCode,
    additional_status: Vec<CipUint>,
    data: CipDataOpt,
) -> MessageRouterResponse {
    MessageRouterResponse {
        service_container: ServiceContainer::new_response(service),
        response_data: ResponseData {
            status,
            additional_status_size: additional_status.len() as u8,
            additional_status,
            data,
        },
    }
}

/// The response accepting `forward_open_request` as it asks
pub(crate) fn successful_forward_open_response(
    forward_open_request: &ForwardOpenRequest,
) -> ForwardOpenResponse {
    ForwardOpenResponse {
        // Chosen by the target; the T->O id is the scanner's
        o2t_network_connection_id: rand::random(),
        t2o_network_connection_id: forward_open_request.t2o_network_connection_id,
        connection_triad: forward_open_request.connection_triad,
        // Granted as requested
        o2t_actual_packet_interval: forward_open_request.o2t_requested_packet_interval,
        t2o_actual_packet_interval: forward_open_request.t2o_requested_packet_interval,
        application_reply_size: 0,
        application_reply: vec![],
    }
}

/// The response closing the connection of `triad`
pub(crate) fn successful_forward_close_response(triad: ConnectionTriad) -> ForwardCloseResponse {
    ForwardCloseResponse {
        connection_triad: triad,
        application_reply_size: 0,
        application_reply: vec![],
    }
}

/// The response carrying `rejection` and the triad of the request it refuses
pub(crate) fn rejection_response(
    rejection: Rejection,
    triad: ConnectionTriad,
) -> MessageRouterResponse {
    tracing::warn!(
        service = ?rejection.service,
        general_status = ?rejection.general_status,
        extended_status = ?rejection.extended_status(),
        "refused a Connection Manager request"
    );
    message_router_response(
        rejection.service,
        rejection.general_status,
        rejection.additional_status,
        // Incomplete: Forward_Close refusals lack the trailing Remaining Path Size and reserved byte
        CipDataOpt::Typed(Box::new(triad)),
    )
}

/// A Connection Manager refusal: Connection Failure with `extended_status` as the Additional Status
pub(crate) fn connection_failure_rejection(
    service: ServiceCode,
    extended_status: ConnectionManagerExtendedStatus,
) -> Rejection {
    // The status codes exist only as binrw magics, so the word is written, not cast
    let mut word = Cursor::new([0u8; 2]);
    extended_status
        .write_le(&mut word)
        .expect("an extended status is one word");
    Rejection {
        service,
        general_status: ResponseStatusCode::ConnectionFailure,
        additional_status: vec![CipUint::from_le_bytes(word.into_inner())],
    }
}

/// The class, instance and optional attribute of a logical path
pub(crate) fn class_instance_and_attribute(path: &CipPath) -> Option<(u16, u16, Option<u16>)> {
    match path.segments.as_slice() {
        [class, instance] => Some((
            segment_id(class, LogicalSegmentType::ClassId)?,
            segment_id(instance, LogicalSegmentType::InstanceId)?,
            None,
        )),
        [class, instance, attribute] => Some((
            segment_id(class, LogicalSegmentType::ClassId)?,
            segment_id(instance, LogicalSegmentType::InstanceId)?,
            Some(segment_id(attribute, LogicalSegmentType::AttributeId)?),
        )),
        _ => None,
    }
}

/// The value of a segment of `logical_segment_type`
pub(crate) fn segment_id(
    segment: &LogicalPathSegment,
    logical_segment_type: LogicalSegmentType,
) -> Option<u16> {
    if segment.path_definition.logical_segment_type() != logical_segment_type {
        return None;
    }
    match segment.data {
        PathData::FormatAsU8(value) => Some(value.into()),
        PathData::FormatAsU16(value) => Some(value),
        PathData::ElectronicKey(_) => None,
    }
}

/// The connection size of a direction
pub(crate) fn network_connection_size(parameters: &NetworkConnectionParameters) -> u16 {
    match parameters {
        NetworkConnectionParameters::Standard(parameters) => parameters.connection_size().value(),
        NetworkConnectionParameters::Large(parameters) => parameters.connection_size(),
    }
}

/// Whether the connection size of a direction is fixed or variable
pub(crate) fn network_connection_size_type(
    parameters: &NetworkConnectionParameters,
) -> ConnectionSizeType {
    match parameters {
        NetworkConnectionParameters::Standard(parameters) => parameters.connection_size_type(),
        NetworkConnectionParameters::Large(parameters) => parameters.connection_size_type(),
    }
}

/// The connection type of a direction
pub(crate) fn network_connection_type(parameters: &NetworkConnectionParameters) -> ConnectionType {
    match parameters {
        NetworkConnectionParameters::Standard(parameters) => parameters.connection_type(),
        NetworkConnectionParameters::Large(parameters) => parameters.connection_type(),
    }
}

/// The timeout multiplier times the O->T actual packet interval
pub(crate) fn o2t_timeout(
    forward_open_request: &ForwardOpenRequest,
    forward_open_response: &ForwardOpenResponse,
) -> Duration {
    // x512 on a 10 s interval overflows 32-bit microseconds
    Duration::from_micros(
        u64::from(
            forward_open_request
                .connection_timeout_multiplier
                .multiplier(),
        ) * u64::from(forward_open_response.o2t_actual_packet_interval),
    )
}

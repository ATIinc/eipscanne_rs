//! What Forward_Open and Forward_Close need: the ids in paths, the connection parameters, the
//! O->T timeout and the responses.

use std::io::Cursor;
use std::time::Duration;

use binrw::BinWrite;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionType, NetworkConnectionParameters,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::response::{Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::cip::types::CipUint;

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

/// The connection size of a direction
pub(crate) fn network_connection_size(parameters: &NetworkConnectionParameters) -> u16 {
    match parameters {
        NetworkConnectionParameters::Standard(parameters) => parameters.connection_size().value(),
        NetworkConnectionParameters::Large(parameters) => parameters.connection_size(),
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

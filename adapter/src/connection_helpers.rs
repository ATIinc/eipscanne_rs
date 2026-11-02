//! What Forward_Open and Forward_Close need: the ids in paths, the connection parameters, the
//! O->T timeout and the responses.

use std::io::Cursor;
use std::time::Duration;

use binrw::BinWrite;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, ConnectionType, NetworkConnectionParameters,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::response::{Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::{
    CipPath, ElectronicKey, LogicalPathSegment, LogicalSegmentType, PathData,
};
use eipscanne_rs::cip::types::CipUint;

use crate::message_router::MessageRouter;

// ======= Responses ========

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
    Rejection {
        service,
        general_status: ResponseStatusCode::ConnectionFailure,
        additional_status: vec![encoded_word_from_binrw_magic(&extended_status)],
    }
}

/// The word `code` is written as: codes that exist only as binrw magics, like extended statuses,
/// vendor IDs and device types, cannot be cast
pub(crate) fn encoded_word_from_binrw_magic(
    code: &impl for<'a> BinWrite<Args<'a> = ()>,
) -> CipUint {
    let mut word = Cursor::new([0u8; 2]);
    code.write_le(&mut word).expect("a code is one word");
    CipUint::from_le_bytes(word.into_inner())
}

// ======= Connection parameters ========

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

// ======= O->T timeout ========

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

// ======= Ids in paths ========

/// `path` without its first electronic key, and that key. Unlike CIP, the key may sit anywhere
/// instead of first
pub(crate) fn split_electronic_key(path: &CipPath) -> (CipPath, Option<ElectronicKey>) {
    let mut segments = Vec::new();
    let mut electronic_key = None;
    for (index, segment) in path.segments.iter().enumerate() {
        match segment.data {
            PathData::ElectronicKey(key) if electronic_key.is_none() => {
                if index != 0 {
                    tracing::warn!(
                        index,
                        ?path,
                        "electronic key is not the path's first segment"
                    );
                }
                electronic_key = Some(key);
            }
            _ => segments.push(*segment),
        }
    }
    (CipPath::from_segments(segments), electronic_key)
}

/// Accepts `electronic_key` when each of its non-zero fields matches `message_router`'s identity,
/// with Compatibility set any minor revision up to the device's too; otherwise the part that does
/// not match. A device without an identity accepts any key
pub(crate) fn verify_electronic_key(
    message_router: &impl MessageRouter,
    electronic_key: &ElectronicKey,
) -> Result<(), ConnectionManagerExtendedStatus> {
    let Some(identity) = message_router.identity() else {
        return Ok(());
    };
    let major_revision = electronic_key
        .major_revision_compatibility
        .major_revision()
        .value();
    let minor_revision_differs = if electronic_key.major_revision_compatibility.compatibility() {
        electronic_key.minor_revision > identity.revision.minor
    } else {
        electronic_key.minor_revision != 0
            && electronic_key.minor_revision != identity.revision.minor
    };

    // A zero key field is not checked
    if (electronic_key.vendor_id != 0
        && electronic_key.vendor_id != encoded_word_from_binrw_magic(&identity.vendor_id))
        || (electronic_key.product_code != 0
            && electronic_key.product_code != identity.product_code)
    {
        Err(ConnectionManagerExtendedStatus::VendorIdOrProductCodeMismatch)
    } else if electronic_key.device_type != 0
        && electronic_key.device_type != encoded_word_from_binrw_magic(&identity.device_type)
    {
        Err(ConnectionManagerExtendedStatus::ProductTypeMismatch)
    } else if (major_revision != 0 && major_revision != identity.revision.major)
        || minor_revision_differs
    {
        Err(ConnectionManagerExtendedStatus::RevisionMismatch)
    } else {
        Ok(())
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

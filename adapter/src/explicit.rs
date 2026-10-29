//! Explicit messaging: each request a session receives, decoded for its handler and answered.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinRead;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::message_router::{MessageRouter, message_router_response};

/// The data of a Set, or a connection's outputs, as `T`, or the General Status refusing it; mirrors
/// the scanner's `decode_reply`
pub fn decode_data<T>(data: &[u8]) -> Result<T, ResponseStatusCode>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut reader = Cursor::new(data);
    let value = T::read_le(&mut reader).map_err(|error| {
        if error.is_eof() {
            ResponseStatusCode::NotEnoughData
        } else {
            ResponseStatusCode::InvalidAttributeValue
        }
    })?;
    if reader.position() < data.len() as u64 {
        return Err(ResponseStatusCode::TooMuchData);
    }
    Ok(value)
}

/// The response to `request` from `originator_ip`, and the reply's Socket Address Info items;
/// Single services need an attribute, All services none
pub(crate) fn handle_request(
    message_router: &impl MessageRouter,
    request: &MessageRouterRequest,
    originator_ip: IpAddr,
    t2o_socket_addr_info: Option<SocketAddrInfo>,
) -> (MessageRouterResponse, SocketAddrInfoItems) {
    let service = request.service_container.service();
    let path_ids = class_instance_and_attribute(&request.request_data.cip_path);
    let CipDataOpt::Raw(request_data) = &request.request_data.additional_data else {
        unreachable!("data read from the wire is always raw");
    };

    let data_or_status = match (service, path_ids) {
        (ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen, _) => {
            return message_router.handle_forward_open(
                service,
                request_data,
                originator_ip,
                t2o_socket_addr_info,
            );
        }
        (ServiceCode::ForwardClose, _) => {
            return (
                message_router.handle_forward_close(request_data, originator_ip),
                SocketAddrInfoItems::empty(),
            );
        }
        (ServiceCode::GetAttributeAll, Some((class, instance, None))) => {
            message_router.handle_get_attributes_all(class, instance)
        }
        (ServiceCode::GetAttributeSingle, Some((class, instance, Some(attribute)))) => {
            message_router.handle_get_attribute_single(class, instance, attribute)
        }
        (ServiceCode::SetAttributeAll, Some((class, instance, None))) => message_router
            .handle_set_attributes_all(class, instance, request_data)
            .map(|()| CipDataOpt::Raw(vec![])),
        (ServiceCode::SetAttributeSingle, Some((class, instance, Some(attribute)))) => {
            message_router
                .handle_set_attribute_single(class, instance, attribute, request_data)
                .map(|()| CipDataOpt::Raw(vec![]))
        }
        (
            ServiceCode::GetAttributeAll
            | ServiceCode::GetAttributeSingle
            | ServiceCode::SetAttributeAll
            | ServiceCode::SetAttributeSingle,
            Some(_),
        ) => Err(ResponseStatusCode::PathDestinationUnknown),
        (
            ServiceCode::GetAttributeAll
            | ServiceCode::GetAttributeSingle
            | ServiceCode::SetAttributeAll
            | ServiceCode::SetAttributeSingle,
            None,
        ) => Err(ResponseStatusCode::PathSegmentError),
        _ => Err(ResponseStatusCode::ServiceNotSupported),
    };

    let response = match data_or_status {
        Ok(response_data) => {
            tracing::debug!(?service, ?path_ids, "answered a request");
            message_router_response(service, ResponseStatusCode::Success, vec![], response_data)
        }
        Err(status) => {
            tracing::warn!(?service, ?path_ids, ?status, "refused a request");
            message_router_response(service, status, vec![], CipDataOpt::Raw(vec![]))
        }
    };
    (response, SocketAddrInfoItems::empty())
}

/// The class, instance and optional attribute of a logical path
fn class_instance_and_attribute(path: &CipPath) -> Option<(u16, u16, Option<u16>)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_data_reads_exactly_the_data() {
        assert_eq!(
            decode_data::<u32>(&[0xef, 0xbe, 0xad, 0xde]),
            Ok(0xdead_beef)
        );
    }

    #[test]
    fn decode_data_refuses_short_and_long_data() {
        assert_eq!(
            decode_data::<u32>(&[0xef, 0xbe]),
            Err(ResponseStatusCode::NotEnoughData)
        );
        assert_eq!(
            decode_data::<u32>(&[0xef, 0xbe, 0xad, 0xde, 0x00]),
            Err(ResponseStatusCode::TooMuchData)
        );
    }
}

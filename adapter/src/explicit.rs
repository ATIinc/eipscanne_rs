//! Explicit messaging: the Message Router, which answers every request a Send RR Data packet
//! carries.

use std::net::IpAddr;
use std::sync::Arc;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::command::{EncapsStatusCode, RRPacketData};
use eipscanne_rs::eip::constants::{CIP_INTERFACE_HANDLE, NO_ENCAPSULATION_TIMEOUT};

use crate::device::Device;
use crate::implicit::connection::ConnectionManager;

/// The reply to the unconnected message `rr_data` carries: Forward_Open and Forward_Close go to the
/// Connection Manager, Get and Set to the device
pub(crate) fn handle_unconnected_message(
    device: &Arc<dyn Device>,
    rr_data: &RRPacketData,
    originator_ip: IpAddr,
    connection_manager: Option<&ConnectionManager>,
) -> Result<RRPacketData, EncapsStatusCode> {
    let CipMessage::Request(request) = &rr_data.cip_message else {
        return Err(EncapsStatusCode::InvalidFormatOrData);
    };

    let response = match (request.service_container.service(), connection_manager) {
        (ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen, Some(connection_manager)) => {
            connection_manager.handle_forward_open(
                device,
                request,
                originator_ip,
                // Only a Forward_Open request carries a T->O item
                rr_data.socket_addr_info_items.t2o,
            )
        }
        (ServiceCode::ForwardClose, Some(connection_manager)) => {
            connection_manager.handle_forward_close(request)
        }
        _ => handle_device_request(device.as_ref(), request),
    };

    let mut reply =
        RRPacketData::new_unconnected(CIP_INTERFACE_HANDLE, NO_ENCAPSULATION_TIMEOUT, response);
    // Only a device with implicit messaging adds Socket Address Info
    if let Some(connection_manager) = connection_manager {
        reply.socket_addr_info_items =
            connection_manager.socket_addr_info_items(&reply.cip_message);
    }
    Ok(reply)
}

/// The device's response to a Get or Set; Single services need an attribute, All services none
fn handle_device_request(
    device: &dyn Device,
    request: &MessageRouterRequest,
) -> MessageRouterResponse {
    let service = request.service_container.service();
    let ids = cip_path_ids(&request.request_data.cip_path);
    let CipDataOpt::Raw(data) = &request.request_data.additional_data else {
        unreachable!("data read from the wire is always raw");
    };

    let reply = match (service, ids) {
        (ServiceCode::GetAttributeAll, Some((class, instance, None))) => {
            device.handle_get_attributes_all(class, instance)
        }
        (ServiceCode::GetAttributeSingle, Some((class, instance, Some(attribute)))) => {
            device.handle_get_attribute_single(class, instance, attribute)
        }
        (ServiceCode::SetAttributeAll, Some((class, instance, None))) => device
            .handle_set_attributes_all(class, instance, data)
            .map(|()| CipDataOpt::Raw(vec![])),
        (ServiceCode::SetAttributeSingle, Some((class, instance, Some(attribute)))) => device
            .handle_set_attribute_single(class, instance, attribute, data)
            .map(|()| CipDataOpt::Raw(vec![])),
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

    match reply {
        Ok(data) => {
            tracing::debug!(?service, ?ids, "answered a request");
            message_router_response(service, ResponseStatusCode::Success, vec![], data)
        }
        Err(status) => {
            tracing::warn!(?service, ?ids, ?status, "refused a request");
            message_router_response(service, status, vec![], CipDataOpt::Raw(vec![]))
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

/// The class, instance and optional attribute of a logical path
fn cip_path_ids(path: &CipPath) -> Option<(u16, u16, Option<u16>)> {
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

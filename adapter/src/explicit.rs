//! Explicit messaging: Get_Attribute and Set_Attribute requests, answered by the device.

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};

use crate::device::Device;

/// Hands a Get or Set to the device; Single services need an attribute, All services none
pub(crate) fn handle_request(
    request: &MessageRouterRequest,
    device: &dyn Device,
) -> std::result::Result<CipDataOpt, ResponseStatusCode> {
    let service = request.service_container.service();
    let Some((class, instance, attribute)) = cip_path_ids(&request.request_data.cip_path) else {
        tracing::warn!(
            ?service,
            "refused a request whose path is not class, instance and attribute"
        );
        return Err(ResponseStatusCode::PathSegmentError);
    };
    let CipDataOpt::Raw(data) = &request.request_data.additional_data else {
        unreachable!("data read from the wire is always raw");
    };

    let reply = match (service, attribute) {
        (ServiceCode::GetAttributeAll, None) => device.handle_get_attributes_all(class, instance),
        (ServiceCode::GetAttributeSingle, Some(attribute)) => {
            device.handle_get_attribute_single(class, instance, attribute)
        }
        (ServiceCode::SetAttributeAll, None) => device
            .handle_set_attributes_all(class, instance, data)
            .map(|()| CipDataOpt::Raw(vec![])),
        (ServiceCode::SetAttributeSingle, Some(attribute)) => device
            .handle_set_attribute_single(class, instance, attribute, data)
            .map(|()| CipDataOpt::Raw(vec![])),
        (
            ServiceCode::GetAttributeAll
            | ServiceCode::GetAttributeSingle
            | ServiceCode::SetAttributeAll
            | ServiceCode::SetAttributeSingle,
            _,
        ) => Err(ResponseStatusCode::PathDestinationUnknown),
        _ => Err(ResponseStatusCode::ServiceNotSupported),
    };

    match &reply {
        Ok(_) => tracing::debug!(?service, class, instance, ?attribute, "answered a request"),
        Err(status) => {
            tracing::warn!(
                ?service,
                class,
                instance,
                ?attribute,
                ?status,
                "refused a request"
            )
        }
    }
    reply
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
fn segment_id(
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

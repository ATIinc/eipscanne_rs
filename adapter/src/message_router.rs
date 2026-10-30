//! The Message Router a device implements and its explicit messaging options: the services a
//! scanner's requests reach, and the paths and responses of those services.

use std::time::Duration;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::cip::types::CipUint;

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

//! Explicit messaging: each request, decoded for its handler and answered.

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;

use crate::message_router::{MessageRouter, class_instance_and_attribute, message_router_response};

pub(crate) mod unconnected;

/// The response to `request`. Single services need an attribute, All services none
pub(crate) fn handle_message_router_request(
    message_router: &impl MessageRouter,
    request: &MessageRouterRequest,
) -> MessageRouterResponse {
    let service = request.service_container.service();
    let path_ids = class_instance_and_attribute(&request.request_data.cip_path);
    let CipDataOpt::Raw(request_data) = &request.request_data.additional_data else {
        unreachable!("data read from the wire is always raw");
    };

    let data_or_status = match (service, path_ids) {
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

    match data_or_status {
        Ok(response_data) => {
            tracing::debug!(?service, ?path_ids, "answered a request");
            message_router_response(service, ResponseStatusCode::Success, vec![], response_data)
        }
        Err(status) => {
            tracing::warn!(?service, ?path_ids, cip_path = ?request.request_data.cip_path, ?status, "refused a request");
            message_router_response(service, status, vec![], CipDataOpt::Raw(vec![]))
        }
    }
}

//! Explicit messaging: each request, decoded for its handler and answered.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinRead;

use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseRequest;
use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::message_router::{MessageRouter, class_instance_and_attribute, message_router_response};

pub(crate) mod unconnected;

/// The response to `request` from `originator_ip`, and the reply's Socket Address Info items.
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
        // Connection Manager: opens a class 1 connection for implicit I/O
        (ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen, _) => {
            let is_large_forward_open = service == ServiceCode::LargeForwardOpen;
            let forward_open_request = match ForwardOpenRequest::read_le_args(
                &mut Cursor::new(request_data),
                (is_large_forward_open,),
            ) {
                Ok(forward_open_request) => forward_open_request,
                Err(error) => {
                    return (
                        decode_failure_response(service, &error),
                        SocketAddrInfoItems::empty(),
                    );
                }
            };
            return message_router.handle_forward_open(
                forward_open_request,
                originator_ip,
                t2o_socket_addr_info,
            );
        }
        // Connection Manager: closes a class 1 connection and its implicit I/O
        (ServiceCode::ForwardClose, _) => {
            let forward_close_request =
                match ForwardCloseRequest::read_le(&mut Cursor::new(request_data)) {
                    Ok(forward_close_request) => forward_close_request,
                    Err(error) => {
                        return (
                            decode_failure_response(service, &error),
                            SocketAddrInfoItems::empty(),
                        );
                    }
                };
            return (
                message_router.handle_forward_close(forward_close_request, originator_ip),
                SocketAddrInfoItems::empty(),
            );
        }
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

/// The response refusing a Connection Manager request that does not decode
fn decode_failure_response(service: ServiceCode, error: &binrw::Error) -> MessageRouterResponse {
    let general_status = if error.is_eof() {
        ResponseStatusCode::NotEnoughData
    } else {
        ResponseStatusCode::InvalidParameterValue
    };
    tracing::warn!(
        ?service,
        ?general_status,
        "refused a Connection Manager request"
    );
    message_router_response(service, general_status, vec![], CipDataOpt::Raw(vec![]))
}

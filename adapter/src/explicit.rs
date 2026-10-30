//! Explicit messaging: each unconnected or connected request, decoded for its handler and
//! answered.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinRead;

use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseRequest;
use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::TransportClass;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::explicit::connected::Class3Connection;
use crate::explicit::connected::open_close::{
    handle_connected_forward_close, handle_connected_forward_open,
};
use crate::message_router::{
    MessageRouter, class_instance_and_attribute, message_router_response, rejection_response,
};

pub(crate) mod connected;
pub(crate) mod unconnected;

/// The response to `request` from `originator_ip`, and the reply's Socket Address Info items; a
/// class 3 Forward_Open and Forward_Close open and close `class_3_connection` when it is accepted.
/// Single services need an attribute, All services none
pub(crate) fn handle_message_router_request(
    message_router: &impl MessageRouter,
    request: &MessageRouterRequest,
    originator_ip: IpAddr,
    class_3_connection: &mut Class3Connection,
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
        // Connection Manager: opens a class 3 connection, or a class 1 one for implicit I/O
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
            let triad = forward_open_request.connection_triad;
            let is_class_3 = forward_open_request
                .transport_type_trigger
                .transport_class()
                == TransportClass::Class3;
            // Class 3 opens the session's connection when it is accepted; any other Forward_Open
            // goes to the Message Router
            let forward_open_result = if class_3_connection.accepted && is_class_3 {
                handle_connected_forward_open(forward_open_request, class_3_connection).map(
                    |forward_open_response| (forward_open_response, SocketAddrInfoItems::empty()),
                )
            } else {
                message_router.handle_forward_open(
                    forward_open_request,
                    originator_ip,
                    t2o_socket_addr_info,
                )
            };
            return match forward_open_result {
                Ok((forward_open_response, socket_addr_info_items)) => (
                    message_router_response(
                        service,
                        ResponseStatusCode::Success,
                        vec![],
                        CipDataOpt::Typed(Box::new(forward_open_response)),
                    ),
                    socket_addr_info_items,
                ),
                Err(rejection) => (
                    rejection_response(rejection, triad),
                    SocketAddrInfoItems::empty(),
                ),
            };
        }
        // Connection Manager: closes a class 3 connection, or a class 1 one and its implicit I/O
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
            let triad = forward_close_request.connection_triad;
            // The session's class 3 connection first; any other goes to the Message Router
            let forward_close_result =
                match handle_connected_forward_close(&forward_close_request, class_3_connection) {
                    Some(forward_close_response) => Ok(forward_close_response),
                    None => {
                        message_router.handle_forward_close(forward_close_request, originator_ip)
                    }
                };
            let response = match forward_close_result {
                Ok(forward_close_response) => message_router_response(
                    service,
                    ResponseStatusCode::Success,
                    vec![],
                    CipDataOpt::Typed(Box::new(forward_close_response)),
                ),
                Err(rejection) => rejection_response(rejection, triad),
            };
            return (response, SocketAddrInfoItems::empty());
        }
        _ => Err(ResponseStatusCode::ServiceNotSupported),
    };

    let response = match data_or_status {
        Ok(response_data) => {
            tracing::debug!(?service, ?path_ids, "answered a request");
            message_router_response(service, ResponseStatusCode::Success, vec![], response_data)
        }
        Err(status) => {
            tracing::warn!(?service, ?path_ids, cip_path = ?request.request_data.cip_path, ?status, "refused a request");
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

//! The Message Router a device implements: the services a scanner's requests reach.

use std::net::IpAddr;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

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

    /// Implicit messaging only: Forward_Open and Large_Forward_Open, and the reply's Socket Address
    /// Info items
    fn handle_forward_open(
        &self,
        service: ServiceCode,
        _forward_open_data: &[u8],
        _originator_ip: IpAddr,
        _t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> (MessageRouterResponse, SocketAddrInfoItems) {
        tracing::warn!(?service, "refused a service that is not supported");
        (
            message_router_response(
                service,
                ResponseStatusCode::ServiceNotSupported,
                vec![],
                CipDataOpt::Raw(vec![]),
            ),
            SocketAddrInfoItems::empty(),
        )
    }

    /// Implicit messaging only: Forward_Close
    fn handle_forward_close(
        &self,
        _forward_close_data: &[u8],
        _originator_ip: IpAddr,
    ) -> MessageRouterResponse {
        let service = ServiceCode::ForwardClose;
        tracing::warn!(?service, "refused a service that is not supported");
        message_router_response(
            service,
            ResponseStatusCode::ServiceNotSupported,
            vec![],
            CipDataOpt::Raw(vec![]),
        )
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

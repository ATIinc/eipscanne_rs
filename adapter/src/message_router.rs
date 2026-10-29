//! The Message Router a device implements: the services a scanner's requests reach.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinWrite;

use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, Rejection, ResponseData, ResponseStatusCode,
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

    /// Explicit messaging only: whether a class 3 Forward_Open opens a connection
    fn accepts_explicit_connections(&self) -> bool {
        false
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

/// The response carrying `rejection` and the triad of the request it refuses
pub(crate) fn rejection_response(
    rejection: Rejection,
    triad: ConnectionTriad,
) -> MessageRouterResponse {
    tracing::warn!(
        service = ?rejection.service,
        general_status = ?rejection.general_status,
        extended_status = ?rejection.extended_status(),
        "refused a Connection Manager request"
    );
    message_router_response(
        rejection.service,
        rejection.general_status,
        rejection.additional_status,
        CipDataOpt::Typed(Box::new(triad)),
    )
}

/// A Connection Manager refusal: Connection Failure with `extended_status` as the Additional Status
pub(crate) fn connection_failure(
    service: ServiceCode,
    extended_status: ConnectionManagerExtendedStatus,
) -> Rejection {
    let mut bytes = Cursor::new(Vec::new());
    extended_status
        .write_le(&mut bytes)
        .expect("writing to memory does not fail");
    let word = CipUint::from_le_bytes(
        bytes
            .into_inner()
            .try_into()
            .expect("an extended status is one word"),
    );
    Rejection {
        service,
        general_status: ResponseStatusCode::ConnectionFailure,
        additional_status: vec![word],
    }
}

/// The refusal of a Connection Manager request that does not decode
pub(crate) fn decode_failure(service: ServiceCode, error: &binrw::Error) -> MessageRouterResponse {
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

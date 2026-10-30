//! The Message Router a device implements and its explicit messaging options: the services a
//! scanner's requests reach.

use std::net::IpAddr;
use std::time::Duration;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::error::{Error, Result};

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

    /// Connections only: Forward_Open and Large_Forward_Open, and the reply's Socket Address Info
    /// items
    fn handle_forward_open(
        &self,
        forward_open_request: ForwardOpenRequest,
        _originator_ip: IpAddr,
        _t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> Result<(ForwardOpenResponse, SocketAddrInfoItems), Rejection> {
        Err(Rejection {
            service: forward_open_request.service_code(),
            general_status: ResponseStatusCode::ServiceNotSupported,
            additional_status: vec![],
        })
    }

    /// Connections only: Forward_Close
    fn handle_forward_close(
        &self,
        _forward_close_request: ForwardCloseRequest,
        _originator_ip: IpAddr,
    ) -> Result<ForwardCloseResponse, Rejection> {
        Err(Rejection {
            service: ServiceCode::ForwardClose,
            general_status: ResponseStatusCode::ServiceNotSupported,
            additional_status: vec![],
        })
    }

    /// Connection Manager: accepts a request on the class 3 connection `connection_id` from
    /// `originator_ip`; the T->O connection ID and size of its reply, or
    /// `Error::UnexpectedPacket` when the request is discarded
    fn accept_connected_request(
        &self,
        connection_id: CipUdint,
        _cip_sequence_count: CipUint,
        _originator_ip: IpAddr,
    ) -> Result<(CipUdint, u16)> {
        Err(Error::UnexpectedPacket(format!(
            "request for no class 3 connection ({connection_id:#010x})"
        )))
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

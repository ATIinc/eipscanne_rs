use crate::cip::connection_manager::forward_close::ForwardCloseRequest;
use crate::cip::connection_manager::forward_open::ForwardOpenRequest;
use crate::cip::message::data::CipData;
use crate::cip::message::{request::MessageRouterRequest, shared::ServiceCode};
use crate::cip::object_ids::{CONNECTION_MANAGER_CLASS_ID, CONNECTION_MANAGER_INSTANCE_ID};
use crate::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use crate::cip::path::CipPath;
use crate::cip::types::CipUdint;
use crate::eip::constants::NO_ENCAPSULATION_TIMEOUT;
use crate::eip::packet::EnIpPacket;

/// An encapsulated packet sent by the scanner. The same type as [`ResponseObjectAssembly`]; the
/// name only documents the direction. Read it with `EnIpPacket::read_request` (`adapter` feature).
pub type RequestObjectAssembly = EnIpPacket;

/// An encapsulated packet received from the adapter. The same type as [`RequestObjectAssembly`];
/// the name only documents the direction. Read it with [`EnIpPacket::read_response`].
pub type ResponseObjectAssembly = EnIpPacket;

// ======= Start of RequestObjectAssembly impl ========

impl RequestObjectAssembly {
    pub fn new_identity(session_handle: CipUdint) -> Self {
        Self::new_service_request(
            session_handle,
            CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
            ServiceCode::GetAttributeAll,
            None,
        )
    }

    pub fn new_service_request(
        session_handle: CipUdint,
        request_path: CipPath,
        service_code: ServiceCode,
        data: Option<Box<dyn CipData>>,
    ) -> Self {
        Self::new_send_rr_data(
            session_handle,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterRequest::new_data(service_code, request_path, data),
        )
    }

    /// Forward_Open or Large_Forward_Open, decided by the width of the request's connection
    /// parameters
    pub fn new_forward_open(session_handle: CipUdint, request: ForwardOpenRequest) -> Self {
        let service_code = request.service_code();
        Self::new_service_request(
            session_handle,
            CipPath::new(CONNECTION_MANAGER_CLASS_ID, CONNECTION_MANAGER_INSTANCE_ID),
            service_code,
            Some(Box::new(request)),
        )
    }

    pub fn new_forward_close(session_handle: CipUdint, request: ForwardCloseRequest) -> Self {
        Self::new_service_request(
            session_handle,
            CipPath::new(CONNECTION_MANAGER_CLASS_ID, CONNECTION_MANAGER_INSTANCE_ID),
            ServiceCode::ForwardClose,
            Some(Box::new(request)),
        )
    }
}

// ^^^^^^^^ End of RequestObjectAssembly impl ^^^^^^^^

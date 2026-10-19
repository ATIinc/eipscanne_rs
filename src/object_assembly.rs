use crate::cip::message::data::CipData;
use crate::cip::message::{request::MessageRouterRequest, shared::ServiceCode};
use crate::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use crate::cip::path::CipPath;
use crate::cip::types::CipUdint;
use crate::eip::constants::NO_ENCAPSULATION_TIMEOUT;
use crate::eip::packet::EnIpPacket;

/// An encapsulated packet sent by the scanner. The same type as [`ResponseObjectAssembly`]; the
/// name only documents the direction.
pub type RequestObjectAssembly = EnIpPacket;

/// An encapsulated packet received from the adapter. The same type as [`RequestObjectAssembly`];
/// the name only documents the direction.
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
}

// ^^^^^^^^ End of RequestObjectAssembly impl ^^^^^^^^

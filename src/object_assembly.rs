use crate::cip::message::data::CipData;
use crate::cip::message::{
    request::MessageRouterRequest, response::MessageRouterResponse, shared::ServiceCode,
};
use crate::cip::path::CipPath;
use crate::cip::types::CipUdint;
use crate::eip::packet::EnIpPacket;

/// An encapsulated packet sent by the scanner
pub type RequestObjectAssembly = EnIpPacket<MessageRouterRequest>;

/// An encapsulated packet received from the adapter
pub type ResponseObjectAssembly = EnIpPacket<MessageRouterResponse>;

// ======= Start of RequestObjectAssembly impl ========

impl RequestObjectAssembly {
    pub fn new_identity(session_handle: CipUdint) -> Self {
        Self::new_service_request(
            session_handle,
            CipPath::new(0x1, 0x1),
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
            0,
            MessageRouterRequest::new_data(service_code, request_path, data),
        )
    }
}

// ^^^^^^^^ End of RequestObjectAssembly impl ^^^^^^^^

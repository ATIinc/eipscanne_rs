//! Forward_Close: closing a connection through the Connection Manager.

use std::io::Cursor;

use binrw::{BinRead, binrw};

use crate::cip::connection_manager::forward_open::ConnectionManagerExtendedStatus;
use crate::cip::connection_manager::parameters::PriorityTimeTick;
use crate::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use crate::cip::message::shared::ServiceCode;
use crate::cip::path::CipPath;
use crate::cip::types::{CipUdint, CipUint, CipUsint};

/// Forward_Close request data (everything after the request path)
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardCloseRequest {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,

    // Connection Path Size in 16-bit words, derived from the path on write, then a reserved byte
    #[br(temp)]
    #[bw(calc = connection_path.word_len() as CipUsint)]
    #[brw(pad_after = 1)]
    connection_path_size: CipUsint,

    #[br(args(connection_path_size))]
    pub connection_path: CipPath,
}

/// Forward_Close reply data when the general status is success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardCloseResponse {
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,

    /// Application Reply Size in 16-bit words, followed by a reserved byte
    #[brw(pad_after = 1)]
    pub application_reply_size: CipUsint,

    #[br(count = usize::from(application_reply_size) * 2)]
    pub application_reply: Vec<CipUsint>,
}

/// Forward_Close reply data when the general status is not success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardCloseUnsuccessfulResponse {
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,

    /// Remaining Path Size in 16-bit words and its reserved byte; only present when a routing
    /// node rejected the request
    #[br(try)]
    pub remaining_path_size: Option<(CipUsint, CipUsint)>,
}

/// Why the target rejected a Forward_Close
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardCloseFailure {
    pub general_status: ResponseStatusCode,
    /// The first Additional Status word, if the reply carried one
    pub extended_status: Option<ConnectionManagerExtendedStatus>,
    pub additional_status: Vec<CipUint>,
    /// The reply data, if the reply carried any
    pub response: Option<ForwardCloseUnsuccessfulResponse>,
}

#[derive(Debug)]
pub enum ForwardCloseError {
    /// The target answered with a general status other than success
    Rejected(ForwardCloseFailure),
    /// The reply is not a Forward_Close reply, or its data could not be parsed
    Malformed(binrw::Error),
}

impl From<binrw::Error> for ForwardCloseError {
    fn from(error: binrw::Error) -> Self {
        ForwardCloseError::Malformed(error)
    }
}

// ======= Start of ForwardCloseResponse impl ========

impl ForwardCloseResponse {
    /// Interprets the reply to a Forward_Close
    pub fn from_message_router_response(
        response: &MessageRouterResponse,
    ) -> Result<ForwardCloseResponse, ForwardCloseError> {
        let service = response.service_container.service();
        if service != ServiceCode::ForwardClose {
            return Err(ForwardCloseError::Malformed(binrw::Error::AssertFail {
                pos: 0,
                message: format!("expected a Forward_Close reply, got {service:?}"),
            }));
        }

        let data = response.response_data.data.to_bytes()?;
        let mut reader = Cursor::new(&data);

        if response.is_success() {
            return Ok(ForwardCloseResponse::read(&mut reader)?);
        }

        let unsuccessful_response = if data.is_empty() {
            None
        } else {
            Some(ForwardCloseUnsuccessfulResponse::read(&mut reader)?)
        };

        Err(ForwardCloseError::Rejected(ForwardCloseFailure {
            general_status: response.response_data.status,
            extended_status: response
                .response_data
                .additional_status
                .first()
                .map(|&code| ConnectionManagerExtendedStatus::from(code)),
            additional_status: response.response_data.additional_status.clone(),
            response: unsuccessful_response,
        }))
    }
}

// ^^^^^^^^ End of ForwardCloseResponse impl ^^^^^^^^

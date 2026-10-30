//! Connected explicit messaging: class 3 connections to the Message Router.
//!
//! ```text
//! Stage    What happens on the wire                                         Module
//! -------  ---------------------------------------------------------------  --------------
//! Open     SendRRData(Forward_Open) -> the reply                            message_router
//! Request  SendUnitData(request) on O->T -> SendUnitData(response) on T->O  send_unit_data
//! Close    SendRRData(Forward_Close) -> the reply                           message_router
//! ```
//!
//! Only a server whose Message Router is a [`ConnectedMessageRouter`] opens them. A request whose
//! CIP sequence count repeats the last one is discarded; a connection is deleted when no request
//! comes within its timeout.
//!
//! [`ConnectedMessageRouter`]: message_router::ConnectedMessageRouter

use std::net::IpAddr;
use std::time::Duration;

use tokio::time::Instant;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::types::CipUint;

pub mod message_router;
pub(crate) mod send_unit_data;

/// An established class 3 connection: the Forward_Open received and the reply sent
#[derive(Debug)]
pub(crate) struct ExplicitConnection {
    request: ForwardOpenRequest,
    response: ForwardOpenResponse,
    originator_ip: IpAddr,
    /// The timeout multiplier times the O->T actual packet interval
    inactivity_timeout: Duration,
    /// When it times out unless a request comes first
    deadline: Instant,
    /// The CIP sequence count of the last request; `None` before the first
    last_cip_sequence_count: Option<CipUint>,
}

impl ExplicitConnection {
    /// An established connection; its inactivity timer starts now
    fn new(
        request: ForwardOpenRequest,
        response: ForwardOpenResponse,
        originator_ip: IpAddr,
    ) -> Self {
        let inactivity_timeout = Duration::from_micros(
            u64::from(request.connection_timeout_multiplier.multiplier())
                * u64::from(response.o2t_actual_packet_interval),
        );
        ExplicitConnection {
            request,
            response,
            originator_ip,
            inactivity_timeout,
            deadline: Instant::now() + inactivity_timeout,
            last_cip_sequence_count: None,
        }
    }
}

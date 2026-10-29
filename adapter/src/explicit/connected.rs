//! Connected explicit messaging: one class 3 connection per session, to the Message Router.
//!
//! ```text
//! Stage    What happens on the wire                                         Module
//! -------  ---------------------------------------------------------------  --------------
//! Open     SendRRData(Forward_Open) -> the reply                            open_close
//! Request  SendUnitData(request) on O->T -> SendUnitData(response) on T->O  send_unit_data
//! Close    SendRRData(Forward_Close) -> the reply                           open_close
//! ```
//!
//! Only a server whose [`ExplicitOptions`] accept explicit connections opens one. A request whose
//! CIP sequence count repeats the last one is discarded; the connection is deleted when no request
//! comes within its timeout, and when its session ends.
//!
//! [`ExplicitOptions`]: crate::message_router::ExplicitOptions

use std::time::Duration;

use tokio::time::Instant;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::types::{CipUdint, CipUint};

use crate::error::{Error, Result};

pub(crate) mod open_close;
pub(crate) mod send_unit_data;

/// An established class 3 connection: the Forward_Open received and the reply sent
#[derive(Debug)]
pub(crate) struct ExplicitConnection {
    request: ForwardOpenRequest,
    response: ForwardOpenResponse,
    /// The timeout multiplier times the O->T actual packet interval
    inactivity_timeout: Duration,
    /// When it times out unless a request comes first
    deadline: Instant,
    /// The CIP sequence count of the last request; `None` before the first
    last_cip_sequence_count: Option<CipUint>,
}

impl ExplicitConnection {
    /// An established connection; its inactivity timer starts now
    fn new(request: ForwardOpenRequest, response: ForwardOpenResponse) -> Self {
        let inactivity_timeout = Duration::from_micros(
            u64::from(request.connection_timeout_multiplier.multiplier())
                * u64::from(response.o2t_actual_packet_interval),
        );
        ExplicitConnection {
            request,
            response,
            inactivity_timeout,
            deadline: Instant::now() + inactivity_timeout,
            last_cip_sequence_count: None,
        }
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Consumes a connected request on `connection_id`: restarts the inactivity timer;
    /// `Error::UnexpectedPacket` when the connection has another O->T ID or `cip_sequence_count`
    /// repeats the last one
    fn consume_request(
        &mut self,
        connection_id: CipUdint,
        cip_sequence_count: CipUint,
    ) -> Result<()> {
        if self.response.o2t_network_connection_id != connection_id {
            return Err(Error::UnexpectedPacket(format!(
                "request for another connection ({connection_id:#010x})"
            )));
        }

        self.deadline = Instant::now() + self.inactivity_timeout;

        if self.last_cip_sequence_count == Some(cip_sequence_count) {
            return Err(Error::UnexpectedPacket(format!(
                "repeated CIP sequence count {cip_sequence_count}"
            )));
        }

        self.last_cip_sequence_count = Some(cip_sequence_count);
        Ok(())
    }
}

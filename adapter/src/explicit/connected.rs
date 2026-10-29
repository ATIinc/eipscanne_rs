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
//! Only a [`MessageRouter`] that accepts explicit connections opens one. A request whose CIP
//! sequence count repeats the last one is discarded; the connection is deleted when no request
//! comes within its timeout, and when its session ends.
//!
//! [`MessageRouter`]: crate::message_router::MessageRouter

use std::time::Duration;

use tokio::time::Instant;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::types::CipUint;

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

    /// Restarts the inactivity timer
    fn restart_inactivity_timer(&mut self) {
        self.deadline = Instant::now() + self.inactivity_timeout;
    }
}

/// Deletes the connection once no request has come within its timeout
fn delete_timed_out_connection(class_3_connection: &mut Option<ExplicitConnection>) {
    if let Some(connection) =
        class_3_connection.take_if(|connection| connection.deadline <= Instant::now())
    {
        tracing::warn!(
            connection_serial_number = connection.request.connection_triad.connection_serial_number,
            "class 3 connection timed out: no request in time"
        );
    }
}

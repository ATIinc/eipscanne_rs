//! Connected explicit messaging: requests over a class 3 connection to the Message Router.
//!
//! ```text
//! Stage     What happens on the wire                                  Function
//! --------  --------------------------------------------------------  ---------------------------------
//! Open      SendRRData(Forward_Open), read the reply                  connection_manager::forward_open
//! Request   SendUnitData(request) on O->T, read SendUnitData on T->O  send_request
//! Close     SendRRData(Forward_Close), read the reply                 connection_manager::forward_close
//! ```
//!
//! Everything goes over the session's TCP connection. The scanner keeps no state between
//! requests: the caller owns the CIP Sequence Count (a new request advances it, a request sent
//! again keeps it) and sends often enough that the adapter does not time the connection out (see
//! the `read-identity-connected` example). Replies are read with [`decode_reply`].
//!
//! [`decode_reply`]: crate::explicit::decode_reply

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenResponse;
use eipscanne_rs::cip::message::data::CipData;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, Rejection};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::command::UnitPacketData;
use eipscanne_rs::eip::packet::EnIpPacket;

use crate::error::{Error, Result};
use crate::session::Session;

/// Sends `service` on `request_path` with optional `data` over the connection
/// `forward_open_response` opened and returns the accepted reply (see [`check_connected_reply`]).
/// Fails with [`Error::Timeout`] when no reply arrives within
/// [`REPLY_TIMEOUT`](crate::session::REPLY_TIMEOUT).
pub async fn send_request(
    session: &mut Session,
    forward_open_response: &ForwardOpenResponse,
    cip_sequence_count: CipUint,
    request_path: CipPath,
    service: ServiceCode,
    data: Option<Box<dyn CipData>>,
) -> Result<EnIpPacket> {
    session
        .send(&EnIpPacket::new_send_unit_data(
            session.session_handle(),
            forward_open_response.o2t_network_connection_id,
            cip_sequence_count,
            MessageRouterRequest::new_data(service, request_path, data),
        ))
        .await?;
    let reply: EnIpPacket = session.read_reply().await?;
    check_connected_reply(forward_open_response, cip_sequence_count, service, &reply)?;
    Ok(reply)
}

/// Whether `reply` answers the request: a Send Unit Data packet on the connection's T->O
/// connection ID that repeats `cip_sequence_count`, answers `service` and is not a rejection
fn check_connected_reply(
    forward_open_response: &ForwardOpenResponse,
    cip_sequence_count: CipUint,
    service: ServiceCode,
    reply: &EnIpPacket,
) -> Result<()> {
    let unit_data: &UnitPacketData =
        reply
            .command_specific_data
            .as_send_unit_data()
            .ok_or_else(|| {
                Error::UnexpectedReply(format!(
                    "a {:?} packet answered a Send Unit Data request",
                    reply.header.command
                ))
            })?;

    let t2o_network_connection_id = forward_open_response.t2o_network_connection_id;
    if unit_data.connection_id != t2o_network_connection_id {
        return Err(Error::UnexpectedReply(format!(
            "the reply is on connection {:#010x}, not {t2o_network_connection_id:#010x}",
            unit_data.connection_id
        )));
    }
    if unit_data.cip_sequence_count != cip_sequence_count {
        return Err(Error::UnexpectedReply(format!(
            "the reply repeats sequence count {}, not {cip_sequence_count}",
            unit_data.cip_sequence_count
        )));
    }

    let response: &MessageRouterResponse = reply.response().ok_or(Error::NoResponse)?;
    let answered = response.service_container.service();
    if answered != service {
        return Err(Error::UnexpectedReply(format!(
            "a {answered:?} reply answered {service:?}"
        )));
    }
    if let Some(rejection) = Rejection::from_response(response) {
        return Err(rejection.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
    use eipscanne_rs::cip::message::data::CipDataOpt;
    use eipscanne_rs::cip::message::response::{
        MessageRouterResponse, ResponseData, ResponseStatusCode,
    };
    use eipscanne_rs::cip::message::shared::ServiceContainer;
    use eipscanne_rs::cip::types::CipUdint;

    use super::*;

    const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xa1b2_c3d4;
    const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;

    /// The reply that opened the connection: the IDs requests and replies travel on
    fn sample_forward_open_response() -> ForwardOpenResponse {
        ForwardOpenResponse {
            o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: 0x0001,
                originator_vendor_id: 342,
                originator_serial_number: 0x0001_2345,
            },
            o2t_actual_packet_interval: 2_000_000,
            t2o_actual_packet_interval: 2_000_000,
            application_reply_size: 0,
            application_reply: vec![],
        }
    }

    /// A Get_Attributes_All reply on `connection_id` repeating `cip_sequence_count`
    fn reply(
        connection_id: CipUdint,
        cip_sequence_count: CipUint,
        status: ResponseStatusCode,
    ) -> EnIpPacket {
        EnIpPacket::new_send_unit_data(
            0x03,
            connection_id,
            cip_sequence_count,
            MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::GetAttributeAll),
                response_data: ResponseData {
                    status,
                    additional_status_size: 0,
                    additional_status: vec![],
                    data: CipDataOpt::Raw(vec![]),
                },
            },
        )
    }

    #[test]
    fn connected_reply_matches_connection_sequence_count_and_service() {
        let forward_open_response = sample_forward_open_response();
        let check = |service: ServiceCode, reply: EnIpPacket| {
            check_connected_reply(&forward_open_response, 7, service, &reply)
        };
        let success = ResponseStatusCode::Success;

        assert!(
            check(
                ServiceCode::GetAttributeAll,
                reply(T2O_NETWORK_CONNECTION_ID, 7, success)
            )
            .is_ok()
        );
        // The O->T ID is where requests go, not where replies come from
        assert!(matches!(
            check(
                ServiceCode::GetAttributeAll,
                reply(O2T_NETWORK_CONNECTION_ID, 7, success)
            ),
            Err(Error::UnexpectedReply(_))
        ));
        // The reply to an earlier request, even one that was refused
        assert!(matches!(
            check(
                ServiceCode::GetAttributeAll,
                reply(
                    T2O_NETWORK_CONNECTION_ID,
                    6,
                    ResponseStatusCode::ServiceNotSupported
                )
            ),
            Err(Error::UnexpectedReply(_))
        ));
        // A reply to another service
        assert!(matches!(
            check(
                ServiceCode::GetAttributeSingle,
                reply(T2O_NETWORK_CONNECTION_ID, 7, success)
            ),
            Err(Error::UnexpectedReply(_))
        ));
        // The right reply, refused
        assert!(matches!(
            check(
                ServiceCode::GetAttributeAll,
                reply(
                    T2O_NETWORK_CONNECTION_ID,
                    7,
                    ResponseStatusCode::ServiceNotSupported
                )
            ),
            Err(Error::Rejected(_))
        ));
    }
}

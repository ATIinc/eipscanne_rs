//! Stage 2: opening the connection with a Forward_Open over the session.

use std::net::{Ipv4Addr, SocketAddrV4};

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerResponse;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::description::CommonPacketItem;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::Error;
use crate::implicit::config::ConnectionConfig;
use crate::session::{Session, router_response};

/// An open connection: what was asked for, what the adapter answered, and where the outputs go.
/// The producer, the consumer and the Forward_Close take what they need from it.
#[derive(Debug)]
pub struct OpenConnection {
    pub config: ConnectionConfig,
    pub request: ForwardOpenRequest,
    pub response: ForwardOpenResponse,
    /// The adapter's IP address: where the inputs come from
    pub target_ip: Ipv4Addr,
    /// Where the outputs are sent
    pub o2t_endpoint: SocketAddrV4,
}

/// Sends the Forward_Open (or Large_Forward_Open) for `config` and reads the adapter's reply
pub async fn forward_open(
    session: &mut Session,
    config: ConnectionConfig,
) -> Result<OpenConnection, Error> {
    let request = config.to_forward_open_request()?;

    session
        .send(&RequestObjectAssembly::new_forward_open(
            session.session_handle(),
            request.clone(),
        ))
        .await?;
    let reply = session.read_reply().await?;

    let router_response = router_response(&reply)?;
    let response = ConnectionManagerResponse::from_message_router_response(router_response)
        .map_err(|error| Error::UnexpectedReply(error.to_string()))?;

    let response = match response {
        ConnectionManagerResponse::ForwardOpen(response) => response,
        ConnectionManagerResponse::Unsuccessful(_) => {
            let service = if config.large_forward_open {
                ServiceCode::LargeForwardOpen
            } else {
                ServiceCode::ForwardOpen
            };
            return Err(Error::rejected(service, router_response));
        }
        ConnectionManagerResponse::ForwardClose(_) => {
            return Err(Error::UnexpectedReply(
                "a Forward_Close reply answered the Forward_Open".to_string(),
            ));
        }
    };

    let target_ip = session.peer_ip();
    Ok(OpenConnection {
        o2t_endpoint: o2t_endpoint(&reply, target_ip),
        config,
        request,
        response,
        target_ip,
    })
}

/// Where the outputs go: the address of the reply's Socket Address Info O->T item when there is
/// one, with the unspecified address `0.0.0.0` standing for the adapter's own address; the
/// adapter's address on the I/O port otherwise
fn o2t_endpoint(reply: &EnIpPacket, target_ip: Ipv4Addr) -> SocketAddrV4 {
    let o2t_sockaddr_info = reply.sockaddr_info_items().find_map(|item| match item {
        CommonPacketItem::O2TSockAddrInfo(info) => Some(info.socket_address()),
        _ => None,
    });

    match o2t_sockaddr_info {
        Some(address) if address.ip().is_unspecified() => {
            SocketAddrV4::new(target_ip, address.port())
        }
        Some(address) => address,
        None => SocketAddrV4::new(target_ip, ETHERNET_IP_IO_UDP_PORT),
    }
}

#[cfg(test)]
mod tests {
    use eipscanne_rs::cip::types::CipUdint;
    use eipscanne_rs::eip::command::{CommandSpecificData, RRPacketData};
    use eipscanne_rs::eip::constants::NO_ENCAPSULATION_TIMEOUT;
    use eipscanne_rs::eip::sockaddr::SockaddrInfo;

    use super::*;

    const SESSION_HANDLE: CipUdint = 0x03;
    const TARGET_IP: Ipv4Addr = Ipv4Addr::new(172, 28, 0, 10);

    /// A SendRRData reply (the message itself does not matter here) with the given Sockaddr Info
    /// items appended
    fn reply_with_items(items: Vec<CommonPacketItem>) -> EnIpPacket {
        let mut reply = EnIpPacket::new_send_rr_data(
            SESSION_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            eipscanne_rs::cip::message::request::MessageRouterRequest::new(
                eipscanne_rs::cip::message::shared::ServiceCode::ForwardOpen,
                eipscanne_rs::cip::path::CipPath::new(0x06, 0x01),
            ),
        );
        if let CommandSpecificData::SendRrData(RRPacketData {
            items: existing, ..
        }) = &mut reply.command_specific_data
        {
            existing.extend(items);
        }
        reply
    }

    #[test]
    fn without_sockaddr_info_the_outputs_go_to_the_target_on_the_io_port() {
        let reply = reply_with_items(vec![]);

        assert_eq!(
            o2t_endpoint(&reply, TARGET_IP),
            SocketAddrV4::new(TARGET_IP, 2222)
        );
    }

    #[test]
    fn an_unspecified_sockaddr_info_address_means_the_target() {
        let reply = reply_with_items(vec![CommonPacketItem::O2TSockAddrInfo(SockaddrInfo::from(
            SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 2223),
        ))]);

        assert_eq!(
            o2t_endpoint(&reply, TARGET_IP),
            SocketAddrV4::new(TARGET_IP, 2223)
        );
    }

    #[test]
    fn a_sockaddr_info_address_is_used_as_given() {
        let other = SocketAddrV4::new(Ipv4Addr::new(172, 28, 0, 20), 2222);
        let reply = reply_with_items(vec![
            // The T->O item describes the adapter's sending side and is not where outputs go
            CommonPacketItem::T2OSockAddrInfo(SockaddrInfo::from(SocketAddrV4::new(
                Ipv4Addr::new(172, 28, 0, 30),
                2222,
            ))),
            CommonPacketItem::O2TSockAddrInfo(SockaddrInfo::from(other)),
        ]);

        assert_eq!(o2t_endpoint(&reply, TARGET_IP), other);
    }
}

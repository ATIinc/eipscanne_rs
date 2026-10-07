//! Stage 2: opening the connection with a Forward_Open over the session.

use std::net::{Ipv4Addr, SocketAddrV4};

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerResponse;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::description::CommonPacketItem;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::Error;
use crate::session::{Session, router_response};

/// An open connection: the Forward_Open that was sent, the reply the adapter sent back, and the
/// real-time format of each direction, the one thing both ends agree on without the wire. Every
/// other value the producer, the consumer and the Forward_Close need is read from these, never
/// copied.
#[derive(Debug)]
pub struct OpenConnection {
    pub request: ForwardOpenRequest,
    pub response: ForwardOpenResponse,
    /// How the outputs signal run/idle (an EDS file or the device manual says which one)
    pub o2t_real_time_format: RealTimeFormat,
    /// How the inputs signal run/idle
    pub t2o_real_time_format: RealTimeFormat,
    /// The adapter's IP address: where the inputs come from
    pub target_ip: Ipv4Addr,
    /// Where the outputs are sent
    pub o2t_endpoint: SocketAddrV4,
}

/// Sends `request` (a Forward_Open or Large_Forward_Open, by the width of its connection
/// parameters) and reads the adapter's reply. The real-time formats are not part of the request;
/// they are kept with the connection so its packets can be framed and read.
pub async fn forward_open(
    session: &mut Session,
    request: ForwardOpenRequest,
    o2t_real_time_format: RealTimeFormat,
    t2o_real_time_format: RealTimeFormat,
) -> Result<OpenConnection, Error> {
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
            return Err(Error::rejected(request.service_code(), router_response));
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
        request,
        response,
        o2t_real_time_format,
        t2o_real_time_format,
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
    use binrw::BinWrite;
    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::types::{CipByte, CipUdint};
    use eipscanne_rs::eip::command::{CommandSpecificData, RRPacketData};
    use eipscanne_rs::eip::constants::NO_ENCAPSULATION_TIMEOUT;
    use eipscanne_rs::eip::sockaddr::SockaddrInfo;

    use crate::test_support::sample_request;

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

    #[test]
    fn sample_request_is_the_forward_open_of_the_captures() {
        // The same bytes as the Forward_Open request test of the library, where Wireshark's
        // dissection of them is documented; the producer and consumer tests build on this request
        let expected_byte_array: Vec<CipByte> = vec![
            0x6f, 0x00, 0x42, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x32, 0x00, 0x54, 0x02,
            0x20, 0x06, 0x24, 0x01, 0x0a, 0x05, 0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
            0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x42,
            0x0f, 0x00, 0x26, 0x48, 0x40, 0x42, 0x0f, 0x00, 0x22, 0x48, 0x01, 0x04, 0x20, 0x04,
            0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64,
        ];

        let packet = RequestObjectAssembly::new_forward_open(SESSION_HANDLE, sample_request());
        let mut bytes = std::io::Cursor::new(Vec::new());
        packet.write(&mut bytes).unwrap();
        let bytes = bytes.into_inner();

        assert_eq_hex!(expected_byte_array, bytes);
    }
}

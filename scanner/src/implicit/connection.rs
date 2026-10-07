//! The connection itself, over the session: the Forward_Open that opens it (stage 2), the
//! Forward_Close that closes it (stage 4), and what both directions read from it.

use std::net::{Ipv4Addr, SocketAddrV4};

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, NetworkConnectionParameters, RealTimeFormat, TransportClass,
    connection_size,
};
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::description::CommonPacketItem;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::error::Result;
use crate::explicit::decode_reply;
use crate::session::Session;

/// An open connection: the Forward_Open that was sent, the reply the adapter sent back, and the
/// real-time format of each direction, the one thing both ends agree on without the wire. Every
/// other value the two directions and the Forward_Close need is read from these, never copied.
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
) -> Result<OpenConnection> {
    let reply = session
        .request(&RequestObjectAssembly::new_forward_open(
            session.session_handle(),
            request.clone(),
        ))
        .await?;
    let response: ForwardOpenResponse = decode_reply(&reply)?;

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

/// Sends the Forward_Close that matches the Forward_Open of `connection` and returns the
/// adapter's reply. The adapter drops the connection on its own once it times out, so a failed
/// close is not fatal.
pub async fn forward_close(
    session: &mut Session,
    connection: &OpenConnection,
) -> Result<ForwardCloseResponse> {
    // A Forward_Close names the connection by the triad and path of the Forward_Open that
    // opened it, with the same timing for the unconnected request itself
    let request = ForwardCloseRequest {
        priority_time_tick: connection.request.priority_time_tick,
        timeout_ticks: connection.request.timeout_ticks,
        connection_triad: connection.request.connection_triad,
        connection_path: connection.request.connection_path.clone(),
    };

    let reply = session
        .request(&RequestObjectAssembly::new_forward_close(
            session.session_handle(),
            request,
        ))
        .await?;
    decode_reply(&reply)
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

/// Bytes of application data a direction carries per packet: its connection size without the
/// sequence count and the real-time header, and whether every packet carries exactly that many
/// bytes or at most that many. A connection size that does not even cover the overhead leaves
/// no room for data.
pub(crate) fn data_size(
    parameters: &NetworkConnectionParameters,
    transport_class: TransportClass,
    real_time_format: RealTimeFormat,
) -> (u16, ConnectionSizeType) {
    let (size, size_type) = match parameters {
        NetworkConnectionParameters::Standard(parameters) => (
            parameters.connection_size().value(),
            parameters.connection_size_type(),
        ),
        NetworkConnectionParameters::Large(parameters) => (
            parameters.connection_size(),
            parameters.connection_size_type(),
        ),
    };
    let overhead = connection_size(0, transport_class, real_time_format);
    (size.saturating_sub(overhead), size_type)
}

/// Whether `data_len` bytes of application data match a direction of `data_size` bytes: exactly
/// that many for a fixed connection size, at most that many for a variable one
pub(crate) fn data_len_matches_connection(
    data_len: usize,
    data_size: u16,
    size_type: ConnectionSizeType,
) -> bool {
    match size_type {
        ConnectionSizeType::Fixed => data_len == usize::from(data_size),
        ConnectionSizeType::Variable => data_len <= usize::from(data_size),
    }
}

/// One open connection to build the O->T and T->O tests on
#[cfg(test)]
pub(crate) mod test_support {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use bilge::prelude::{u4, u9};

    use eipscanne_rs::cip::connection_manager::forward_open::{
        ForwardOpenRequest, ForwardOpenResponse,
    };
    use eipscanne_rs::cip::connection_manager::parameters::{
        ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType,
        Direction, NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger,
        RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters, TransportClass,
        TransportTypeTrigger,
    };
    use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
    use eipscanne_rs::cip::path::CipPath;
    use eipscanne_rs::cip::types::CipUdint;
    use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;

    use super::OpenConnection;

    pub const TARGET_IP: Ipv4Addr = Ipv4Addr::new(172, 28, 0, 10);
    /// The connection IDs of the library's I/O packet tests
    pub const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xa1b2_c3d4;
    pub const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;

    /// The 16-bit parameter word of a point-to-point, exclusive-owner direction
    pub fn standard_parameters(
        connection_size: u16,
        connection_size_type: ConnectionSizeType,
    ) -> NetworkConnectionParameters {
        NetworkConnectionParameters::Standard(
            StandardNetworkConnectionParameters::builder()
                .connection_size(u9::new(connection_size))
                .connection_size_type(connection_size_type)
                .priority(ConnectionPriority::Scheduled)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        )
    }

    /// The Forward_Open of the library's Forward_Open test: configuration assembly 151, 32
    /// output bytes to assembly 150 behind a sequence count and a run/idle header (38), 32 input
    /// bytes from assembly 100 behind a sequence count (34), both every second, x4 timeout
    pub fn sample_request() -> ForwardOpenRequest {
        ForwardOpenRequest {
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            o2t_network_connection_id: 0,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: 0x0001,
                originator_vendor_id: 342,
                originator_serial_number: 0x0001_2345,
            },
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            o2t_requested_packet_interval: 1_000_000,
            o2t_network_connection_parameters: standard_parameters(38, ConnectionSizeType::Fixed),
            t2o_requested_packet_interval: 1_000_000,
            t2o_network_connection_parameters: standard_parameters(34, ConnectionSizeType::Fixed),
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(TransportClass::Class1)
                .production_trigger(ProductionTrigger::Cyclic)
                .direction(Direction::Client)
                .build(),
            connection_path: CipPath::new_assembly_connection(151, 150, 100),
        }
    }

    /// The connection `sample_request` opens, with the run/idle header on the outputs only
    pub fn sample_connection() -> OpenConnection {
        let request = sample_request();
        let response = ForwardOpenResponse {
            o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: request.connection_triad,
            o2t_actual_packet_interval: 1_000_000,
            t2o_actual_packet_interval: 1_000_000,
            application_reply_size: 0,
            application_reply: vec![],
        };

        OpenConnection {
            request,
            response,
            o2t_real_time_format: RealTimeFormat::Header32Bit,
            t2o_real_time_format: RealTimeFormat::Modeless,
            target_ip: TARGET_IP,
            o2t_endpoint: SocketAddrV4::new(TARGET_IP, ETHERNET_IP_IO_UDP_PORT),
        }
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

    use test_support::sample_request;

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
        // dissection of them is documented; the O->T and T->O tests build on this request
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

//! Opening the connection (stage 2) and what both directions read from it.

use std::net::{Ipv4Addr, SocketAddrV4};

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, NetworkConnectionParameters, RealTimeFormat, TransportClass,
    connection_size,
};
use eipscanne_rs::eip::command::RRPacketData;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::packet::EnIpPacket;

use crate::connection_manager;
use crate::error::Result;
use crate::session::Session;

/// An open connection: the Forward_Open sent, the adapter's reply, and each direction's
/// real-time format (agreed off the wire). Everything else is read from these, never copied.
///
/// Connection IDs and packet intervals come from `response`; the request's are only proposals.
#[derive(Debug)]
pub struct OpenConnection {
    pub request: ForwardOpenRequest,
    pub response: ForwardOpenResponse,
    /// How the outputs signal run/idle (from the EDS file or device manual)
    pub o2t_real_time_format: RealTimeFormat,
    /// How the inputs signal run/idle
    pub t2o_real_time_format: RealTimeFormat,
    /// The adapter's IP address: where the inputs come from
    pub target_ip: Ipv4Addr,
    /// Where the outputs are sent
    pub o2t_endpoint: SocketAddrV4,
}

/// Sends `request` (a Forward_Open or Large_Forward_Open, by the width of its connection
/// parameters) and reads the adapter's reply.
pub async fn forward_open(
    session: &mut Session,
    request: ForwardOpenRequest,
    o2t_real_time_format: RealTimeFormat,
    t2o_real_time_format: RealTimeFormat,
) -> Result<OpenConnection> {
    let (response, reply): (ForwardOpenResponse, EnIpPacket) =
        connection_manager::forward_open(session, &request).await?;

    let target_ip: Ipv4Addr = session.peer_ip();
    Ok(OpenConnection {
        o2t_endpoint: o2t_endpoint(&reply, target_ip),
        request,
        response,
        o2t_real_time_format,
        t2o_real_time_format,
        target_ip,
    })
}

/// Where the outputs go: the reply's O->T Socket Address Info address (`0.0.0.0` meaning the
/// adapter), or the adapter on the I/O port
fn o2t_endpoint(reply: &EnIpPacket, target_ip: Ipv4Addr) -> SocketAddrV4 {
    let rr_data: Option<&RRPacketData> = reply.command_specific_data.as_send_rr_data();
    // The adapter adds an O->T Socket Address Info item only to redirect the outputs
    let o2t_address: Option<SocketAddrV4> = rr_data
        .and_then(|rr_data| rr_data.socket_addr_info_items.o2t)
        .map(|info| info.socket_address());

    match o2t_address {
        Some(address) if address.ip().is_unspecified() => {
            SocketAddrV4::new(target_ip, address.port())
        }
        Some(address) => address,
        None => SocketAddrV4::new(target_ip, ETHERNET_IP_IO_UDP_PORT),
    }
}

/// Application data bytes per packet of a direction (its connection size minus the sequence
/// count and real-time header, at least 0) and whether that size is fixed or variable
pub(crate) fn data_size(
    parameters: &NetworkConnectionParameters,
    transport_class: TransportClass,
    real_time_format: RealTimeFormat,
) -> (u16, ConnectionSizeType) {
    let (size, size_type): (u16, ConnectionSizeType) = match parameters {
        NetworkConnectionParameters::Standard(parameters) => (
            parameters.connection_size().value(),
            parameters.connection_size_type(),
        ),
        NetworkConnectionParameters::Large(parameters) => (
            parameters.connection_size(),
            parameters.connection_size_type(),
        ),
    };
    let overhead: u16 = connection_size(0, transport_class, real_time_format);
    (size.saturating_sub(overhead), size_type)
}

/// Whether `data_len` bytes fit a direction of `data_size` bytes: exactly for a fixed size, at
/// most for a variable one. A fixed-size direction therefore rejects the empty packet that
/// signals idle in the `ZeroLength` real-time format, so the scanner neither sends nor accepts it.
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

    /// The library's Forward_Open test: assemblies 151/150/100, 32 bytes each way (38 and 34 with
    /// overhead), every second, x4 timeout
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
    use eipscanne_rs::eip::command::{CommandSpecificData, RRPacketData};
    use eipscanne_rs::eip::constants::NO_ENCAPSULATION_TIMEOUT;
    use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

    use test_support::TARGET_IP;

    use super::*;

    /// A SendRRData reply with the given Socket Address Info items
    fn reply_with_items(items: SocketAddrInfoItems) -> EnIpPacket {
        let mut reply = EnIpPacket::new_send_rr_data(
            0x03,
            NO_ENCAPSULATION_TIMEOUT,
            eipscanne_rs::cip::message::request::MessageRouterRequest::new(
                eipscanne_rs::cip::message::shared::ServiceCode::ForwardOpen,
                eipscanne_rs::cip::path::CipPath::new(0x06, 0x01),
            ),
        );
        if let CommandSpecificData::SendRrData(RRPacketData {
            socket_addr_info_items,
            ..
        }) = &mut reply.command_specific_data
        {
            *socket_addr_info_items = items;
        }
        reply
    }

    #[test]
    fn o2t_endpoint_follows_the_socket_addr_info() {
        let o2t = |address| Some(SocketAddrInfo::from(address));
        let other = SocketAddrV4::new(Ipv4Addr::new(172, 28, 0, 20), 2222);

        // None: the target on the I/O port
        assert_eq!(
            o2t_endpoint(&reply_with_items(SocketAddrInfoItems::empty()), TARGET_IP),
            SocketAddrV4::new(TARGET_IP, 2222)
        );
        // 0.0.0.0: the target on the given port
        assert_eq!(
            o2t_endpoint(
                &reply_with_items(SocketAddrInfoItems {
                    o2t: o2t(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 2223)),
                    t2o: None,
                }),
                TARGET_IP
            ),
            SocketAddrV4::new(TARGET_IP, 2223)
        );
        // Any other address as given; the T->O item is not where outputs go
        let t2o = Some(SocketAddrInfo::from(SocketAddrV4::new(
            Ipv4Addr::new(172, 28, 0, 30),
            2222,
        )));
        assert_eq!(
            o2t_endpoint(
                &reply_with_items(SocketAddrInfoItems {
                    o2t: o2t(other),
                    t2o
                }),
                TARGET_IP
            ),
            other
        );
    }
}

//! Stage 3, T->O: the inputs, always as bytes. The caller passes every packet to
//! [`accept_t2o_packet`] and keeps the deadline: [`input_timeout`] after the last accepted packet,
//! or [`FIRST_PACKET_GRACE`] if longer before the first. Nothing here keeps state.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use binrw::BinRead;
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::parameters::connection_size;
use eipscanne_rs::cip::io_data::IoData;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::io_packet::{IoPacket, SequencedAddress};

use crate::error::{Error, Result};
use crate::implicit::connection::{OpenConnection, data_len_matches_connection, data_size};

/// The largest UDP payload, so no packet is cut short
const MAX_UDP_PAYLOAD: usize = 65_535;

/// Binds the I/O socket on every interface, before the Forward_Open: adapters send as soon as
/// they reply.
pub async fn bind_io_socket() -> Result<UdpSocket> {
    Ok(UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        ETHERNET_IP_IO_UDP_PORT,
    ))
    .await?)
}

/// Receives one I/O packet and its sender. Cancel safe.
pub async fn recv_io_packet(socket: &UdpSocket) -> Result<(IoPacket, SocketAddr)> {
    let mut bytes = vec![0u8; MAX_UDP_PAYLOAD];
    let (len, from) = socket.recv_from(&mut bytes).await?;
    bytes.truncate(len);

    let packet = IoPacket::read(&mut Cursor::new(&bytes))?;
    Ok((packet, from))
}

/// The least time the adapter gets for its first packet
pub const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// The smallest allowed sequence number gap
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Screens a packet of `connection` from `from`, given the last accepted encapsulation sequence
/// number (`None` before the first). Returns its Sequenced Address and Connected Data Item, or
/// `Error::UnexpectedPacket` saying why it was discarded (`Error::Parse` if the data does not
/// decode).
pub fn accept_t2o_packet(
    connection: &OpenConnection,
    last_sequence_number: Option<CipUdint>,
    packet: &IoPacket,
    from: SocketAddr,
) -> Result<(SequencedAddress, IoData)> {
    // 1. It is for this connection
    let Some(address) = packet.sequenced_address().copied() else {
        return Err(Error::UnexpectedPacket(
            "packet without a Sequenced Address Item".to_string(),
        ));
    };
    if address.connection_id != connection.response.t2o_network_connection_id {
        return Err(Error::UnexpectedPacket(format!(
            "packet for another connection ({:#010x})",
            address.connection_id
        )));
    }

    // 2. It comes from the adapter (whatever port it chose)
    if from.ip() != IpAddr::V4(connection.target_ip) {
        return Err(Error::UnexpectedPacket(format!(
            "packet from {from}, not the adapter"
        )));
    }

    // 3. It is newer than the last accepted one (distance modulo 2^32 in 1..2^31), by at most
    //    the timeout multiplier plus one (never under 16). Any first number is accepted
    if let Some(last) = last_sequence_number {
        let received = address.encapsulation_sequence_number;
        let distance = received.wrapping_sub(last);
        if distance == 0 || distance >= 1 << 31 {
            return Err(Error::UnexpectedPacket(format!(
                "stale sequence number {received} (last accepted {last})"
            )));
        }
        let multiplier = connection
            .request
            .connection_timeout_multiplier
            .multiplier();
        let allowed = MIN_ALLOWED_SEQUENCE_GAP.max(multiplier.saturating_add(1));
        if distance > allowed {
            return Err(Error::UnexpectedPacket(format!(
                "sequence number {received} is more than {allowed} ahead of the last accepted {last}"
            )));
        }
    }

    // 4. Its data has the agreed size and decodes in this direction's shape
    let Some(CipDataOpt::Raw(bytes)) = packet.connected_data() else {
        return Err(Error::UnexpectedPacket(
            "packet without a Connected Data Item".to_string(),
        ));
    };
    let transport_class = connection.request.transport_type_trigger.transport_class();
    let real_time_format = connection.t2o_real_time_format;
    let (expected, size_type) = data_size(
        &connection.request.t2o_network_connection_parameters,
        transport_class,
        real_time_format,
    );
    // The sequence count and header precede the data
    let overhead = usize::from(connection_size(0, transport_class, real_time_format));
    let data_len = bytes.len().saturating_sub(overhead);
    if bytes.len() < overhead || !data_len_matches_connection(data_len, expected, size_type) {
        return Err(Error::UnexpectedPacket(format!(
            "{data_len} input bytes, the connection carries {expected}"
        )));
    }
    // A Connected Data Item's length is a 16-bit field on the wire
    let inputs = IoData::read_le_args(
        &mut Cursor::new(bytes),
        (bytes.len() as u16, transport_class, real_time_format),
    )?;

    Ok((address, inputs))
}

/// How long the inputs may stop before the connection times out: the timeout multiplier times
/// the T->O actual packet interval
pub fn input_timeout(connection: &OpenConnection) -> Duration {
    let multiplier = connection
        .request
        .connection_timeout_multiplier
        .multiplier();
    // x512 on a 10 s interval overflows 32-bit microseconds
    Duration::from_micros(
        u64::from(multiplier) * u64::from(connection.response.t2o_actual_packet_interval),
    )
}

#[cfg(test)]
mod tests {
    use eipscanne_rs::cip::connection_manager::parameters::{
        ConnectionSizeType, ConnectionTimeoutMultiplier, RealTimeFormat,
    };
    use eipscanne_rs::cip::io_data::RunIdleHeader;
    use eipscanne_rs::cip::types::CipUint;
    use eipscanne_rs::eip::description::CommonPacketItem;

    use crate::implicit::connection::test_support::{
        T2O_NETWORK_CONNECTION_ID, TARGET_IP, sample_connection, standard_parameters,
    };

    use super::*;

    /// Where the adapter sends from (the port is its choice)
    fn from_adapter() -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(TARGET_IP, 2222))
    }

    /// An input packet of the sample connection, as the wire delivers it: the data raw
    fn input_packet(sequence_number: CipUdint, count: CipUint, data: &[u8]) -> IoPacket {
        let mut bytes = count.to_le_bytes().to_vec();
        bytes.extend_from_slice(data);
        IoPacket::new(
            T2O_NETWORK_CONNECTION_ID,
            sequence_number,
            CipDataOpt::Raw(bytes),
        )
    }

    /// Accepts a 32-byte input packet numbered `received` after one numbered `last`
    fn accept_after(
        connection: &OpenConnection,
        last: CipUdint,
        received: CipUdint,
    ) -> Result<(SequencedAddress, IoData)> {
        accept_t2o_packet(
            connection,
            Some(last),
            &input_packet(received, 1, &[0; 32]),
            from_adapter(),
        )
    }

    /// Why a packet numbered `received` after one numbered `last` is discarded
    fn discarded_after(connection: &OpenConnection, last: CipUdint, received: CipUdint) -> String {
        accept_after(connection, last, received)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn inputs_are_read_in_the_shape_of_the_connection() {
        // Modeless and fixed: any first sequence number, any adapter port
        let data: Vec<u8> = (0..32).rev().collect();
        let any_port = SocketAddr::V4(SocketAddrV4::new(TARGET_IP, 49152));
        let (address, inputs) = accept_t2o_packet(
            &sample_connection(),
            None,
            &input_packet(0xdead_beef, 7, &data),
            any_port,
        )
        .unwrap();
        assert_eq!(
            address,
            SequencedAddress {
                connection_id: T2O_NETWORK_CONNECTION_ID,
                encapsulation_sequence_number: 0xdead_beef,
            }
        );
        assert_eq!(
            inputs,
            IoData {
                cip_sequence_count: Some(7),
                run_idle_header: None,
                data: CipDataOpt::Raw(data),
            }
        );

        // Variable: the data may be shorter
        let mut variable = sample_connection();
        variable.request.t2o_network_connection_parameters =
            standard_parameters(34, ConnectionSizeType::Variable);
        let (_, inputs) = accept_t2o_packet(
            &variable,
            None,
            &input_packet(1, 1, &[7; 3]),
            from_adapter(),
        )
        .unwrap();
        assert_eq!(inputs.data, CipDataOpt::Raw(vec![7; 3]));

        // A run/idle header: the same 32 bytes of data behind it
        let mut header = sample_connection();
        header.t2o_real_time_format = RealTimeFormat::Header32Bit;
        header.request.t2o_network_connection_parameters =
            standard_parameters(38, ConnectionSizeType::Fixed);
        let idle_header = RunIdleHeader::builder()
            .run_idle(false)
            .claim_output_ownership(false)
            .ready_for_ownership_of_outputs(bilge::prelude::u2::new(0))
            .build();
        let mut bytes = u32::from(idle_header).to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0; 32]);
        let (_, inputs) =
            accept_t2o_packet(&header, None, &input_packet(1, 1, &bytes), from_adapter()).unwrap();
        assert_eq!(
            inputs,
            IoData {
                cip_sequence_count: Some(1),
                run_idle_header: Some(idle_header),
                data: CipDataOpt::Raw(vec![0; 32]),
            }
        );
    }

    #[test]
    fn foreign_and_malformed_packets_are_discarded() {
        let stranger = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(172, 28, 0, 99), 2222));
        let without_address = IoPacket {
            items: vec![CommonPacketItem::ConnectedDataItem(CipDataOpt::Raw(vec![
                0;
                34
            ]))],
        };
        let without_data = IoPacket {
            items: vec![CommonPacketItem::SequencedAddressItem(SequencedAddress {
                connection_id: T2O_NETWORK_CONNECTION_ID,
                encapsulation_sequence_number: 1,
            })],
        };
        let cases = [
            (
                IoPacket::new(
                    T2O_NETWORK_CONNECTION_ID + 1,
                    1,
                    CipDataOpt::Raw(vec![0; 34]),
                ),
                from_adapter(),
                "packet for another connection (0x12345679)",
            ),
            (
                input_packet(1, 1, &[0; 32]),
                stranger,
                "packet from 172.28.0.99:2222, not the adapter",
            ),
            (
                without_address,
                from_adapter(),
                "packet without a Sequenced Address Item",
            ),
            (
                without_data,
                from_adapter(),
                "packet without a Connected Data Item",
            ),
            (
                input_packet(1, 1, &[0; 31]),
                from_adapter(),
                "31 input bytes, the connection carries 32",
            ),
            (
                input_packet(1, 1, &[0; 33]),
                from_adapter(),
                "33 input bytes, the connection carries 32",
            ),
            // Not even a sequence count
            (
                IoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(vec![0])),
                from_adapter(),
                "0 input bytes, the connection carries 32",
            ),
        ];

        for (packet, from, reason) in cases {
            assert_eq!(
                accept_t2o_packet(&sample_connection(), None, &packet, from)
                    .unwrap_err()
                    .to_string(),
                format!("unexpected packet: {reason}")
            );
        }
    }

    #[test]
    fn sequence_numbers_move_forward_within_the_allowed_gap() {
        let connection = sample_connection();

        // Repeated, older, or back across the rollover: stale
        for (last, received) in [(10, 10), (10, 9), (1, CipUdint::MAX)] {
            assert_eq!(
                discarded_after(&connection, last, received),
                format!(
                    "unexpected packet: stale sequence number {received} (last accepted {last})"
                )
            );
        }
        assert!(accept_after(&connection, CipUdint::MAX, 0).is_ok());

        // x4 is below the floor of 16
        assert!(accept_after(&connection, 100, 116).is_ok());
        assert_eq!(
            discarded_after(&connection, 116, 133),
            "unexpected packet: sequence number 133 is more than 16 ahead of the last accepted 116"
        );

        // x512 allows the multiplier plus one
        let mut large = sample_connection();
        large.request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
        assert!(accept_after(&large, 1000, 1513).is_ok());
        assert_eq!(
            discarded_after(&large, 1513, 2027),
            "unexpected packet: sequence number 2027 is more than 513 ahead of the last accepted 1513"
        );
    }

    #[test]
    fn timeout_is_the_multiplier_times_the_interval() {
        // x4 on a 1 s interval
        let mut connection = sample_connection();
        assert_eq!(input_timeout(&connection), Duration::from_secs(4));

        // The largest multiplier and interval do not overflow
        connection.request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
        connection.response.t2o_actual_packet_interval = 10_000_000;
        assert_eq!(input_timeout(&connection), Duration::from_secs(5120));
    }
}

//! Stage 3, T->O: the inputs. The caller hands every packet that arrives on the I/O socket to
//! [`accept_t2o_packet`] with the sequence number of the last accepted one, and gets back the
//! packet's Sequenced Address and Connected Data as they were on the wire, or
//! `Error::UnexpectedPacket` saying why it was discarded, for the caller to report. The caller
//! also keeps the deadline: the connection has timed out when no packet is accepted within
//! [`input_timeout`] (and within [`FIRST_PACKET_GRACE`], if longer, of the Forward_Open reply).
//! Nothing here keeps state.
//!
//! The inputs are always bytes, since only the caller knows their type; a `binrw` input assembly
//! is read from them.

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

/// Binds the I/O socket on every interface. Bind it before the Forward_Open: adapters start
/// sending as soon as they have replied.
pub async fn bind_io_socket() -> Result<UdpSocket> {
    Ok(UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        ETHERNET_IP_IO_UDP_PORT,
    ))
    .await?)
}

/// Waits for one datagram and parses it as an I/O packet, returning it with its sender. Cancel
/// safe: a dropped call loses no datagram.
pub async fn recv_io_packet(socket: &UdpSocket) -> Result<(IoPacket, SocketAddr)> {
    let mut bytes = vec![0u8; MAX_UDP_PAYLOAD];
    let (len, from) = socket.recv_from(&mut bytes).await?;
    bytes.truncate(len);

    let packet = IoPacket::read(&mut Cursor::new(&bytes))?;
    Ok((packet, from))
}

/// Before the first packet the adapter gets at least this long, whatever the timeout
pub const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// A packet may be at least this many numbers ahead of the last accepted one
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Screens a packet of `connection` received from `from`, given the encapsulation sequence
/// number of the last accepted packet (`None` before the first). An accepted packet returns its
/// Sequenced Address and its Connected Data Item read with the shape of the T->O direction; a
/// discarded one says why.
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

    // 3. It is newer than the last accepted packet, but not unreasonably so: the distance
    //    modulo 2^32 is at least 1 and at most the timeout multiplier plus one (never under 16).
    //    A distance of 0 or of 2^31 and over means an older packet (or a repeat). The first
    //    packet is accepted with any number
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

    // 4. Its data has the agreed size and decodes with the shape of this direction: sequence
    //    count, run/idle header, then the data
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
    // The sequence count and header come before the data; a shorter item has no data to speak
    // of
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

/// How long the inputs of `connection` may stop before it times out: the timeout multiplier
/// times the T->O actual packet interval
pub fn input_timeout(connection: &OpenConnection) -> Duration {
    let multiplier = connection
        .request
        .connection_timeout_multiplier
        .multiplier();
    // In 64 bits: a x512 multiplier on a 10 s interval overflows 32-bit microseconds
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

    /// Why `packet` is discarded as the first packet of `connection`
    fn discarded(connection: &OpenConnection, packet: &IoPacket, from: SocketAddr) -> String {
        accept_t2o_packet(connection, None, packet, from)
            .unwrap_err()
            .to_string()
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
    fn first_packet_is_accepted_with_any_sequence_number() {
        let data: Vec<u8> = (0..32).rev().collect();

        let (address, inputs) = accept_t2o_packet(
            &sample_connection(),
            None,
            &input_packet(0xdead_beef, 7, &data),
            from_adapter(),
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
    }

    #[test]
    fn packets_for_another_connection_are_discarded() {
        let packet = IoPacket::new(
            T2O_NETWORK_CONNECTION_ID + 1,
            1,
            CipDataOpt::Raw(vec![0; 34]),
        );

        assert_eq!(
            discarded(&sample_connection(), &packet, from_adapter()),
            "unexpected packet: packet for another connection (0x12345679)"
        );
    }

    #[test]
    fn packets_from_another_host_are_discarded() {
        let stranger = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(172, 28, 0, 99), 2222));

        assert_eq!(
            discarded(
                &sample_connection(),
                &input_packet(1, 1, &[0; 32]),
                stranger
            ),
            "unexpected packet: packet from 172.28.0.99:2222, not the adapter"
        );
    }

    #[test]
    fn the_adapter_may_send_from_any_port() {
        let from = SocketAddr::V4(SocketAddrV4::new(TARGET_IP, 49152));

        assert!(
            accept_t2o_packet(
                &sample_connection(),
                None,
                &input_packet(1, 1, &[0; 32]),
                from
            )
            .is_ok()
        );
    }

    #[test]
    fn repeated_and_older_sequence_numbers_are_stale() {
        let connection = sample_connection();

        assert_eq!(
            discarded_after(&connection, 10, 10),
            "unexpected packet: stale sequence number 10 (last accepted 10)"
        );
        assert_eq!(
            discarded_after(&connection, 10, 9),
            "unexpected packet: stale sequence number 9 (last accepted 10)"
        );
    }

    #[test]
    fn sequence_numbers_roll_over() {
        let connection = sample_connection();

        assert!(accept_after(&connection, CipUdint::MAX - 1, CipUdint::MAX).is_ok());
        assert!(accept_after(&connection, CipUdint::MAX, 0).is_ok());
        // Back across the rollover is stale
        assert_eq!(
            discarded_after(&connection, 1, CipUdint::MAX),
            format!(
                "unexpected packet: stale sequence number {} (last accepted 1)",
                CipUdint::MAX
            )
        );
    }

    #[test]
    fn small_multiplier_allows_a_gap_of_sixteen() {
        // The sample connection has a x4 multiplier, so the floor of 16 applies
        let connection = sample_connection();

        assert!(accept_after(&connection, 100, 116).is_ok());
        assert_eq!(
            discarded_after(&connection, 116, 133),
            "unexpected packet: sequence number 133 is more than 16 ahead of the last accepted 116"
        );
    }

    #[test]
    fn large_multiplier_allows_a_gap_of_multiplier_plus_one() {
        let mut connection = sample_connection();
        connection.request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;

        assert!(accept_after(&connection, 1000, 1513).is_ok());
        assert_eq!(
            discarded_after(&connection, 1513, 2027),
            "unexpected packet: sequence number 2027 is more than 513 ahead of the last accepted 1513"
        );
    }

    #[test]
    fn data_of_the_wrong_size_is_discarded() {
        let connection = sample_connection();

        assert_eq!(
            discarded(&connection, &input_packet(1, 1, &[0; 31]), from_adapter()),
            "unexpected packet: 31 input bytes, the connection carries 32"
        );
        assert_eq!(
            discarded(&connection, &input_packet(1, 1, &[0; 33]), from_adapter()),
            "unexpected packet: 33 input bytes, the connection carries 32"
        );
        // Not even a sequence count
        assert_eq!(
            discarded(
                &connection,
                &IoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(vec![0])),
                from_adapter()
            ),
            "unexpected packet: 0 input bytes, the connection carries 32"
        );
    }

    #[test]
    fn variable_size_data_may_be_shorter() {
        let mut connection = sample_connection();
        connection.request.t2o_network_connection_parameters =
            standard_parameters(34, ConnectionSizeType::Variable);

        let (_, inputs) = accept_t2o_packet(
            &connection,
            None,
            &input_packet(1, 1, &[7; 3]),
            from_adapter(),
        )
        .unwrap();

        assert_eq!(inputs.data, CipDataOpt::Raw(vec![7; 3]));
    }

    #[test]
    fn packets_without_a_connected_data_item_are_discarded() {
        let packet = IoPacket {
            items: vec![
                eipscanne_rs::eip::description::CommonPacketItem::SequencedAddressItem(
                    SequencedAddress {
                        connection_id: T2O_NETWORK_CONNECTION_ID,
                        encapsulation_sequence_number: 1,
                    },
                ),
            ],
        };

        assert_eq!(
            discarded(&sample_connection(), &packet, from_adapter()),
            "unexpected packet: packet without a Connected Data Item"
        );
    }

    #[test]
    fn header_direction_carries_the_header() {
        let mut connection = sample_connection();
        connection.t2o_real_time_format = RealTimeFormat::Header32Bit;
        // The same 32 bytes of data, now behind a 4-byte header
        connection.request.t2o_network_connection_parameters =
            standard_parameters(38, ConnectionSizeType::Fixed);

        let idle_header = RunIdleHeader::builder()
            .run_idle(false)
            .claim_output_ownership(false)
            .ready_for_ownership_of_outputs(bilge::prelude::u2::new(0))
            .build();
        let mut bytes = 1u16.to_le_bytes().to_vec();
        bytes.extend_from_slice(&u32::from(idle_header).to_le_bytes());
        bytes.extend_from_slice(&[0; 32]);
        let packet = IoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(bytes));

        let (_, inputs) = accept_t2o_packet(&connection, None, &packet, from_adapter()).unwrap();

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
    fn timeout_is_the_multiplier_times_the_interval() {
        // x4 on a 1 s interval
        assert_eq!(input_timeout(&sample_connection()), Duration::from_secs(4));
    }

    #[test]
    fn timeout_does_not_overflow_for_the_largest_multiplier_and_interval() {
        let mut connection = sample_connection();
        connection.request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
        connection.response.t2o_actual_packet_interval = 10_000_000;

        assert_eq!(input_timeout(&connection), Duration::from_secs(5120));
    }
}

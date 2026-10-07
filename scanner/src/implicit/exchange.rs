//! Stage 3: exchanging I/O over UDP port 2222, in two independent directions.
//!
//! Nothing here keeps state: the caller's loop owns the sequence numbers and the deadline, and
//! decides when to send and receive (see the `implicit-io` example). Framing and screening touch
//! no socket; only the three socket functions do.
//!
//! * Outputs (O->T): [`output_packet`] frames the outputs; the caller sends one packet every
//!   O->T actual packet interval (`connection.response.o2t_actual_packet_interval`).
//! * Inputs (T->O): [`accept_input`] screens a received packet and returns its Sequenced Address
//!   and Connected Data as they were on the wire, or the reason it was [`Discarded`]. The
//!   connection has timed out when no packet is accepted within [`input_timeout`] (and within
//!   [`FIRST_PACKET_GRACE`], if longer, of the Forward_Open reply).
//!
//! The outputs are the caller's assembly as bytes or as a `binrw` struct (`CipDataOpt`). The
//! inputs are always bytes, since only the caller knows their type; a `binrw` input assembly is
//! read from them.

use std::fmt;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use binrw::{BinRead, BinResult, BinWrite};
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, NetworkConnectionParameters, RealTimeFormat, TransportClass,
    connection_size,
};
use eipscanne_rs::cip::io_data::{IoData, RunIdleHeader};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::io_packet::{IoPacket, SequencedAddress};

use crate::Error;
use crate::implicit::open::OpenConnection;

// ======= The socket ========

/// The largest UDP payload, so no packet is cut short
const MAX_UDP_PAYLOAD: usize = 65_535;

/// Binds the I/O socket on every interface. Bind it before the Forward_Open: adapters start
/// sending as soon as they have replied.
pub async fn bind_io_socket() -> std::io::Result<UdpSocket> {
    UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        ETHERNET_IP_IO_UDP_PORT,
    ))
    .await
}

/// Sends one I/O packet to `to`
pub async fn send_io_packet(
    socket: &UdpSocket,
    packet: &IoPacket,
    to: SocketAddrV4,
) -> BinResult<()> {
    let mut bytes = Cursor::new(Vec::new());
    packet.write(&mut bytes)?;
    socket.send_to(bytes.get_ref(), to).await?;
    Ok(())
}

/// Waits for one datagram and parses it as an I/O packet, returning it with its sender. Cancel
/// safe: a dropped call loses no datagram.
pub async fn recv_io_packet(socket: &UdpSocket) -> BinResult<(IoPacket, SocketAddr)> {
    let mut bytes = vec![0u8; MAX_UDP_PAYLOAD];
    let (len, from) = socket.recv_from(&mut bytes).await?;
    bytes.truncate(len);

    let packet = IoPacket::read(&mut Cursor::new(&bytes))?;
    Ok((packet, from))
}

// ======= Outputs (O->T) ========

/// The output packet of `connection` carrying `outputs` with the run flag set to `run`. The
/// outputs are the caller's assembly, as bytes (`CipDataOpt::Raw`) or as a `binrw` struct
/// (`CipDataOpt::Typed`); the packet holds them as the bytes that go on the wire.
///
/// The caller numbers the packets: every packet gets the next `encapsulation_sequence_number`
/// (start it at a random number, so a restarted scanner does not repeat the numbers the adapter
/// last saw), and `cip_sequence_count` moves only when the outputs change, so a resend of
/// unchanged outputs tells the adapter nothing new arrived.
pub fn output_packet(
    connection: &OpenConnection,
    encapsulation_sequence_number: CipUdint,
    cip_sequence_count: CipUint,
    outputs: CipDataOpt,
    run: bool,
) -> Result<IoPacket, Error> {
    // A typed assembly's size is only known once it is written
    let mut outputs_bytes = Cursor::new(Vec::new());
    outputs.write_le_args(&mut outputs_bytes, (0,))?;
    let outputs = outputs_bytes.into_inner();

    let transport_class = connection.request.transport_type_trigger.transport_class();
    let (data_size, connection_size_type) = data_size(
        &connection.request.o2t_network_connection_parameters,
        transport_class,
        connection.o2t_real_time_format,
    );
    if !fits(data_size, connection_size_type, outputs.len()) {
        return Err(Error::OutputSize {
            connection_size_type,
            data_size,
            actual: outputs.len(),
        });
    }

    let cip_sequence_count = match transport_class {
        TransportClass::Class1 | TransportClass::Class2 | TransportClass::Class3 => {
            Some(cip_sequence_count)
        }
        TransportClass::Class0 | TransportClass::Unknown(_) => None,
    };
    let run_idle_header = match connection.o2t_real_time_format {
        RealTimeFormat::Header32Bit => Some(
            RunIdleHeader::builder()
                .run_idle(run)
                .claim_output_ownership(false)
                .ready_for_ownership_of_outputs(bilge::prelude::u2::new(0))
                .build(),
        ),
        RealTimeFormat::Modeless | RealTimeFormat::ZeroLength | RealTimeFormat::Heartbeat => None,
    };

    let mut bytes = Cursor::new(Vec::new());
    IoData {
        cip_sequence_count,
        run_idle_header,
        data: CipDataOpt::Raw(outputs),
    }
    .write_le(&mut bytes)?;

    Ok(IoPacket::new(
        connection.response.o2t_network_connection_id,
        encapsulation_sequence_number,
        CipDataOpt::Raw(bytes.into_inner()),
    ))
}

// ======= Inputs (T->O) ========

/// Before the first packet the adapter gets at least this long, whatever the timeout
pub const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// A packet may be at least this many numbers ahead of the last accepted one
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Why a packet was not taken as inputs
#[derive(Debug, Clone, PartialEq)]
pub enum Discarded {
    /// The packet belongs to another connection (or has no Sequenced Address Item)
    UnknownConnection { connection_id: Option<CipUdint> },
    /// The packet did not come from the adapter
    WrongSender(SocketAddr),
    /// The packet is older than, or the same as, the last accepted one
    StaleSequenceNumber { received: CipUdint, last: CipUdint },
    /// The packet is too far ahead of the last accepted one
    SequenceGapTooLarge {
        received: CipUdint,
        last: CipUdint,
        allowed: u32,
    },
    /// The packet has no Connected Data Item or its data does not decode
    Malformed(String),
    /// The application data has the wrong length
    WrongSize { expected: u16, actual: usize },
}

/// Screens a packet of `connection` received from `from`, given the encapsulation sequence
/// number of the last accepted packet (`None` before the first). An accepted packet returns its
/// Sequenced Address and its Connected Data Item read with the shape of the T->O direction.
pub fn accept_input(
    connection: &OpenConnection,
    last_sequence_number: Option<CipUdint>,
    packet: &IoPacket,
    from: SocketAddr,
) -> Result<(SequencedAddress, IoData), Discarded> {
    // 1. It is for this connection
    let address = match packet.sequenced_address() {
        Some(address) if address.connection_id == connection.response.t2o_network_connection_id => {
            *address
        }
        address => {
            return Err(Discarded::UnknownConnection {
                connection_id: address.map(|address| address.connection_id),
            });
        }
    };

    // 2. It comes from the adapter (whatever port it chose)
    if from.ip() != IpAddr::V4(connection.target_ip) {
        return Err(Discarded::WrongSender(from));
    }

    // 3. It is newer than the last accepted packet, but not unreasonably so: the distance
    //    modulo 2^32 is at least 1 and at most the timeout multiplier plus one (never under 16).
    //    A distance of 0 or of 2^31 and over means an older packet (or a repeat). The first
    //    packet is accepted with any number
    if let Some(last) = last_sequence_number {
        let received = address.encapsulation_sequence_number;
        let distance = received.wrapping_sub(last);
        if distance == 0 || distance >= 1 << 31 {
            return Err(Discarded::StaleSequenceNumber { received, last });
        }
        let multiplier = connection
            .request
            .connection_timeout_multiplier
            .multiplier();
        let allowed = MIN_ALLOWED_SEQUENCE_GAP.max(multiplier.saturating_add(1));
        if distance > allowed {
            return Err(Discarded::SequenceGapTooLarge {
                received,
                last,
                allowed,
            });
        }
    }

    // 4. Its data has the agreed size and decodes with the shape of this direction: sequence
    //    count, run/idle header, then the data
    let bytes = match packet.connected_data() {
        Some(CipDataOpt::Raw(bytes)) => bytes,
        Some(CipDataOpt::Typed(_)) => {
            return Err(Discarded::Malformed(
                "the Connected Data Item is not raw bytes".to_string(),
            ));
        }
        None => return Err(Discarded::Malformed("no Connected Data Item".to_string())),
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
    let byte_len = match u16::try_from(bytes.len()) {
        Ok(byte_len) if bytes.len() >= overhead && fits(expected, size_type, data_len) => byte_len,
        _ => {
            return Err(Discarded::WrongSize {
                expected,
                actual: data_len,
            });
        }
    };
    let inputs = IoData::read_le_args(
        &mut Cursor::new(bytes),
        (byte_len, transport_class, real_time_format),
    )
    .map_err(|error| Discarded::Malformed(error.to_string()))?;

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

// ======= Start of Discarded impl ========

impl fmt::Display for Discarded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Discarded::UnknownConnection {
                connection_id: Some(id),
            } => write!(f, "packet for another connection ({id:#010x})"),
            Discarded::UnknownConnection {
                connection_id: None,
            } => write!(f, "packet without a Sequenced Address Item"),
            Discarded::WrongSender(from) => write!(f, "packet from {from}, not the adapter"),
            Discarded::StaleSequenceNumber { received, last } => {
                write!(f, "stale sequence number {received} (last accepted {last})")
            }
            Discarded::SequenceGapTooLarge {
                received,
                last,
                allowed,
            } => write!(
                f,
                "sequence number {received} is more than {allowed} ahead of the last accepted {last}"
            ),
            Discarded::Malformed(what) => write!(f, "malformed packet: {what}"),
            Discarded::WrongSize { expected, actual } => {
                write!(f, "{actual} input bytes, the connection carries {expected}")
            }
        }
    }
}

impl std::error::Error for Discarded {}

// ^^^^^^^^ End of Discarded impl ^^^^^^^^

// ======= Connection sizes ========

/// The application data a direction carries per packet: its connection size without the
/// sequence count and the real-time header, and whether every packet carries exactly that many
/// bytes or at most that many. A connection size that does not even cover the overhead leaves
/// no room for data.
fn data_size(
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

/// Whether `actual` bytes of application data fit a direction of `data_size` bytes
fn fits(data_size: u16, size_type: ConnectionSizeType, actual: usize) -> bool {
    match size_type {
        ConnectionSizeType::Fixed => actual == usize::from(data_size),
        ConnectionSizeType::Variable => actual <= usize::from(data_size),
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::connection_manager::parameters::ConnectionTimeoutMultiplier;
    use eipscanne_rs::cip::types::CipByte;

    use crate::test_support::{
        O2T_NETWORK_CONNECTION_ID, T2O_NETWORK_CONNECTION_ID, TARGET_IP, sample_connection,
        standard_parameters,
    };

    use super::*;

    // ======= Outputs ========

    /// The bytes a packet serializes to
    fn bytes_of(packet: &IoPacket) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        packet.write(&mut bytes).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn first_packet_matches_the_capture() {
        // The first O->T packet of the library's I/O packet test (sequence number 1, count 1,
        // run, outputs 0x00..0x1f), where Wireshark's dissection of the bytes is documented
        let expected_byte_array: Vec<CipByte> = vec![
            0x02, 0x00, 0x02, 0x80, 0x08, 0x00, 0xd4, 0xc3, 0xb2, 0xa1, 0x01, 0x00, 0x00, 0x00,
            0xb1, 0x00, 0x26, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03,
            0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11,
            0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let outputs: Vec<u8> = (0..32).collect();

        let packet =
            output_packet(&sample_connection(), 1, 1, CipDataOpt::Raw(outputs), true).unwrap();

        assert_eq!(
            packet.sequenced_address().unwrap().connection_id,
            O2T_NETWORK_CONNECTION_ID
        );
        assert_eq_hex!(expected_byte_array, bytes_of(&packet));
    }

    #[test]
    fn run_flag_is_carried_by_the_header() {
        let connection = sample_connection();
        let outputs = || CipDataOpt::Raw(vec![0; 32]);

        let running = bytes_of(&output_packet(&connection, 1, 1, outputs(), true).unwrap());
        let idle = bytes_of(&output_packet(&connection, 2, 1, outputs(), false).unwrap());

        // The header is the 32-bit word after the sequence count, bit 0 is run/idle
        assert_eq!(running[20..24], [0x01, 0x00, 0x00, 0x00]);
        assert_eq!(idle[20..24], [0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn modeless_direction_has_no_header() {
        let mut connection = sample_connection();
        connection.o2t_real_time_format = RealTimeFormat::Modeless;
        // The same 32 bytes of data, now without the 4-byte header
        connection.request.o2t_network_connection_parameters =
            standard_parameters(34, ConnectionSizeType::Fixed);

        let bytes = bytes_of(
            &output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 32]), true).unwrap(),
        );

        // Item count, address item (4 + 8), data item header (4), count (2), 32 data bytes
        assert_eq!(bytes.len(), 2 + 12 + 4 + 2 + 32);
        assert_eq!(bytes[16..18], [34, 0]);
    }

    #[test]
    fn fixed_size_outputs_must_match_the_data_size() {
        let connection = sample_connection();

        assert!(matches!(
            output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 31]), true),
            Err(Error::OutputSize {
                connection_size_type: ConnectionSizeType::Fixed,
                data_size: 32,
                actual: 31
            })
        ));
        assert!(output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 33]), true).is_err());
        assert!(output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 32]), true).is_ok());
    }

    /// A caller's output assembly for the sample connection's 32 bytes
    #[binrw::binwrite]
    #[bw(little)]
    #[derive(Debug)]
    struct SampleOutputs {
        command: CipUdint,
        values: [CipUint; 14],
    }

    #[test]
    fn typed_outputs_send_the_same_packet_as_their_bytes() {
        let connection = sample_connection();
        let outputs = SampleOutputs {
            command: 0x0403_0201,
            values: core::array::from_fn(|index| index as CipUint),
        };
        let mut bytes = vec![0x01, 0x02, 0x03, 0x04];
        bytes.extend((0..14u16).flat_map(|value| value.to_le_bytes()));

        let typed = output_packet(
            &connection,
            1,
            1,
            CipDataOpt::Typed(Box::new(outputs)),
            true,
        );
        let raw = output_packet(&connection, 1, 1, CipDataOpt::Raw(bytes), true);

        let raw = bytes_of(&raw.unwrap());
        let typed = bytes_of(&typed.unwrap());

        assert_eq_hex!(raw, typed);
    }

    #[test]
    fn typed_outputs_must_fit_the_connection() {
        assert!(matches!(
            output_packet(
                &sample_connection(),
                1,
                1,
                CipDataOpt::Typed(Box::new([0u8; 31])),
                true
            ),
            Err(Error::OutputSize { actual: 31, .. })
        ));
    }

    #[test]
    fn variable_size_outputs_may_be_shorter() {
        let mut connection = sample_connection();
        connection.request.o2t_network_connection_parameters =
            standard_parameters(38, ConnectionSizeType::Variable);

        assert!(output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 0]), true).is_ok());
        assert!(output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 31]), true).is_ok());
        assert!(output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 32]), true).is_ok());
        assert!(output_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 33]), true).is_err());
    }

    // ======= Inputs ========

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
    ) -> Result<(SequencedAddress, IoData), Discarded> {
        accept_input(
            connection,
            Some(last),
            &input_packet(received, 1, &[0; 32]),
            from_adapter(),
        )
    }

    #[test]
    fn first_packet_is_accepted_with_any_sequence_number() {
        let data: Vec<u8> = (0..32).rev().collect();

        assert_eq!(
            accept_input(
                &sample_connection(),
                None,
                &input_packet(0xdead_beef, 7, &data),
                from_adapter()
            ),
            Ok((
                SequencedAddress {
                    connection_id: T2O_NETWORK_CONNECTION_ID,
                    encapsulation_sequence_number: 0xdead_beef,
                },
                IoData {
                    cip_sequence_count: Some(7),
                    run_idle_header: None,
                    data: CipDataOpt::Raw(data),
                }
            ))
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
            accept_input(&sample_connection(), None, &packet, from_adapter()),
            Err(Discarded::UnknownConnection {
                connection_id: Some(T2O_NETWORK_CONNECTION_ID + 1)
            })
        );
    }

    #[test]
    fn packets_from_another_host_are_discarded() {
        let stranger = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(172, 28, 0, 99), 2222));

        assert_eq!(
            accept_input(
                &sample_connection(),
                None,
                &input_packet(1, 1, &[0; 32]),
                stranger
            ),
            Err(Discarded::WrongSender(stranger))
        );
    }

    #[test]
    fn the_adapter_may_send_from_any_port() {
        let from = SocketAddr::V4(SocketAddrV4::new(TARGET_IP, 49152));

        assert!(
            accept_input(
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
            accept_after(&connection, 10, 10),
            Err(Discarded::StaleSequenceNumber {
                received: 10,
                last: 10
            })
        );
        assert_eq!(
            accept_after(&connection, 10, 9),
            Err(Discarded::StaleSequenceNumber {
                received: 9,
                last: 10
            })
        );
    }

    #[test]
    fn sequence_numbers_roll_over() {
        let connection = sample_connection();

        assert!(accept_after(&connection, CipUdint::MAX - 1, CipUdint::MAX).is_ok());
        assert!(accept_after(&connection, CipUdint::MAX, 0).is_ok());
        // Back across the rollover is stale
        assert_eq!(
            accept_after(&connection, 1, CipUdint::MAX),
            Err(Discarded::StaleSequenceNumber {
                received: CipUdint::MAX,
                last: 1
            })
        );
    }

    #[test]
    fn small_multiplier_allows_a_gap_of_sixteen() {
        // The sample connection has a x4 multiplier, so the floor of 16 applies
        let connection = sample_connection();

        assert!(accept_after(&connection, 100, 116).is_ok());
        assert_eq!(
            accept_after(&connection, 116, 133),
            Err(Discarded::SequenceGapTooLarge {
                received: 133,
                last: 116,
                allowed: 16
            })
        );
    }

    #[test]
    fn large_multiplier_allows_a_gap_of_multiplier_plus_one() {
        let mut connection = sample_connection();
        connection.request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;

        assert!(accept_after(&connection, 1000, 1513).is_ok());
        assert_eq!(
            accept_after(&connection, 1513, 2027),
            Err(Discarded::SequenceGapTooLarge {
                received: 2027,
                last: 1513,
                allowed: 513
            })
        );
    }

    #[test]
    fn data_of_the_wrong_size_is_discarded() {
        let connection = sample_connection();
        let accept = |packet: &IoPacket| accept_input(&connection, None, packet, from_adapter());

        assert_eq!(
            accept(&input_packet(1, 1, &[0; 31])),
            Err(Discarded::WrongSize {
                expected: 32,
                actual: 31
            })
        );
        assert_eq!(
            accept(&input_packet(1, 1, &[0; 33])),
            Err(Discarded::WrongSize {
                expected: 32,
                actual: 33
            })
        );
        // Not even a sequence count
        assert_eq!(
            accept(&IoPacket::new(
                T2O_NETWORK_CONNECTION_ID,
                1,
                CipDataOpt::Raw(vec![0])
            )),
            Err(Discarded::WrongSize {
                expected: 32,
                actual: 0
            })
        );
    }

    #[test]
    fn variable_size_data_may_be_shorter() {
        let mut connection = sample_connection();
        connection.request.t2o_network_connection_parameters =
            standard_parameters(34, ConnectionSizeType::Variable);

        let (_, inputs) = accept_input(
            &connection,
            None,
            &input_packet(1, 1, &[7; 3]),
            from_adapter(),
        )
        .unwrap();

        assert_eq!(inputs.data, CipDataOpt::Raw(vec![7; 3]));
    }

    #[test]
    fn packets_without_a_connected_data_item_are_malformed() {
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

        assert!(matches!(
            accept_input(&sample_connection(), None, &packet, from_adapter()),
            Err(Discarded::Malformed(_))
        ));
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

        let (_, inputs) = accept_input(&connection, None, &packet, from_adapter()).unwrap();

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

//! Stage 3b: consuming the T->O packets (the inputs).
//!
//! The consumer touches no socket: the caller hands it every packet that arrives on the I/O
//! socket, and it answers with the inputs or with the reason the packet was discarded. It also
//! keeps the connection's deadline: the moment it has timed out unless another packet arrives.

use std::fmt;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use binrw::{BinRead, BinResult, BinWrite};

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, RealTimeFormat, TransportClass, connection_size,
};
use eipscanne_rs::cip::io_data::IoData;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::IoPacket;

use crate::implicit::open::OpenConnection;

/// Before the first packet the adapter gets at least this long, whatever the packet interval
const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// A packet may be at least this many numbers ahead of the last accepted one
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Screens, decodes and times the input packets of one connection
#[derive(Debug)]
pub struct Consumer {
    /// The T->O connection ID this scanner chose; every input packet must carry it
    connection_id: CipUdint,
    target_ip: Ipv4Addr,
    transport_class: TransportClass,
    real_time_format: RealTimeFormat,
    data_size: u16,
    connection_size_type: ConnectionSizeType,
    /// Timeout multiplier x T->O actual packet interval
    timeout: Duration,
    /// How far ahead of the last accepted number a packet may be
    allowed_sequence_gap: u32,
    established_at: Instant,
    last_accepted: Option<Accepted>,
}

/// What the consumer remembers of the last accepted packet
#[derive(Debug, Clone, Copy)]
struct Accepted {
    encapsulation_sequence_number: CipUdint,
    cip_sequence_count: Option<CipUint>,
    at: Instant,
}

/// The inputs carried by an accepted packet
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    /// The application data (the input assembly), for the caller to decode
    pub data: Vec<u8>,
    /// The run/idle flag of the 32-bit header, when the direction carries one
    pub run_idle: Option<bool>,
    /// Whether the adapter counted this as new data: false when the CIP sequence count equals the
    /// previous accepted packet's
    pub new_data: bool,
}

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

// ======= Start of Consumer impl ========

impl Consumer {
    /// A consumer for the inputs of `connection`, established at `established_at` (when the
    /// Forward_Open reply arrived), from which the first deadline counts
    pub fn new(connection: &OpenConnection, established_at: Instant) -> Self {
        let multiplier = connection.config.connection_timeout_multiplier.multiplier();
        let packet_interval = u64::from(connection.response.t2o_actual_packet_interval);

        Consumer {
            connection_id: connection.response.t2o_network_connection_id,
            target_ip: connection.target_ip,
            transport_class: connection.config.transport_class,
            real_time_format: connection.config.t2o.real_time_format,
            data_size: connection.config.t2o.data_size,
            connection_size_type: connection.config.t2o.connection_size_type,
            // In 64 bits: a x512 multiplier on a 10 s interval overflows 32-bit microseconds
            timeout: Duration::from_micros(u64::from(multiplier) * packet_interval),
            allowed_sequence_gap: MIN_ALLOWED_SEQUENCE_GAP.max(multiplier.saturating_add(1)),
            established_at,
            last_accepted: None,
        }
    }

    /// Takes a packet received at `now` from `from`: the inputs it carries, or why it was
    /// discarded. Only accepted packets move the deadline.
    pub fn accept(
        &mut self,
        packet: &IoPacket,
        from: SocketAddr,
        now: Instant,
    ) -> Result<Input, Discarded> {
        // 1. It is for this connection
        let address = packet.sequenced_address();
        if address.map(|address| address.connection_id) != Some(self.connection_id) {
            return Err(Discarded::UnknownConnection {
                connection_id: address.map(|address| address.connection_id),
            });
        }
        let received = address
            .expect("checked above")
            .encapsulation_sequence_number;

        // 2. It comes from the adapter (whatever port it chose)
        if from.ip() != IpAddr::V4(self.target_ip) {
            return Err(Discarded::WrongSender(from));
        }

        // 3. It is newer than the last accepted packet, but not unreasonably so. The first
        //    packet is accepted with any number
        if let Some(last) = self.last_accepted {
            self.check_sequence_number(received, last.encapsulation_sequence_number)?;
        }

        // 4. Its data decodes with the shape of this direction and has the agreed size
        let (data, run_idle, cip_sequence_count) = self.decode(packet)?;

        let new_data = match (self.last_accepted, cip_sequence_count) {
            (Some(last), Some(count)) => last.cip_sequence_count != Some(count),
            _ => true,
        };
        self.last_accepted = Some(Accepted {
            encapsulation_sequence_number: received,
            cip_sequence_count,
            at: now,
        });

        Ok(Input {
            data,
            run_idle,
            new_data,
        })
    }

    /// When the connection has timed out unless a packet is accepted first: a grace period after
    /// it was established, then the timeout after every accepted packet
    pub fn deadline(&self) -> Instant {
        match self.last_accepted {
            None => self.established_at + FIRST_PACKET_GRACE.max(self.timeout),
            Some(last) => last.at + self.timeout,
        }
    }

    /// Compares two sequence numbers modulo 2^32: the distance from `last` to `received` must
    /// be at least 1 and at most the allowed gap. A distance of 0 or of 2^31 and over means an
    /// older packet (or a repeat).
    fn check_sequence_number(&self, received: CipUdint, last: CipUdint) -> Result<(), Discarded> {
        let distance = received.wrapping_sub(last);
        if distance == 0 || distance >= 1 << 31 {
            return Err(Discarded::StaleSequenceNumber { received, last });
        }
        if distance > self.allowed_sequence_gap {
            return Err(Discarded::SequenceGapTooLarge {
                received,
                last,
                allowed: self.allowed_sequence_gap,
            });
        }
        Ok(())
    }

    /// The application data, run/idle flag and CIP sequence count of the packet's Connected
    /// Data Item
    #[allow(clippy::type_complexity)]
    fn decode(
        &self,
        packet: &IoPacket,
    ) -> Result<(Vec<u8>, Option<bool>, Option<CipUint>), Discarded> {
        let Some(connected_data) = packet.connected_data() else {
            return Err(Discarded::Malformed("no Connected Data Item".to_string()));
        };
        let mut bytes = Cursor::new(Vec::new());
        connected_data
            .write_le_args(&mut bytes, (0,))
            .map_err(|error| Discarded::Malformed(error.to_string()))?;
        let bytes = bytes.into_inner();

        // The sequence count and header come before the data; a shorter item has no data to
        // speak of
        let overhead = connection_size(0, self.transport_class, self.real_time_format);
        let Ok(byte_len) = u16::try_from(bytes.len()) else {
            return Err(Discarded::WrongSize {
                expected: self.data_size,
                actual: bytes.len(),
            });
        };
        if byte_len < overhead {
            return Err(Discarded::WrongSize {
                expected: self.data_size,
                actual: 0,
            });
        }

        let io_data = IoData::read_le_args(
            &mut Cursor::new(&bytes),
            (byte_len, self.transport_class, self.real_time_format),
        )
        .map_err(|error| Discarded::Malformed(error.to_string()))?;

        let data = match io_data.data {
            CipDataOpt::Raw(data) => data,
            CipDataOpt::Typed(_) => unreachable!("data read from bytes is always raw"),
        };
        let fits = match self.connection_size_type {
            ConnectionSizeType::Fixed => data.len() == usize::from(self.data_size),
            ConnectionSizeType::Variable => data.len() <= usize::from(self.data_size),
        };
        if !fits {
            return Err(Discarded::WrongSize {
                expected: self.data_size,
                actual: data.len(),
            });
        }

        Ok((
            data,
            io_data.run_idle_header.map(|header| header.run_idle()),
            io_data.cip_sequence_count,
        ))
    }
}

// ^^^^^^^^ End of Consumer impl ^^^^^^^^

// ======= Start of Input impl ========

impl Input {
    /// The inputs decoded as a `T` declared by the caller (the input assembly), the implicit
    /// counterpart of `explicit::decode_reply`. Every byte must belong to `T`: bytes left over
    /// mean `T` does not describe this assembly.
    pub fn decode<T>(&self) -> BinResult<T>
    where
        T: for<'a> BinRead<Args<'a> = ()>,
    {
        let mut cursor = Cursor::new(&self.data);
        let value = T::read_le(&mut cursor)?;
        let pos = cursor.position();
        if pos != self.data.len() as u64 {
            return Err(binrw::Error::AssertFail {
                pos,
                message: format!(
                    "the inputs are {} bytes, the type read only {pos}",
                    self.data.len()
                ),
            });
        }
        Ok(value)
    }
}

// ^^^^^^^^ End of Input impl ^^^^^^^^

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

#[cfg(test)]
mod tests {
    use std::net::SocketAddrV4;

    use eipscanne_rs::cip::connection_manager::parameters::ConnectionTimeoutMultiplier;
    use eipscanne_rs::cip::io_data::RunIdleHeader;

    use crate::test_support::{T2O_NETWORK_CONNECTION_ID, TARGET_IP, sample_connection};

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

    fn consumer() -> (Consumer, Instant) {
        let established_at = Instant::now();
        (
            Consumer::new(&sample_connection(), established_at),
            established_at,
        )
    }

    #[test]
    fn first_packet_is_accepted_with_any_sequence_number() {
        let (mut consumer, now) = consumer();
        let inputs: Vec<u8> = (0..32).rev().collect();

        let input = consumer
            .accept(&input_packet(0xdead_beef, 7, &inputs), from_adapter(), now)
            .unwrap();

        assert_eq!(
            input,
            Input {
                data: inputs,
                run_idle: None,
                new_data: true
            }
        );
    }

    #[test]
    fn packets_for_another_connection_are_discarded() {
        let (mut consumer, now) = consumer();
        let packet = IoPacket::new(
            T2O_NETWORK_CONNECTION_ID + 1,
            1,
            CipDataOpt::Raw(vec![0; 34]),
        );

        assert_eq!(
            consumer.accept(&packet, from_adapter(), now),
            Err(Discarded::UnknownConnection {
                connection_id: Some(T2O_NETWORK_CONNECTION_ID + 1)
            })
        );
    }

    #[test]
    fn packets_from_another_host_are_discarded() {
        let (mut consumer, now) = consumer();
        let stranger = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(172, 28, 0, 99), 2222));

        assert_eq!(
            consumer.accept(&input_packet(1, 1, &[0; 32]), stranger, now),
            Err(Discarded::WrongSender(stranger))
        );
    }

    #[test]
    fn the_adapter_may_send_from_any_port() {
        let (mut consumer, now) = consumer();
        let from = SocketAddr::V4(SocketAddrV4::new(TARGET_IP, 49152));

        assert!(
            consumer
                .accept(&input_packet(1, 1, &[0; 32]), from, now)
                .is_ok()
        );
    }

    #[test]
    fn repeated_and_older_sequence_numbers_are_stale() {
        let (mut consumer, now) = consumer();
        consumer
            .accept(&input_packet(10, 1, &[0; 32]), from_adapter(), now)
            .unwrap();

        assert_eq!(
            consumer.accept(&input_packet(10, 2, &[0; 32]), from_adapter(), now),
            Err(Discarded::StaleSequenceNumber {
                received: 10,
                last: 10
            })
        );
        assert_eq!(
            consumer.accept(&input_packet(9, 2, &[0; 32]), from_adapter(), now),
            Err(Discarded::StaleSequenceNumber {
                received: 9,
                last: 10
            })
        );
    }

    #[test]
    fn sequence_numbers_roll_over() {
        let (mut consumer, now) = consumer();
        consumer
            .accept(
                &input_packet(CipUdint::MAX - 1, 1, &[0; 32]),
                from_adapter(),
                now,
            )
            .unwrap();

        assert!(
            consumer
                .accept(
                    &input_packet(CipUdint::MAX, 2, &[0; 32]),
                    from_adapter(),
                    now
                )
                .is_ok()
        );
        assert!(
            consumer
                .accept(&input_packet(0, 3, &[0; 32]), from_adapter(), now)
                .is_ok()
        );
        assert!(
            consumer
                .accept(&input_packet(1, 4, &[0; 32]), from_adapter(), now)
                .is_ok()
        );
        // Back across the rollover is stale
        assert_eq!(
            consumer.accept(
                &input_packet(CipUdint::MAX, 5, &[0; 32]),
                from_adapter(),
                now
            ),
            Err(Discarded::StaleSequenceNumber {
                received: CipUdint::MAX,
                last: 1
            })
        );
    }

    #[test]
    fn small_multiplier_allows_a_gap_of_sixteen() {
        // The sample connection has a x4 multiplier, so the floor of 16 applies
        let (mut consumer, now) = consumer();
        consumer
            .accept(&input_packet(100, 1, &[0; 32]), from_adapter(), now)
            .unwrap();

        assert!(
            consumer
                .accept(&input_packet(116, 2, &[0; 32]), from_adapter(), now)
                .is_ok()
        );
        assert_eq!(
            consumer.accept(&input_packet(133, 3, &[0; 32]), from_adapter(), now),
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
        connection.config.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
        let now = Instant::now();
        let mut consumer = Consumer::new(&connection, now);
        consumer
            .accept(&input_packet(1000, 1, &[0; 32]), from_adapter(), now)
            .unwrap();

        assert!(
            consumer
                .accept(&input_packet(1513, 2, &[0; 32]), from_adapter(), now)
                .is_ok()
        );
        assert_eq!(
            consumer.accept(&input_packet(2027, 3, &[0; 32]), from_adapter(), now),
            Err(Discarded::SequenceGapTooLarge {
                received: 2027,
                last: 1513,
                allowed: 513
            })
        );
    }

    #[test]
    fn unchanged_cip_sequence_count_is_not_new_data() {
        let (mut consumer, now) = consumer();

        let first = consumer
            .accept(&input_packet(1, 5, &[1; 32]), from_adapter(), now)
            .unwrap();
        let resent = consumer
            .accept(&input_packet(2, 5, &[1; 32]), from_adapter(), now)
            .unwrap();
        let changed = consumer
            .accept(&input_packet(3, 6, &[2; 32]), from_adapter(), now)
            .unwrap();

        assert_eq!(
            (first.new_data, resent.new_data, changed.new_data),
            (true, false, true)
        );
    }

    #[test]
    fn data_of_the_wrong_size_is_discarded() {
        let (mut consumer, now) = consumer();

        assert_eq!(
            consumer.accept(&input_packet(1, 1, &[0; 31]), from_adapter(), now),
            Err(Discarded::WrongSize {
                expected: 32,
                actual: 31
            })
        );
        assert_eq!(
            consumer.accept(&input_packet(1, 1, &[0; 33]), from_adapter(), now),
            Err(Discarded::WrongSize {
                expected: 32,
                actual: 33
            })
        );
        // Not even a sequence count
        assert_eq!(
            consumer.accept(
                &IoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(vec![0])),
                from_adapter(),
                now
            ),
            Err(Discarded::WrongSize {
                expected: 32,
                actual: 0
            })
        );
        // A discarded packet does not count as accepted
        assert!(
            consumer
                .accept(&input_packet(1, 1, &[0; 32]), from_adapter(), now)
                .is_ok()
        );
    }

    /// A caller's input assembly for the sample connection's 32 bytes
    #[binrw::binrw]
    #[brw(little)]
    #[derive(Debug, PartialEq)]
    struct SampleInputs {
        status: CipUdint,
        values: [CipUint; 14],
    }

    #[test]
    fn inputs_decode_as_the_callers_assembly() {
        let (mut consumer, now) = consumer();
        let mut inputs = vec![0x01, 0x02, 0x03, 0x04];
        inputs.extend((0..14u16).flat_map(|value| value.to_le_bytes()));

        let input = consumer
            .accept(&input_packet(1, 1, &inputs), from_adapter(), now)
            .unwrap();

        assert_eq!(
            input.decode::<SampleInputs>().unwrap(),
            SampleInputs {
                status: 0x0403_0201,
                values: core::array::from_fn(|index| index as CipUint),
            }
        );
    }

    #[test]
    fn inputs_with_bytes_left_over_do_not_decode() {
        let input = Input {
            data: vec![0; 32],
            run_idle: None,
            new_data: true,
        };

        assert!(input.decode::<[u8; 31]>().is_err());
        assert!(input.decode::<[u8; 33]>().is_err());
        assert!(input.decode::<[u8; 32]>().is_ok());
    }

    #[test]
    fn variable_size_data_may_be_shorter() {
        let mut connection = sample_connection();
        connection.config.t2o.connection_size_type = ConnectionSizeType::Variable;
        let now = Instant::now();
        let mut consumer = Consumer::new(&connection, now);

        let input = consumer
            .accept(&input_packet(1, 1, &[7; 3]), from_adapter(), now)
            .unwrap();

        assert_eq!(input.data, vec![7; 3]);
    }

    #[test]
    fn packets_without_a_connected_data_item_are_malformed() {
        let (mut consumer, now) = consumer();
        let packet = IoPacket {
            items: vec![
                eipscanne_rs::eip::description::CommonPacketItem::SequencedAddressItem(
                    eipscanne_rs::eip::io_packet::SequencedAddress {
                        connection_id: T2O_NETWORK_CONNECTION_ID,
                        encapsulation_sequence_number: 1,
                    },
                ),
            ],
        };

        assert!(matches!(
            consumer.accept(&packet, from_adapter(), now),
            Err(Discarded::Malformed(_))
        ));
    }

    #[test]
    fn header_direction_reports_run_idle() {
        let mut connection = sample_connection();
        connection.config.t2o.real_time_format = RealTimeFormat::Header32Bit;
        let now = Instant::now();
        let mut consumer = Consumer::new(&connection, now);

        let idle_header = RunIdleHeader::builder()
            .run_idle(false)
            .claim_output_ownership(false)
            .ready_for_ownership_of_outputs(bilge::prelude::u2::new(0))
            .build();
        let mut bytes = 1u16.to_le_bytes().to_vec();
        bytes.extend_from_slice(&u32::from(idle_header).to_le_bytes());
        bytes.extend_from_slice(&[0; 32]);
        let packet = IoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(bytes));

        let input = consumer.accept(&packet, from_adapter(), now).unwrap();

        assert_eq!(input.run_idle, Some(false));
        assert_eq!(input.data.len(), 32);
    }

    #[test]
    fn first_deadline_is_the_larger_of_the_grace_period_and_the_timeout() {
        // x4 on a 1 s interval is 4 s, below the 10 s grace period
        let (consumer, established_at) = consumer();
        assert_eq!(
            consumer.deadline(),
            established_at + Duration::from_secs(10)
        );

        // x16 on a 1 s interval is 16 s, above it
        let mut connection = sample_connection();
        connection.config.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X16;
        let consumer = Consumer::new(&connection, established_at);
        assert_eq!(
            consumer.deadline(),
            established_at + Duration::from_secs(16)
        );
    }

    #[test]
    fn deadline_follows_the_last_accepted_packet() {
        let (mut consumer, established_at) = consumer();
        let later = established_at + Duration::from_secs(3);

        consumer
            .accept(&input_packet(1, 1, &[0; 32]), from_adapter(), later)
            .unwrap();

        assert_eq!(consumer.deadline(), later + Duration::from_secs(4));

        // A discarded packet does not move it
        let _ = consumer.accept(
            &input_packet(1, 1, &[0; 32]),
            from_adapter(),
            later + Duration::from_secs(1),
        );
        assert_eq!(consumer.deadline(), later + Duration::from_secs(4));
    }

    #[test]
    fn timeout_does_not_overflow_for_the_largest_multiplier_and_interval() {
        let mut connection = sample_connection();
        connection.config.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
        connection.response.t2o_actual_packet_interval = 10_000_000;
        let established_at = Instant::now();
        let consumer = Consumer::new(&connection, established_at);

        assert_eq!(
            consumer.deadline(),
            established_at + Duration::from_secs(5120)
        );
    }
}

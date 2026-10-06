//! Stage 3a: producing the O->T packets (the outputs).
//!
//! The producer touches no socket: it turns output bytes into numbered packets, and the caller
//! sends them every `period()`.

use std::fmt;
use std::time::Duration;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, RealTimeFormat, TransportClass,
};
use eipscanne_rs::cip::io_data::{IoData, RunIdleHeader};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::IoPacket;

use crate::implicit::open::OpenConnection;

/// Numbers and frames the output packets of one connection
#[derive(Debug)]
pub struct Producer {
    /// The O->T connection ID the adapter chose; every output packet is addressed with it
    connection_id: CipUdint,
    /// The number the next packet carries
    encapsulation_sequence_number: CipUdint,
    /// The count of the last packet; the first packet gets 1
    cip_sequence_count: CipUint,
    /// The outputs of the last packet, to tell a change from a resend
    last_outputs: Option<Vec<u8>>,
    transport_class: TransportClass,
    real_time_format: RealTimeFormat,
    data_size: u16,
    connection_size_type: ConnectionSizeType,
    period: Duration,
}

/// The outputs do not fit the connection
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeError {
    pub connection_size_type: ConnectionSizeType,
    pub data_size: u16,
    pub actual: usize,
}

// ======= Start of Producer impl ========

impl Producer {
    /// A producer for the outputs of `connection`, whose first packet carries
    /// `initial_encapsulation_sequence_number`. The caller picks that number (randomly, in
    /// the example) so a restarted scanner does not repeat the numbers the adapter last saw.
    pub fn new(
        connection: &OpenConnection,
        initial_encapsulation_sequence_number: CipUdint,
    ) -> Self {
        Producer {
            connection_id: connection.response.o2t_network_connection_id,
            encapsulation_sequence_number: initial_encapsulation_sequence_number,
            cip_sequence_count: 0,
            last_outputs: None,
            transport_class: connection.config.transport_class,
            real_time_format: connection.config.o2t.real_time_format,
            data_size: connection.config.o2t.data_size,
            connection_size_type: connection.config.o2t.connection_size_type,
            period: Duration::from_micros(u64::from(
                connection.response.o2t_actual_packet_interval,
            )),
        }
    }

    /// How often a packet must be sent: the O->T actual packet interval the adapter granted
    pub fn period(&self) -> Duration {
        self.period
    }

    /// The next packet to send, carrying `outputs` with the run flag set to `run`.
    ///
    /// Every packet gets a new encapsulation sequence number. The CIP sequence count only moves
    /// when the outputs differ from the last packet's: a resend of unchanged outputs keeps the
    /// count, which tells the adapter nothing new arrived.
    pub fn next_packet(&mut self, outputs: &[u8], run: bool) -> Result<IoPacket, SizeError> {
        self.check_size(outputs.len())?;

        if self.last_outputs.as_deref() != Some(outputs) {
            self.cip_sequence_count = self.cip_sequence_count.wrapping_add(1);
            self.last_outputs = Some(outputs.to_vec());
        }

        let cip_sequence_count = match self.transport_class {
            TransportClass::Class1 | TransportClass::Class2 | TransportClass::Class3 => {
                Some(self.cip_sequence_count)
            }
            TransportClass::Class0 | TransportClass::Unknown(_) => None,
        };
        let run_idle_header = match self.real_time_format {
            RealTimeFormat::Header32Bit => Some(
                RunIdleHeader::builder()
                    .run_idle(run)
                    .claim_output_ownership(false)
                    .ready_for_ownership_of_outputs(bilge::prelude::u2::new(0))
                    .build(),
            ),
            RealTimeFormat::Modeless | RealTimeFormat::ZeroLength | RealTimeFormat::Heartbeat => {
                None
            }
        };

        let data = IoData {
            cip_sequence_count,
            run_idle_header,
            data: CipDataOpt::Raw(outputs.to_vec()),
        };
        let packet = IoPacket::new(
            self.connection_id,
            self.encapsulation_sequence_number,
            CipDataOpt::Typed(Box::new(data)),
        );
        self.encapsulation_sequence_number = self.encapsulation_sequence_number.wrapping_add(1);

        Ok(packet)
    }

    fn check_size(&self, actual: usize) -> Result<(), SizeError> {
        let fits = match self.connection_size_type {
            ConnectionSizeType::Fixed => actual == usize::from(self.data_size),
            ConnectionSizeType::Variable => actual <= usize::from(self.data_size),
        };
        if fits {
            Ok(())
        } else {
            Err(SizeError {
                connection_size_type: self.connection_size_type,
                data_size: self.data_size,
                actual,
            })
        }
    }
}

// ^^^^^^^^ End of Producer impl ^^^^^^^^

// ======= Start of SizeError impl ========

impl fmt::Display for SizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let expected = match self.connection_size_type {
            ConnectionSizeType::Fixed => "exactly",
            ConnectionSizeType::Variable => "at most",
        };
        write!(
            f,
            "{} output bytes given, the connection carries {expected} {}",
            self.actual, self.data_size
        )
    }
}

impl std::error::Error for SizeError {}

// ^^^^^^^^ End of SizeError impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use binrw::BinWrite;
    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::types::CipByte;

    use crate::test_support::{O2T_NETWORK_CONNECTION_ID, sample_connection};

    use super::*;

    /// The bytes a packet serializes to
    fn bytes_of(packet: &IoPacket) -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        packet.write(&mut bytes).unwrap();
        bytes.into_inner()
    }

    /// The encapsulation sequence number and the CIP sequence count of a packet, decoded from
    /// its bytes: the sequenced address item data, then the first word of the connected data
    fn numbers_of(packet: &IoPacket) -> (CipUdint, CipUint) {
        let bytes = bytes_of(packet);
        (
            CipUdint::from_le_bytes(bytes[10..14].try_into().unwrap()),
            CipUint::from_le_bytes(bytes[18..20].try_into().unwrap()),
        )
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

        let connection = sample_connection();
        let mut producer = Producer::new(&connection, 1);
        let outputs: Vec<u8> = (0..32).collect();

        let packet = producer.next_packet(&outputs, true).unwrap();

        assert_eq!(
            packet.sequenced_address().unwrap().connection_id,
            O2T_NETWORK_CONNECTION_ID
        );
        assert_eq_hex!(expected_byte_array, bytes_of(&packet));
        assert_eq!(producer.period(), Duration::from_secs(1));
    }

    #[test]
    fn every_packet_gets_a_new_encapsulation_sequence_number() {
        let connection = sample_connection();
        let mut producer = Producer::new(&connection, 100);
        let outputs = [0u8; 32];

        let numbers: Vec<CipUdint> = (0..3)
            .map(|_| numbers_of(&producer.next_packet(&outputs, true).unwrap()).0)
            .collect();

        assert_eq!(numbers, vec![100, 101, 102]);
    }

    #[test]
    fn encapsulation_sequence_number_rolls_over() {
        let connection = sample_connection();
        let mut producer = Producer::new(&connection, CipUdint::MAX);
        let outputs = [0u8; 32];

        let first = numbers_of(&producer.next_packet(&outputs, true).unwrap()).0;
        let second = numbers_of(&producer.next_packet(&outputs, true).unwrap()).0;

        assert_eq!((first, second), (CipUdint::MAX, 0));
    }

    #[test]
    fn cip_sequence_count_only_moves_when_the_outputs_change() {
        let connection = sample_connection();
        let mut producer = Producer::new(&connection, 1);
        let a = [0xaau8; 32];
        let b = [0xbbu8; 32];

        let counts: Vec<CipUint> = [a, a, b, b, a]
            .iter()
            .map(|outputs| numbers_of(&producer.next_packet(outputs, true).unwrap()).1)
            .collect();

        assert_eq!(counts, vec![1, 1, 2, 2, 3]);
    }

    #[test]
    fn run_flag_is_carried_by_the_header() {
        let connection = sample_connection();
        let mut producer = Producer::new(&connection, 1);
        let outputs = [0u8; 32];

        let running = bytes_of(&producer.next_packet(&outputs, true).unwrap());
        let idle = bytes_of(&producer.next_packet(&outputs, false).unwrap());

        // The header is the 32-bit word after the sequence count, bit 0 is run/idle
        assert_eq!(running[20..24], [0x01, 0x00, 0x00, 0x00]);
        assert_eq!(idle[20..24], [0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn modeless_direction_has_no_header() {
        let mut connection = sample_connection();
        connection.config.o2t.real_time_format = RealTimeFormat::Modeless;
        let mut producer = Producer::new(&connection, 1);

        let bytes = bytes_of(&producer.next_packet(&[0u8; 32], true).unwrap());

        // Item count, address item (4 + 8), data item header (4), count (2), 32 data bytes
        assert_eq!(bytes.len(), 2 + 12 + 4 + 2 + 32);
        assert_eq!(bytes[16..18], [34, 0]);
    }

    #[test]
    fn fixed_size_outputs_must_match_the_data_size() {
        let connection = sample_connection();
        let mut producer = Producer::new(&connection, 1);

        assert_eq!(
            producer.next_packet(&[0u8; 31], true).unwrap_err(),
            SizeError {
                connection_size_type: ConnectionSizeType::Fixed,
                data_size: 32,
                actual: 31
            }
        );
        assert!(producer.next_packet(&[0u8; 33], true).is_err());
        assert!(producer.next_packet(&[0u8; 32], true).is_ok());
    }

    #[test]
    fn variable_size_outputs_may_be_shorter() {
        let mut connection = sample_connection();
        connection.config.o2t.connection_size_type = ConnectionSizeType::Variable;
        let mut producer = Producer::new(&connection, 1);

        assert!(producer.next_packet(&[0u8; 0], true).is_ok());
        assert!(producer.next_packet(&[0u8; 31], true).is_ok());
        assert!(producer.next_packet(&[0u8; 32], true).is_ok());
        assert!(producer.next_packet(&[0u8; 33], true).is_err());
    }
}

//! Stage 3, O->T: the outputs. The caller sends one packet every O->T actual packet interval
//! (`connection.response.o2t_actual_packet_interval`) and owns the two sequence numbers; nothing
//! here keeps state.

use std::io::Cursor;
use std::net::SocketAddrV4;

use binrw::{BinResult, BinWrite};
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::parameters::{RealTimeFormat, TransportClass};
use eipscanne_rs::cip::io_data::{IoData, RunIdleHeader};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::IoPacket;

use crate::error::Error;
use crate::implicit::connection::{OpenConnection, data_len_matches_connection, data_size};

/// The O->T packet of `connection` carrying `outputs` with the run flag set to `run`. The
/// outputs are the caller's assembly, as bytes (`CipDataOpt::Raw`) or as a `binrw` struct
/// (`CipDataOpt::Typed`); the packet holds them as the bytes that go on the wire.
///
/// The caller numbers the packets: every packet gets the next `encapsulation_sequence_number`
/// (start it at a random number, so a restarted scanner does not repeat the numbers the adapter
/// last saw), and `cip_sequence_count` moves only when the outputs change, so a resend of
/// unchanged outputs tells the adapter nothing new arrived.
pub fn build_o2t_packet(
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
    if !data_len_matches_connection(outputs.len(), data_size, connection_size_type) {
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

#[cfg(test)]
mod tests {
    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::connection_manager::parameters::ConnectionSizeType;
    use eipscanne_rs::cip::types::CipByte;

    use crate::implicit::connection::test_support::{
        O2T_NETWORK_CONNECTION_ID, sample_connection, standard_parameters,
    };

    use super::*;

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
            build_o2t_packet(&sample_connection(), 1, 1, CipDataOpt::Raw(outputs), true).unwrap();

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

        let running = bytes_of(&build_o2t_packet(&connection, 1, 1, outputs(), true).unwrap());
        let idle = bytes_of(&build_o2t_packet(&connection, 2, 1, outputs(), false).unwrap());

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
            &build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 32]), true).unwrap(),
        );

        // Item count, address item (4 + 8), data item header (4), count (2), 32 data bytes
        assert_eq!(bytes.len(), 2 + 12 + 4 + 2 + 32);
        assert_eq!(bytes[16..18], [34, 0]);
    }

    #[test]
    fn fixed_size_outputs_must_match_the_data_size() {
        let connection = sample_connection();

        assert!(matches!(
            build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 31]), true),
            Err(Error::OutputSize {
                connection_size_type: ConnectionSizeType::Fixed,
                data_size: 32,
                actual: 31
            })
        ));
        assert!(build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 33]), true).is_err());
        assert!(build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 32]), true).is_ok());
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

        let typed = build_o2t_packet(
            &connection,
            1,
            1,
            CipDataOpt::Typed(Box::new(outputs)),
            true,
        );
        let raw = build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(bytes), true);

        let raw = bytes_of(&raw.unwrap());
        let typed = bytes_of(&typed.unwrap());

        assert_eq_hex!(raw, typed);
    }

    #[test]
    fn typed_outputs_must_fit_the_connection() {
        assert!(matches!(
            build_o2t_packet(
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

        assert!(build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 0]), true).is_ok());
        assert!(build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 31]), true).is_ok());
        assert!(build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 32]), true).is_ok());
        assert!(build_o2t_packet(&connection, 1, 1, CipDataOpt::Raw(vec![0; 33]), true).is_err());
    }
}

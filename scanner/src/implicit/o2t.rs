//! Stage 3, O->T: the outputs. The caller sends one packet every O->T actual packet interval and
//! owns the sequence numbers; nothing here keeps state.

use std::io::Cursor;
use std::net::SocketAddrV4;

use binrw::BinWrite;
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, RealTimeFormat, TransportClass,
};
use eipscanne_rs::cip::io_data::{CipIoData, RunIdleHeader};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::EnIpIoPacket;

use crate::error::{Error, Result};
use crate::implicit::connection::{OpenConnection, data_len_matches_connection, data_size};

/// The O->T packet of `connection` carrying `assembly_data` (raw bytes or a `binrw` struct). `run`
/// sets the run/idle flag when the O->T real-time format is `Header32Bit`; other formats ignore it.
///
/// `encapsulation_sequence_number` advances every packet (start it at random, so a restarted
/// scanner does not repeat old numbers); `cip_sequence_count` only when the assembly data changes.
pub fn build_o2t_packet(
    connection: &OpenConnection,
    encapsulation_sequence_number: CipUdint,
    cip_sequence_count: CipUint,
    assembly_data: CipDataOpt,
    run: bool,
) -> Result<EnIpIoPacket> {
    let assembly_data_bytes: Vec<u8> = to_bytes(&assembly_data)?;

    let transport_class: TransportClass =
        connection.request.transport_type_trigger.transport_class();
    let (data_size, connection_size_type): (u16, ConnectionSizeType) = data_size(
        &connection.request.o2t_network_connection_parameters,
        transport_class,
        connection.o2t_real_time_format,
    );
    if !data_len_matches_connection(assembly_data_bytes.len(), data_size, connection_size_type) {
        return Err(Error::OutputSize {
            connection_size_type,
            data_size,
            actual: assembly_data_bytes.len(),
        });
    }

    let cip_sequence_count: Option<CipUint> = match transport_class {
        TransportClass::Class1 | TransportClass::Class2 | TransportClass::Class3 => {
            Some(cip_sequence_count)
        }
        TransportClass::Class0 | TransportClass::Unknown(_) => None,
    };
    let run_idle_header: Option<RunIdleHeader> = match connection.o2t_real_time_format {
        RealTimeFormat::Header32Bit => Some(
            RunIdleHeader::builder()
                .run_idle(run)
                .claim_output_ownership(false)
                .ready_for_ownership_of_outputs(bilge::prelude::u2::new(0))
                .build(),
        ),
        RealTimeFormat::Modeless | RealTimeFormat::ZeroLength | RealTimeFormat::Heartbeat => None,
    };

    let cip_io_data = CipIoData {
        cip_sequence_count,
        run_idle_header,
        data: CipDataOpt::Raw(assembly_data_bytes),
    };
    Ok(EnIpIoPacket::new(
        connection.response.o2t_network_connection_id,
        encapsulation_sequence_number,
        CipDataOpt::Typed(Box::new(cip_io_data)),
    ))
}

/// The bytes of `assembly_data`, written once: the size check needs them, and a typed assembly's
/// size is only known once written
fn to_bytes(assembly_data: &CipDataOpt) -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    assembly_data.write_le_args(&mut bytes, (0,))?;
    Ok(bytes.into_inner())
}

/// Sends one I/O packet to `to`
pub async fn send_io_packet(
    socket: &UdpSocket,
    packet: &EnIpIoPacket,
    to: SocketAddrV4,
) -> Result<()> {
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

    use crate::implicit::connection::test_support::{sample_connection, standard_parameters};

    use super::*;

    /// The bytes of the packet `build_o2t_packet` builds as the first one of `connection`
    fn build(connection: &OpenConnection, outputs: CipDataOpt, run: bool) -> Result<Vec<u8>> {
        let packet = build_o2t_packet(connection, 1, 1, outputs, run)?;
        let mut bytes = Cursor::new(Vec::new());
        packet.write(&mut bytes).unwrap();
        Ok(bytes.into_inner())
    }

    #[test]
    fn packet_matches_the_capture() {
        // The first O->T packet of the library's I/O packet test (sequence 1, count 1, run,
        // outputs 0x00..0x1f)
        let mut expected_byte_array: Vec<CipByte> = vec![
            0x02, 0x00, 0x02, 0x80, 0x08, 0x00, 0xd4, 0xc3, 0xb2, 0xa1, 0x01, 0x00, 0x00, 0x00,
            0xb1, 0x00, 0x26, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03,
            0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11,
            0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let connection = sample_connection();
        let outputs: [u8; 32] = core::array::from_fn(|index| index as u8);

        let raw = build(&connection, CipDataOpt::Raw(outputs.to_vec()), true).unwrap();
        let typed = build(&connection, CipDataOpt::Typed(Box::new(outputs)), true).unwrap();
        assert_eq_hex!(expected_byte_array, raw);
        assert_eq!(typed, raw);

        // Idle clears bit 0 of the run/idle header
        expected_byte_array[20] = 0x00;
        let idle = build(&connection, CipDataOpt::Raw(outputs.to_vec()), false).unwrap();
        assert_eq!(idle, expected_byte_array);
    }

    #[test]
    fn outputs_must_fit_the_connection() {
        // Fixed: exactly 32 bytes, raw or typed
        let fixed = sample_connection();
        assert!(matches!(
            build(&fixed, CipDataOpt::Raw(vec![0; 31]), true),
            Err(Error::OutputSize {
                connection_size_type: ConnectionSizeType::Fixed,
                data_size: 32,
                actual: 31
            })
        ));
        assert!(build(&fixed, CipDataOpt::Raw(vec![0; 33]), true).is_err());
        assert!(matches!(
            build(&fixed, CipDataOpt::Typed(Box::new([0u8; 31])), true),
            Err(Error::OutputSize { actual: 31, .. })
        ));

        // Variable: at most 32 bytes
        let mut variable = sample_connection();
        variable.request.o2t_network_connection_parameters =
            standard_parameters(38, ConnectionSizeType::Variable);
        assert!(build(&variable, CipDataOpt::Raw(vec![]), true).is_ok());
        assert!(build(&variable, CipDataOpt::Raw(vec![0; 33]), true).is_err());
    }
}

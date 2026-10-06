mod common;

use binrw::{BinRead, BinWrite};

use bilge::prelude::u2;

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::parameters::{RealTimeFormat, TransportClass};
use eipscanne_rs::cip::io_data::{IoData, RunIdleHeader};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::io_packet::{IoPacket, SequencedAddress};

use common::{
    IO_DATA_SIZE, O2T_CONNECTION_SIZE, O2T_NETWORK_CONNECTION_ID, T2O_CONNECTION_SIZE,
    T2O_NETWORK_CONNECTION_ID,
};

/// The data of the first packet the scanner sends: sequence count 1, the 32-bit header with the
/// run flag set, then the output bytes 0x00 to 0x1f
fn o2t_sample_io_data() -> IoData {
    IoData {
        cip_sequence_count: Some(1),
        run_idle_header: Some(
            RunIdleHeader::builder()
                .run_idle(true)
                .claim_output_ownership(false)
                .ready_for_ownership_of_outputs(u2::new(0))
                .build(),
        ),
        data: CipDataOpt::Raw((0..IO_DATA_SIZE as u8).collect()),
    }
}

/// The data of the first packet the adapter sends: sequence count 1, no header, then the input
/// bytes 0x1f down to 0x00
fn t2o_sample_io_data() -> IoData {
    IoData {
        cip_sequence_count: Some(1),
        run_idle_header: None,
        data: CipDataOpt::Raw((0..IO_DATA_SIZE as u8).rev().collect()),
    }
}

#[test]
fn test_o2t_io_packet_with_32bit_header() {
    // Wireshark leaves the data undecoded: it has not seen the Forward_Open of the connection
    /*
    EtherNet/IP (Industrial Protocol)
        Item Count: 2
            Type ID: Sequenced Address Item (0x8002)
                Length: 8
                Connection ID: 0xa1b2c3d4
                Encapsulation Sequence Number: 1
            Type ID: Connected Data Item (0x00b1)
                Length: 38
    Common Industrial Protocol, I/O
        Data: 010001000000000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f

    Hex Dump:
    0000   02 00 02 80 08 00 d4 c3 b2 a1 01 00 00 00 b1 00
    0010   26 00 01 00 01 00 00 00 00 01 02 03 04 05 06 07
    0020   08 09 0a 0b 0c 0d 0e 0f 10 11 12 13 14 15 16 17
    0030   18 19 1a 1b 1c 1d 1e 1f
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x02, 0x00, 0x02, 0x80, 0x08, 0x00, 0xd4, 0xc3, 0xb2, 0xa1, 0x01, 0x00, 0x00, 0x00, 0xb1,
        0x00, 0x26, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14,
        0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    ];

    let packet = IoPacket::new(
        O2T_NETWORK_CONNECTION_ID,
        1,
        CipDataOpt::Typed(Box::new(o2t_sample_io_data())),
    );

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    packet.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    // Read back, the Connected Data Item keeps its bytes raw; they compare equal to the typed data
    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = IoPacket::read(&mut buf_reader).unwrap();

    assert_eq!(deserialized, packet);
    assert_eq!(
        deserialized.sequenced_address(),
        Some(&SequencedAddress {
            connection_id: O2T_NETWORK_CONNECTION_ID,
            encapsulation_sequence_number: 1,
        })
    );

    // The raw bytes decoded with what the connection says about them; the length of the item is
    // the connection size
    let Some(CipDataOpt::Raw(raw_data)) = deserialized.connected_data() else {
        panic!(
            "expected a raw Connected Data Item, got {:?}",
            deserialized.connected_data()
        );
    };
    let io_data = IoData::read_le_args(
        &mut std::io::Cursor::new(raw_data),
        (
            O2T_CONNECTION_SIZE,
            TransportClass::Class1,
            RealTimeFormat::Header32Bit,
        ),
    )
    .unwrap();

    assert_eq!(io_data, o2t_sample_io_data());
}

#[test]
fn test_t2o_io_packet_modeless() {
    // Wireshark leaves the data undecoded: it has not seen the Forward_Open of the connection
    /*
    EtherNet/IP (Industrial Protocol)
        Item Count: 2
            Type ID: Sequenced Address Item (0x8002)
                Length: 8
                Connection ID: 0x12345678
                Encapsulation Sequence Number: 1
            Type ID: Connected Data Item (0x00b1)
                Length: 34
    Common Industrial Protocol, I/O
        Data: 01001f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100

    Hex Dump:
    0000   02 00 02 80 08 00 78 56 34 12 01 00 00 00 b1 00
    0010   22 00 01 00 1f 1e 1d 1c 1b 1a 19 18 17 16 15 14
    0020   13 12 11 10 0f 0e 0d 0c 0b 0a 09 08 07 06 05 04
    0030   03 02 01 00
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x02, 0x00, 0x02, 0x80, 0x08, 0x00, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x00, 0x00, 0xb1,
        0x00, 0x22, 0x00, 0x01, 0x00, 0x1f, 0x1e, 0x1d, 0x1c, 0x1b, 0x1a, 0x19, 0x18, 0x17, 0x16,
        0x15, 0x14, 0x13, 0x12, 0x11, 0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08, 0x07,
        0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
    ];

    let packet = IoPacket::new(
        T2O_NETWORK_CONNECTION_ID,
        1,
        CipDataOpt::Typed(Box::new(t2o_sample_io_data())),
    );

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    packet.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    // Read back, the Connected Data Item keeps its bytes raw; they compare equal to the typed data
    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = IoPacket::read(&mut buf_reader).unwrap();

    assert_eq!(deserialized, packet);
    assert_eq!(
        deserialized.sequenced_address(),
        Some(&SequencedAddress {
            connection_id: T2O_NETWORK_CONNECTION_ID,
            encapsulation_sequence_number: 1,
        })
    );

    // The raw bytes decoded with what the connection says about them; the length of the item is
    // the connection size
    let Some(CipDataOpt::Raw(raw_data)) = deserialized.connected_data() else {
        panic!(
            "expected a raw Connected Data Item, got {:?}",
            deserialized.connected_data()
        );
    };
    let io_data = IoData::read_le_args(
        &mut std::io::Cursor::new(raw_data),
        (
            T2O_CONNECTION_SIZE,
            TransportClass::Class1,
            RealTimeFormat::Modeless,
        ),
    )
    .unwrap();

    assert_eq!(io_data, t2o_sample_io_data());
}

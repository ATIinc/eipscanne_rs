//! The fixture end to end: file → `Eds` → `ForwardOpenRequest` → the Forward_Open bytes the library
//! tests were built on.

use binrw::BinWrite;
use hex_test_macros::prelude::*;

use bilge::prelude::u4;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionTimeoutMultiplier, PriorityTimeTick,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use eds_parser::Eds;
use scanner::implicit::eds::forward_open_request;

const SAMPLE_ADAPTER: &str = include_str!("fixtures/sample_adapter.eds");

/// `Connection1`'s Forward_Open, from the originator of the library's Forward_Open test
fn request(eds: &Eds) -> ForwardOpenRequest {
    ForwardOpenRequest {
        priority_time_tick: PriorityTimeTick::builder()
            .tick_time(u4::new(10))
            .priority(false)
            .build(),
        timeout_ticks: 5,
        o2t_network_connection_id: 0,
        t2o_network_connection_id: 0x1234_5678,
        connection_triad: ConnectionTriad {
            connection_serial_number: 0x0001,
            originator_vendor_id: 342,
            originator_serial_number: 0x0001_2345,
        },
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        ..forward_open_request(eds, &eds.connections[&1], None).unwrap()
    }
}

#[test]
fn the_exclusive_owner_connection_builds_the_captured_forward_open() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();

    // The same bytes as the library's Forward_Open request test, where Wireshark's dissection
    // of them is documented
    let expected_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x42, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x32, 0x00, 0x54, 0x02, 0x20, 0x06, 0x24,
        0x01, 0x0a, 0x05, 0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01,
        0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x26, 0x48, 0x40,
        0x42, 0x0f, 0x00, 0x22, 0x48, 0x01, 0x04, 0x20, 0x04, 0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64,
    ];
    let packet = RequestObjectAssembly::new_forward_open(0x03, request(&eds));
    let mut bytes = std::io::Cursor::new(Vec::new());
    packet.write(&mut bytes).unwrap();
    let bytes = bytes.into_inner();

    assert_eq_hex!(expected_byte_array, bytes);
}

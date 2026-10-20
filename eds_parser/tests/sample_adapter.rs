//! The fixture end to end: file → `Eds` → `ForwardOpenRequest` → the Forward_Open bytes the library
//! tests were built on.

use binrw::BinWrite;
use hex_test_macros::prelude::*;

use bilge::prelude::u4;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionTimeoutMultiplier, PriorityTimeTick, RealTimeFormat,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use eipscanne_rs::cip::path::CipPath;

use eds_parser::Eds;
use eds_parser::connection::Connection;

const SAMPLE_ADAPTER: &str = include_str!("fixtures/sample_adapter.eds");

/// The connection's Forward_Open, from the originator of the library's Forward_Open test
fn request(connection: &Connection) -> ForwardOpenRequest {
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
        o2t_requested_packet_interval: connection.o2t_requested_packet_interval,
        o2t_network_connection_parameters: connection.o2t_network_connection_parameters,
        t2o_requested_packet_interval: connection.t2o_requested_packet_interval,
        t2o_network_connection_parameters: connection.t2o_network_connection_parameters,
        transport_type_trigger: connection.transport_type_trigger,
        connection_path: connection.connection_path.clone(),
    }
}

#[test]
fn the_exclusive_owner_connection_reads_with_its_references_resolved() {
    let connection = Eds::parse(SAMPLE_ADAPTER)
        .unwrap()
        .connection(1, None)
        .unwrap();

    assert_eq!(connection.name, "Exclusive Owner");
    assert_eq!(
        connection.o2t_requested_packet_interval, 1_000_000,
        "from Param1"
    );
    assert_eq!(connection.o2t_size, 32, "from Param3");
    assert_eq!(connection.t2o_size, 32, "from Assem100");
    assert_eq!(
        connection.connection_path,
        CipPath::new_assembly_connection(0x97, 0x96, 0x64),
        "[Param2] replaced by its default"
    );
    assert_eq!(
        Eds::parse(SAMPLE_ADAPTER).unwrap().first_connection(None),
        Ok((1, connection)),
        "the first connection this scanner can open"
    );
}

#[test]
fn the_exclusive_owner_connection_builds_the_captured_forward_open() {
    let (_, connection) = Eds::parse(SAMPLE_ADAPTER)
        .unwrap()
        .first_connection(None)
        .unwrap();

    assert_eq!(connection.o2t_real_time_format, RealTimeFormat::Header32Bit);
    assert_eq!(connection.t2o_real_time_format, RealTimeFormat::Modeless);

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
    let packet = RequestObjectAssembly::new_forward_open(0x03, request(&connection));
    let mut bytes = std::io::Cursor::new(Vec::new());
    packet.write(&mut bytes).unwrap();
    let bytes = bytes.into_inner();

    assert_eq_hex!(expected_byte_array, bytes);
}

#[test]
fn the_input_only_connection_is_refused_with_what_it_offers() {
    let error = Eds::parse(SAMPLE_ADAPTER)
        .unwrap()
        .connection(2, None)
        .unwrap_err()
        .to_string();

    assert!(
        error.starts_with(
            "Connection2: this scanner opens class 1, cyclic, exclusive-owner connections; \
             this one offers TriggerAndTransport {"
        ),
        "{error}"
    );
    assert!(
        error.contains("input_only: true, exclusive_owner: false"),
        "{error}"
    );
}

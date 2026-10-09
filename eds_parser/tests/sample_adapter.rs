//! The fixture end to end: file → `Eds` → `ForwardOpenRequest` → the Forward_Open bytes the library
//! tests were built on. Plus an ignored test against a device's own EDS, named by `EDS_FILE`.

use binrw::BinWrite;
use hex_test_macros::prelude::*;

use bilge::prelude::u4;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionTimeoutMultiplier, NetworkConnectionParameters, PriorityTimeTick,
    RealTimeFormat,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

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
        o2t_requested_packet_interval: connection.o2t_requested_packet_interval().unwrap(),
        o2t_network_connection_parameters: connection
            .o2t_network_connection_parameters(false)
            .unwrap(),
        t2o_requested_packet_interval: connection.t2o_requested_packet_interval().unwrap(),
        t2o_network_connection_parameters: connection
            .t2o_network_connection_parameters(false)
            .unwrap(),
        transport_type_trigger: connection.transport_type_trigger().unwrap(),
        connection_path: connection.connection_path().unwrap(),
    }
}

#[test]
fn the_fixture_parses_with_its_references_resolved() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();

    assert_eq!(eds.params.len(), 3);
    assert_eq!(eds.assemblies.len(), 3);
    assert_eq!(eds.connections.len(), 2);

    let connection = eds.connection("Exclusive Owner").unwrap();
    assert_eq!(connection.keyword, "Connection1");
    assert_eq!(
        connection.o2t.requested_packet_interval,
        Some(1_000_000),
        "from Param1"
    );
    assert_eq!(connection.o2t.size, 32, "from Param3");
    assert_eq!(connection.t2o.size, 32, "from Assem100");
    assert_eq!(
        connection.path,
        vec![0x20, 0x04, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64],
        "[Param2] replaced by its default"
    );

    assert_eq!(eds.first_exclusive_owner_connection(), Some(connection));
    assert_eq!(eds.connection("connection2").unwrap().name, "Input Only");
}

#[test]
fn the_exclusive_owner_connection_builds_the_captured_forward_open() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();
    let connection = eds.first_exclusive_owner_connection().unwrap();

    assert_eq!(
        connection.o2t_real_time_format(),
        Ok(RealTimeFormat::Header32Bit)
    );
    assert_eq!(
        connection.t2o_real_time_format(),
        Ok(RealTimeFormat::Modeless)
    );

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
    let packet = RequestObjectAssembly::new_forward_open(0x03, request(connection));
    let mut bytes = std::io::Cursor::new(Vec::new());
    packet.write(&mut bytes).unwrap();
    let bytes = bytes.into_inner();

    assert_eq_hex!(expected_byte_array, bytes);
}

#[test]
fn the_input_only_connection_is_refused() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();
    let connection = eds.connection("Connection2").unwrap();

    let error = connection.transport_type_trigger().unwrap_err();

    assert_eq!(error.entry, "Connection2");
    assert!(error.message.contains("exclusive-owner"), "{error}");
}

/// A device's own EDS: `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored`
#[test]
#[ignore = "needs a local EDS file named by the EDS_FILE environment variable"]
fn a_local_eds_file_yields_the_expected_connection() {
    let path = std::env::var("EDS_FILE").expect("EDS_FILE names the file to read");
    // Cargo runs tests from the crate directory; a relative path is taken from the repository
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(&path);
    let text = std::fs::read_to_string(&path).expect("the EDS file is readable");
    let path = path.to_string_lossy();

    let eds = Eds::parse(&text).unwrap();
    let connection = eds.first_exclusive_owner_connection().unwrap();
    let request = request(connection);
    let o2t_real_time_format = connection.o2t_real_time_format().unwrap();
    let t2o_real_time_format = connection.t2o_real_time_format().unwrap();

    println!(
        "{}: {request:#?}, O->T {o2t_real_time_format:?}, T->O {t2o_real_time_format:?}",
        connection.name
    );

    // What the IO-HUB-4-E file describes; another device's file prints its own values above
    if path.contains("IO-HUB") {
        let NetworkConnectionParameters::Standard(o2t) = request.o2t_network_connection_parameters
        else {
            panic!("a Forward_Open has 16-bit parameters");
        };
        let NetworkConnectionParameters::Standard(t2o) = request.t2o_network_connection_parameters
        else {
            panic!("a Forward_Open has 16-bit parameters");
        };
        assert_eq!(
            request.connection_path,
            CipPath::new_assembly_connection(1, 101, 100)
        );
        // 148 bytes, the sequence count and the 32-bit header
        assert_eq!(o2t.connection_size().value(), 154);
        assert_eq!(o2t_real_time_format, RealTimeFormat::Header32Bit);
        // 228 bytes and the sequence count
        assert_eq!(t2o.connection_size().value(), 230);
        assert_eq!(t2o_real_time_format, RealTimeFormat::Modeless);
        assert_eq!(request.o2t_requested_packet_interval, 10_000);
        assert_eq!(o2t.priority(), ConnectionPriority::Scheduled);
    }
}

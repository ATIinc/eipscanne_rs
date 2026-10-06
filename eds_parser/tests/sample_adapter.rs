//! The fixture end to end: file → `Eds` → `ConnectionConfig` → the Forward_Open bytes the library
//! tests were built on. Plus an ignored test against a device's own EDS, named by `EDS_FILE`.

use binrw::BinWrite;
use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ProductionTrigger,
    RealTimeFormat, TransportClass,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::object_assembly::RequestObjectAssembly;
use scanner::implicit::{ConnectionConfig, DirectionConfig};

use eds_parser::{
    BridgeError, Eds, Finding, OriginatorSettings, check_assembly, to_connection_config,
};

const SAMPLE_ADAPTER: &str = include_str!("fixtures/sample_adapter.eds");

/// The originator of the library's Forward_Open test
fn originator() -> OriginatorSettings {
    OriginatorSettings {
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        t2o_network_connection_id: 0x1234_5678,
        connection_triad: ConnectionTriad {
            connection_serial_number: 0x0001,
            originator_vendor_id: 342,
            originator_serial_number: 0x0001_2345,
        },
        large_forward_open: false,
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
    assert_eq!(connection.configuration.target_size, Some(0));
    assert_eq!(
        connection.configuration.target_format.as_deref(),
        Some("Assem151")
    );
    assert_eq!(
        connection.path,
        vec![0x20, 0x04, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64],
        "[Param2] replaced by its default"
    );

    assert_eq!(eds.first_exclusive_owner_connection(), Some(connection));
    assert_eq!(eds.connection("connection2").unwrap().name, "Input Only");

    // The document keeps everything, including what gets no typed view
    assert_eq!(
        eds.document
            .section("TCP/IP Interface Class")
            .unwrap()
            .entry("MaxInst")
            .unwrap()
            .field(0)
            .as_integer(),
        Some(1)
    );
    assert!(
        eds.document
            .section("Device")
            .unwrap()
            .entry("IconContents")
            .unwrap()
            .field(0)
            .as_text()
            .unwrap()
            .ends_with("AAA=")
    );
}

#[test]
fn the_exclusive_owner_connection_builds_the_captured_forward_open() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();
    let connection = eds.first_exclusive_owner_connection().unwrap();

    let config = to_connection_config(connection, originator()).unwrap();

    assert_eq!(
        config,
        ConnectionConfig {
            configuration_instance: 151,
            o2t: DirectionConfig {
                connection_point: 150,
                data_size: 32,
                requested_packet_interval: 1_000_000,
                real_time_format: RealTimeFormat::Header32Bit,
                connection_size_type: ConnectionSizeType::Fixed,
            },
            t2o: DirectionConfig {
                connection_point: 100,
                data_size: 32,
                requested_packet_interval: 1_000_000,
                real_time_format: RealTimeFormat::Modeless,
                connection_size_type: ConnectionSizeType::Fixed,
            },
            transport_class: TransportClass::Class1,
            production_trigger: ProductionTrigger::Cyclic,
            priority: ConnectionPriority::Scheduled,
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            t2o_network_connection_id: 0x1234_5678,
            connection_triad: originator().connection_triad,
            large_forward_open: false,
        }
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
    let request = config.to_forward_open_request().unwrap();
    let packet = RequestObjectAssembly::new_forward_open(0x03, request);
    let mut bytes = std::io::Cursor::new(Vec::new());
    packet.write(&mut bytes).unwrap();
    let bytes = bytes.into_inner();

    assert_eq_hex!(expected_byte_array, bytes);
}

#[test]
fn the_input_only_connection_is_refused() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();
    let connection = eds.connection("Connection2").unwrap();

    let error = to_connection_config(connection, originator()).unwrap_err();

    assert!(
        matches!(&error, BridgeError::Unsupported { connection, what }
            if connection == "Connection2" && what.contains("exclusive-owner")),
        "{error}"
    );
}

#[test]
fn the_fixtures_assemblies_check_against_byte_arrays_of_their_size() {
    let eds = Eds::parse(SAMPLE_ADAPTER).unwrap();
    let connection = eds.first_exclusive_owner_connection().unwrap();
    let inputs = eds
        .assembly(connection.t2o.format.as_deref().unwrap())
        .unwrap();
    let outputs = eds
        .assembly(connection.o2t.format.as_deref().unwrap())
        .unwrap();

    // Both are one 256-bit member without a param: only the size and coverage steps apply
    assert_eq!(
        (inputs.instance(), outputs.instance()),
        (Some(100), Some(150))
    );
    assert_eq!(check_assembly::<[u8; 32]>(inputs), Ok(vec![]));
    assert_eq!(check_assembly::<[u8; 32]>(outputs), Ok(vec![]));
    assert_eq!(
        check_assembly::<[u8; 31]>(inputs).unwrap_err().findings,
        vec![
            Finding::ReadSize { read: 31, size: 32 },
            Finding::WriteSize {
                written: 31,
                size: 32
            }
        ]
    );
}

/// A device's own EDS: `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -p eds_parser -- --ignored`
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
    let config = to_connection_config(connection, originator()).unwrap();

    println!("{}: {config:#?}", connection.name);

    // What the IO-HUB-4-E file describes; another device's file prints its own values above
    if path.contains("IO-HUB") {
        assert_eq!(config.configuration_instance, 1);
        assert_eq!(
            (config.o2t.connection_point, config.o2t.data_size),
            (101, 148)
        );
        assert_eq!(config.o2t.real_time_format, RealTimeFormat::Header32Bit);
        assert_eq!(
            (config.t2o.connection_point, config.t2o.data_size),
            (100, 228)
        );
        assert_eq!(config.t2o.real_time_format, RealTimeFormat::Modeless);
        assert_eq!(config.o2t.requested_packet_interval, 10_000);
        assert_eq!(config.priority, ConnectionPriority::Scheduled);
    }
}

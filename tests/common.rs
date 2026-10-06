//! Values shared by the integration tests, taken from the captures the tests are built on.
//! Each test binary compiles this module on its own and uses only some of it.
// Cargo also compiles this file as an integration test target of its own, where nothing uses
// these items.
#![allow(dead_code)]

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::cip::types::{CipUint, CipUsint};

/// Session handle of the read-identity captures (Register Session, Identity, Unregister Session)
pub const IDENTITY_SESSION_HANDLE: CipUdint = 0x06;

/// Session handle of the ClearLink assembly and I/O connection captures
pub const CLEARLINK_IO_SESSION_HANDLE: CipUdint = 0x03;

/// Assembly instances of the ClearLink
pub const CLEARLINK_CONFIG_ASSEMBLY_INSTANCE: u8 = 0x96;
pub const CLEARLINK_OUTPUT_ASSEMBLY_INSTANCE: u8 = 0x70;

/// Connection path of the Forward_Open and Forward_Close packets
pub const FORWARD_OPEN_CONFIG_INSTANCE: u8 = 0x97;
pub const FORWARD_OPEN_O2T_CONNECTION_POINT: u8 = 0x96;
pub const FORWARD_OPEN_T2O_CONNECTION_POINT: u8 = 0x64;

/// Originator (this scanner) identity in the Forward_Open and Forward_Close packets
pub const ORIGINATOR_VENDOR_ID: CipUint = 342;
pub const ORIGINATOR_SERIAL_NUMBER: CipUdint = 0x0001_2345;
pub const CONNECTION_SERIAL_NUMBER: CipUint = 0x0001;

/// Unconnected request timing of the Forward_Open and Forward_Close packets
pub const TICK_TIME: u8 = 10;
pub const TIMEOUT_TICKS: CipUsint = 5;

/// O->T network connection ID of the Forward_Open request: 0, the target picks it
pub const REQUESTED_O2T_NETWORK_CONNECTION_ID: CipUdint = 0;
/// O->T network connection ID the target picked in the Forward_Open reply
pub const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xa1b2_c3d4;
pub const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;

/// Requested (and granted) packet interval of both directions, in microseconds
pub const RPI_MICROSECONDS: CipUdint = 1_000_000;

/// I/O data bytes of each direction, and the resulting connection sizes: O->T adds the 2-byte
/// sequence count and the 4-byte 32-bit header, T->O only the sequence count
pub const IO_DATA_SIZE: u16 = 32;
pub const O2T_CONNECTION_SIZE: u16 = 38;
pub const T2O_CONNECTION_SIZE: u16 = 34;

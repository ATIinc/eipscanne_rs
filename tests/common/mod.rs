//! Values shared by the integration tests, taken from the captures the tests are built on.
//! Each test binary compiles this module on its own and uses only some of it.
#![allow(dead_code)]

use eipscanne_rs::cip::types::CipUdint;

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

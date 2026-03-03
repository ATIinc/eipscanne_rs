use std::time::Duration;

pub const SOCKET_OPEN_TIMEOUT: Duration = Duration::from_secs(3);

// 0xAF12 = 44818 decimal
pub const ETHERNET_IP_PORT: u16 = 0xAF12;

// The Clearlink Assembly Instance IDs from the datasheet:
// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=29
pub const ASSEMBLY_OBJECT_ID: u8 = 0x4;
pub const ASSEMBLY_ATTRIBUTE_ID: u8 = 0x3;
pub const CONFIG_ASSEMBLY_INSTANCE_ID: u8 = 0x96;
pub const INPUT_ASSEMBLY_INSTANCE_ID: u8 = 0x64;
pub const OUTPUT_ASSEMBLY_INSTANCE_ID: u8 = 0x70;

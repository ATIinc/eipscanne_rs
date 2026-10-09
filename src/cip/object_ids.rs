//! Class, instance and attribute IDs of the CIP objects addressed by this crate

/// Identity object
pub const IDENTITY_CLASS_ID: u16 = 0x01;
pub const IDENTITY_INSTANCE_ID: u16 = 0x01;

/// Message Router object, the end point of a class 3 connection
pub const MESSAGE_ROUTER_CLASS_ID: u8 = 0x02;
pub const MESSAGE_ROUTER_INSTANCE_ID: u8 = 0x01;

/// Assembly object
pub const ASSEMBLY_CLASS_ID: u8 = 0x04;
/// The Data attribute of an Assembly instance
pub const ASSEMBLY_DATA_ATTRIBUTE_ID: u8 = 0x03;

/// Connection Manager object, addressed by Forward_Open and Forward_Close
pub const CONNECTION_MANAGER_CLASS_ID: u8 = 0x06;
pub const CONNECTION_MANAGER_INSTANCE_ID: u8 = 0x01;

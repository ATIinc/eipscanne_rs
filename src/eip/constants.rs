use crate::cip::types::{CipByte, CipUdint, CipUint};

pub const SENDER_CONTEXT_SIZE: usize = 8;

/// TCP port of the EtherNet/IP encapsulation protocol (44818)
pub const ETHERNET_IP_TCP_PORT: u16 = 0xAF12;

/// UDP port of class 1 (implicit) I/O traffic (2222)
pub const ETHERNET_IP_IO_UDP_PORT: u16 = 0x08AE;

/// Session handle of packets sent before a session exists (Register Session)
pub const UNREGISTERED_SESSION_HANDLE: CipUdint = 0;

/// Sender context of the packets this scanner sends
pub const EMPTY_SENDER_CONTEXT: [CipByte; SENDER_CONTEXT_SIZE] = [0x00; SENDER_CONTEXT_SIZE];

/// Options of the encapsulation header: none are defined, so always 0
pub const DEFAULT_ENCAPSULATION_OPTIONS: CipUdint = 0;

/// Interface handle of Send RR Data and Send Unit Data: always 0 for CIP
pub const CIP_INTERFACE_HANDLE: CipUdint = 0;

/// Timeout of Send RR Data: 0 relies on the CIP timeout, which is always the case for CIP. Send
/// Unit Data always has 0.
pub const NO_ENCAPSULATION_TIMEOUT: CipUint = 0;

/// Items every Send RR Data packet starts with: an address item followed by a data item
pub const SEND_RR_DATA_REQUIRED_ITEM_COUNT: CipUint = 2;

/// Items of a Send Unit Data packet: the Connected Address Item followed by the Connected Data Item
pub const SEND_UNIT_DATA_ITEM_COUNT: CipUint = 2;

/// Protocol version requested by Register Session
pub const ENCAPSULATION_PROTOCOL_VERSION: CipUint = 1;

/// Option flags of Register Session: none are defined, so always 0
pub const REGISTER_SESSION_OPTION_FLAGS: CipUint = 0;

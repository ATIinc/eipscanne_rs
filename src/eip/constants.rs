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

/// Interface handle of Send RR Data: always 0 for CIP
pub const CIP_INTERFACE_HANDLE: CipUdint = 0;

/// Timeout of Send RR Data: 0 relies on the CIP timeout, which is always the case for CIP
pub const NO_ENCAPSULATION_TIMEOUT: CipUint = 0;

/// Protocol version requested by Register Session
pub const ENCAPSULATION_PROTOCOL_VERSION: CipUint = 1;

/// Option flags of Register Session: none are defined, so always 0
pub const REGISTER_SESSION_OPTION_FLAGS: CipUint = 0;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::mem;

use super::cip_data::CipDataPacket;
use crate::cip::types::{CipByte, CipUdint, CipUint};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[repr(u16)]
pub enum EncapsCommand {
    // Needs to be of type CipUint (u16)
    NOP = 0,
    ListServices = 0x0004,
    ListIdentity = 0x0063,
    ListInterfaces = 0x0064,
    RegisterSession = 0x0065,
    UnRegisterSession = 0x0066,
    SendRrData = 0x006F,
    SendUnitData = 0x0070,
    IndicateStatus = 0x0072,
    Cancel = 0x0073,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[repr(u32)]
pub enum EncapsStatusCode {
    // Needs to be of type CipUdint (u32)
    Success = 0x0000,
    UnsupportedCommand = 0x0001,
    InsufficientMemory = 0x0002,
    InvalidFormatOrData = 0x0003,
    InvalidSessionHandle = 0x0064,
    UnsupportedProtocolVersion = 0x0069,
}

const DEFAULT_PACKET_OPTIONS: CipUint = 8;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct PacketData<T> {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,
    pub cip_data_packet: CipDataPacket<T>,
}

// These should be equal
const ENCAPSULATED_HEADER_SIZE: usize = 24;
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct EncapsulatedHeader {
    pub command: EncapsCommand,
    pub length: CipUint,
    pub session_handle: CipUdint,
    pub status_code: EncapsStatusCode,
    pub sender_context: CipUint,
    pub options: CipUint,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct EncapsulatedPacket<T> {
    pub header: EncapsulatedHeader,
    pub data: PacketData<T>,
}

impl<T: Serialize> EncapsulatedPacket<T> {
    pub fn new(
        session_handle: CipUdint,
        timeout: CipUint,
        cip_data_packet: CipDataPacket<T>,
    ) -> Self {
        // with explicit messaging, there is no interface handle
        let interface_handle: CipUdint = 0;

        let packet_data = PacketData {
            interface_handle,
            timeout,
            cip_data_packet,
        };

        let packet_size: CipUint = mem::size_of_val(&packet_data) as CipUint;

        EncapsulatedPacket {
            header: EncapsulatedHeader {
                command: EncapsCommand::SendRrData,
                length: packet_size,
                session_handle,
                status_code: EncapsStatusCode::Success,
                sender_context: DEFAULT_PACKET_OPTIONS,
                options: 0,
            },
            data: packet_data,
        }
    }
}

// create a default implementation for EncapsulatedPacket with CipByte
impl EncapsulatedPacket<CipByte> {
    pub fn new_empty(session_handle: CipUdint, timeout: CipUint) -> Self {
        EncapsulatedPacket::new(session_handle, timeout, CipDataPacket::new_empty())
    }
}

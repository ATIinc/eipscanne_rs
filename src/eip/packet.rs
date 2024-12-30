use serde::{de::DeserializeOwned, Deserialize, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::mem;

use super::cip_data::CipDataPacket;
use crate::cip::types::{CipByte, CipUdint, CipUint};

#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq, Copy, Clone)]
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

#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq, Copy, Clone)]
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

// Enum definition with `Serialize` and `Deserialize` traits.
#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq, Copy, Clone)]
#[repr(u16)]
pub enum CommonPacketItemId {
    NullAddr = 0x0000,
    ListIdentity = 0x000C,
    ConnectionAddressItem = 0x00A1,
    ConnectedTransportPacket = 0x00B1,
    UnconnectedMessage = 0x00B2,
    O2TSockAddrInfo = 0x8000,
    T2OSockAddrInfo = 0x8001,
    SequencedAddressItem = 0x8002,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
pub struct CommonPacketDescriptor {
    pub type_id: CommonPacketItemId,
    pub packet_length: CipUint,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,
    pub cip_data_packet: [CommonPacketDescriptor; 2],
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RegisterData {
    pub protocol_version: CipUint,
    pub option_flags: CipUint,
}

#[derive(Deserialize, Debug, PartialEq)]
pub enum CommandSpecificData {
    RegisterSession(RegisterData),
    SendRrData(PacketData),
}

impl Serialize for CommandSpecificData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            CommandSpecificData::RegisterSession(register_data) => {
                register_data.serialize(serializer)
            }
            CommandSpecificData::SendRrData(packet_data) => packet_data.serialize(serializer),
        }
    }
}

const SENDER_CONTEXT_SIZE: usize = 8;

// These should be equal
// const ENCAPSULATED_HEADER_SIZE: usize = 24;
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EncapsulatedHeader {
    pub command: EncapsCommand,
    pub length: CipUint,
    pub session_handle: CipUdint,
    pub status_code: EncapsStatusCode,
    pub sender_context: [CipByte; SENDER_CONTEXT_SIZE],
    pub options: CipUdint,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct EncapsulatedPacket {
    pub header: EncapsulatedHeader,
    pub command_data: CommandSpecificData,
    // pub data: CipDataPacket<CipByte>,
}

// impl EncapsulatedPacket
// where
//     T: Serialize + DeserializeOwned,
// {
//     pub fn new(
//         command: EncapsCommand,
//         session_handle: CipUdint,
//         command_specific_data: CommandSpecificData,
//     ) -> Self {
//         // with explicit messaging, there is no interface handle
//         let data_packet_size = command_specific_data.get_size() as CipUint;

//         EncapsulatedPacket {
//             header: EncapsulatedHeader {
//                 command,
//                 length: data_packet_size,
//                 session_handle,
//                 status_code: EncapsStatusCode::Success,
//                 sender_context: [0x00; SENDER_CONTEXT_SIZE],
//                 options: 0x00,
//             },
//             data: command_specific_data,
//         }
//     }

//     pub fn new_data(
//         session_handle: CipUdint,
//         timeout: CipUint,
//         cip_data_packet: CipDataPacket<T>,
//     ) -> Self {
//         EncapsulatedPacket::new(
//             EncapsCommand::SendRrData,
//             session_handle,
//             CommandSpecificData::SendRrData(PacketData {
//                 interface_handle: 0,
//                 timeout,
//                 cip_data_packet,
//             }),
//         )
//     }
// }

// create a default implementation for EncapsulatedPacket with CipByte
// impl EncapsulatedPacket<CipByte> {
//     pub fn new_registration() -> Self {
//         EncapsulatedPacket::new(
//             EncapsCommand::RegisterSession,
//             0,
//             CommandSpecificData::RegisterSession(RegisterData {
//                 protocol_version: 1,
//                 option_flags: 0,
//             }),
//         )
//     }
// }

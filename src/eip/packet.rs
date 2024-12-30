use std::fmt;
use std::io;
use std::mem;

use serde::de::{Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::cip::{
    message::MessageRouter,
    types::{CipByte, CipUdint, CipUint},
};

// TODO: Investigate replacing all deserialize calls with bincode::Decode and bincode::Encode

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

// Convert from a MessageRouter to the CommonPacketDescriptors
impl MessageRouter {
    pub fn generate_packet_descriptors(&self) -> [CommonPacketDescriptor; 2] {
        [
            CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: 0,
            },
            CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: self.byte_size(),
            },
        ]
    }
}

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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,
    pub item_count: CipUint,
    pub cip_data_packets: [CommonPacketDescriptor; 2],
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RegisterData {
    pub protocol_version: CipUint,
    pub option_flags: CipUint,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum CommandSpecificData {
    RegisterSession(RegisterData),
    SendRrData(PacketData),
}

impl CommandSpecificData {
    pub fn byte_size(&self) -> CipUint {
        match self {
            CommandSpecificData::RegisterSession(register_data) => {
                mem::size_of_val(register_data) as CipUint
            }
            CommandSpecificData::SendRrData(packet_data) => {
                let eip_component_size = mem::size_of_val(packet_data) as CipUint;
                let mut cip_component_size = 0;
                for descriptor in packet_data.cip_data_packets {
                    cip_component_size += descriptor.packet_length;
                }

                eip_component_size + cip_component_size
            }
        }
    }
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

// TODO: Implement EncapsulatedPacket deserialization
//  - First read the header, then decide how to handle the remaining bytes
// Maybe: https://stackoverflow.com/questions/63306229/how-to-pass-options-to-rusts-serde-that-can-be-accessed-in-deserializedeseria

#[derive(Serialize, Debug, PartialEq)]
pub struct EncapsulatedPacket {
    pub header: EncapsulatedHeader,
    pub command_data: CommandSpecificData,
}

impl EncapsulatedPacket {
    pub fn new(
        command: EncapsCommand,
        session_handle: CipUdint,
        command_specific_data: CommandSpecificData,
    ) -> Self {
        // with explicit messaging, there is no interface handle
        let data_packet_size = command_specific_data.byte_size();

        EncapsulatedPacket {
            header: EncapsulatedHeader {
                command,
                length: data_packet_size,
                session_handle,
                status_code: EncapsStatusCode::Success,
                sender_context: [0x00; SENDER_CONTEXT_SIZE],
                options: 0x00,
            },
            command_data: command_specific_data,
        }
    }

    pub fn new_cip(
        session_handle: CipUdint,
        timeout: CipUint,
        message_router: &MessageRouter,
    ) -> Self {
        let package_descriptors = message_router.generate_packet_descriptors();

        EncapsulatedPacket::new(
            EncapsCommand::SendRrData,
            session_handle,
            CommandSpecificData::SendRrData(PacketData {
                interface_handle: 0,
                timeout,
                item_count: package_descriptors.len() as CipUint,
                cip_data_packets: package_descriptors,
            }),
        )
    }
}

// create a default implementation for EncapsulatedPacket with CipByte
impl EncapsulatedPacket {
    pub fn new_registration() -> Self {
        EncapsulatedPacket::new(
            EncapsCommand::RegisterSession,
            0,
            CommandSpecificData::RegisterSession(RegisterData {
                protocol_version: 1,
                option_flags: 0,
            }),
        )
    }
}

struct EncapsulatedPacketVisitor;

impl EncapsulatedPacketVisitor {
    fn deserialize_command_specific_data<E>(
        self,
        v: Vec<u8>,
        command_type: EncapsCommand,
    ) -> Result<CommandSpecificData, E>
    where
        E: serde::de::Error,
    {
        match command_type {
            EncapsCommand::RegisterSession => {
                let registration_data: RegisterData = bincode::deserialize(&v).unwrap();
                Ok(CommandSpecificData::RegisterSession(registration_data))
            }
            EncapsCommand::SendRrData => {
                let packet_data: PacketData = bincode::deserialize(&v).unwrap();
                Ok(CommandSpecificData::SendRrData(packet_data))
            }
            _ => Err(E::custom(format!(
                "Unable to deserialize the provided EncapsCommand: {:?}",
                command_type
            ))),
        }
    }
}

impl<'de> Visitor<'de> for EncapsulatedPacketVisitor {
    type Value = EncapsulatedPacket;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a serialized EncapsulatedPacket")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let header_length = mem::size_of::<EncapsulatedHeader>();

        let (header_bytes, command_bytes) = v.split_at(header_length);
        let encapsulated_header: EncapsulatedHeader = bincode::deserialize(header_bytes).unwrap();

        let encapsulated_command: Result<CommandSpecificData, E> = self
            .deserialize_command_specific_data(command_bytes.to_vec(), encapsulated_header.command);

        Ok(EncapsulatedPacket {
            header: encapsulated_header,
            command_data: encapsulated_command.unwrap(),
        })
    }
}

impl<'de> Deserialize<'de> for EncapsulatedPacket {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(EncapsulatedPacketVisitor)
    }
}

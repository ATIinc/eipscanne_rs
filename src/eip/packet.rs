use serde::{de::DeserializeOwned, Deserialize, Serialize, Serializer};
use std::mem;

use super::cip_data::CipDataPacket;
use crate::cip::types::{CipByte, CipUdint, CipUint};

#[derive(Deserialize, Debug, PartialEq, Copy, Clone)]
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

impl Serialize for EncapsCommand {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Use the underlying numeric value of the enum for serialization
        let value = *self as CipUint;
        serializer.serialize_u16(value)
    }
}

#[derive(Deserialize, Debug, PartialEq, Copy, Clone)]
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

impl Serialize for EncapsStatusCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Use the underlying numeric value of the enum for serialization
        let value = *self as CipUdint;
        serializer.serialize_u32(value)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct PacketData<T> {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,
    pub cip_data_packet: CipDataPacket<T>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RegisterData {
    pub protocol_version: CipUint,
    pub option_flags: CipUint,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub enum CommandSpecificData<T> {
    RegisterSession(RegisterData),
    SendRrData(PacketData<T>),
}

impl<T> CommandSpecificData<T>
where
    T: Serialize + DeserializeOwned,
{
    // Method to get the size of the contained value in the enum
    pub fn get_size(&self) -> usize {
        match self {
            CommandSpecificData::RegisterSession(register_data) => {
                // Calculate the size of RegisterData
                mem::size_of_val(register_data)
            }
            CommandSpecificData::SendRrData(packet_data) => {
                // Calculate the size of CipDataPacket<T>
                mem::size_of_val(packet_data)
            }
        }
    }
}

impl<T> Serialize for CommandSpecificData<T>
where
    T: Serialize + DeserializeOwned,
{
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
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct EncapsulatedPacket<T> {
    pub header: EncapsulatedHeader,
    pub data: CommandSpecificData<T>,
}

impl<T> EncapsulatedPacket<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn new(
        command: EncapsCommand,
        session_handle: CipUdint,
        command_specific_data: CommandSpecificData<T>,
    ) -> Self {
        // with explicit messaging, there is no interface handle
        let data_packet_size = command_specific_data.get_size() as CipUint;

        EncapsulatedPacket {
            header: EncapsulatedHeader {
                command,
                length: data_packet_size,
                session_handle,
                status_code: EncapsStatusCode::Success,
                sender_context: [0x00; SENDER_CONTEXT_SIZE],
                options: 0x00,
            },
            data: command_specific_data,
        }
    }

    pub fn new_data(
        session_handle: CipUdint,
        timeout: CipUint,
        cip_data_packet: CipDataPacket<T>,
    ) -> Self {
        EncapsulatedPacket::new(
            EncapsCommand::SendRrData,
            session_handle,
            CommandSpecificData::SendRrData(PacketData {
                interface_handle: 0,
                timeout,
                cip_data_packet,
            }),
        )
    }
}

// create a default implementation for EncapsulatedPacket with CipByte
impl EncapsulatedPacket<CipByte> {
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

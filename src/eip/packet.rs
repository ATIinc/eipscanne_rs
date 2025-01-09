// use std::fmt;
use std::mem;

use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // trait for reading
    BinWrite, // trait for writing
};

// use bincode::ErrorKind;
// use bincode::Result as BincodeResult;

// use crate::cip::message;
use crate::cip::{
    message::MessageRouter,
    types::{CipByte, CipUdint, CipUint},
};

// TODO: Investigate replacing all deserialize calls with bincode::Decode and bincode::Encode

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUint)]
#[bw(little, repr = CipUint)]
#[derive(Debug, PartialEq, Copy, Clone)]
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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
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

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUint)]
#[bw(little, repr = CipUint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EnIpCommand {
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

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUdint)]
#[bw(little, repr = CipUdint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EncapsStatusCode {
    // Needs to be of type CipUdint (u32)
    Success = 0x0000,
    UnsupportedCommand = 0x0001,
    InsufficientMemory = 0x0002,
    InvalidFormatOrData = 0x0003,
    InvalidSessionHandle = 0x0064,
    UnsupportedProtocolVersion = 0x0069,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct PacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,
    pub item_count: CipUint, // will always be 2
    pub cip_data_packets: [CommonPacketDescriptor; 2],
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RegisterData {
    pub protocol_version: CipUint,
    pub option_flags: CipUint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub enum CommandSpecificData {
    RegisterSession(RegisterData),
    SendRrData(PacketData),
}

impl CommandSpecificData {
    pub fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: 1,
            option_flags: 0,
        })
    }

    pub fn new_request(
        interface_handle: CipUdint,
        timeout: CipUint,
        message_router: &MessageRouter,
    ) -> Self {
        let package_descriptors = message_router.generate_packet_descriptors();

        Self::SendRrData(PacketData {
            interface_handle,
            timeout,
            item_count: message_router.byte_size(),
            cip_data_packets: package_descriptors,
        })
    }

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

const SENDER_CONTEXT_SIZE: usize = 8;

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct EncapsulationHeader {
    pub command: EnIpCommand,
    pub length: CipUint,
    pub session_handle: CipUdint,
    pub status_code: EncapsStatusCode,
    pub sender_context: [CipByte; SENDER_CONTEXT_SIZE],
    pub options: CipUdint,
}

// TODO: Implement EncapsulatedPacket deserialization
//  - First read the header, then decide how to handle the remaining bytes
// Maybe: https://stackoverflow.com/questions/63306229/how-to-pass-options-to-rusts-serde-that-can-be-accessed-in-deserializedeseria

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EnIpPacketRegisterDescription {
    pub header: EncapsulationHeader,
    pub register_description: RegisterData,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EnIpPacketPacketDescription {
    pub header: EncapsulationHeader,
    pub packet_description: PacketData,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EnIpPacketDescription {
    pub header: EncapsulationHeader,
    pub command_data: CommandSpecificData,
}

impl EnIpPacketDescription {
    pub fn new(
        command: EnIpCommand,
        session_handle: CipUdint,
        command_specific_data: CommandSpecificData,
    ) -> Self {
        // with explicit messaging, there is no interface handle
        let data_packet_size = command_specific_data.byte_size();

        EnIpPacketDescription {
            header: EncapsulationHeader {
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
        package_descriptors: [CommonPacketDescriptor; 2],
    ) -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::SendRrData,
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
impl EnIpPacketDescription {
    pub fn new_registration() -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::RegisterSession,
            0,
            CommandSpecificData::RegisterSession(RegisterData {
                protocol_version: 1,
                option_flags: 0,
            }),
        )
    }
}

// pub fn deserialize_packet_from<R>(mut reader: R) -> BincodeResult<EnIpPacketDescription>
// where
//     R: std::io::BufRead,
// {
//     // Deserialize the header first
//     let deserialized_header: EncapsulationHeader = bincode::deserialize_from(&mut reader).unwrap();

//     // Deserialize the command-specific data based on the command in the header
//     let deserialize_command_data = match deserialized_header.command {
//         EnIpCommand::RegisterSession => {
//             // Deserialize the specific data related to RegisterSession
//             let deserialized_register_data: RegisterData =
//                 bincode::deserialize_from(&mut reader).unwrap();
//             Ok(CommandSpecificData::RegisterSession(
//                 deserialized_register_data,
//             ))
//         }
//         _ => Err(Box::new(ErrorKind::Custom(
//             "Command not supported".to_string(),
//         ))),
//     };

//     // Assemble the EncapsulatedPacket
//     let deserialized_packet = EnIpPacketDescription {
//         header: deserialized_header,
//         command_data: deserialize_command_data?,
//     };

//     Ok(deserialized_packet)
// }

// use std::fmt;
use std::mem;

use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // trait for reading
    BinWrite, // trait for writing
};

use crate::cip::{
    message::MessageRouter,
    types::{CipByte, CipUdint, CipUint},
};

use crate::eip::constants as eip_constants;

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

pub fn generate_packet_descriptors(packet_size: usize) -> [CommonPacketDescriptor; 2] {
    [
        CommonPacketDescriptor {
            type_id: CommonPacketItemId::NullAddr,
            packet_length: 0,
        },
        CommonPacketDescriptor {
            type_id: CommonPacketItemId::UnconnectedMessage,
            packet_length: packet_size.try_into().unwrap(),
        },
    ]
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
pub struct RRPacketData {
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
#[br(import(commandType: EnIpCommand))]
pub enum CommandSpecificData {
    #[br(pre_assert(commandType == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    #[br(pre_assert(commandType == EnIpCommand::SendRrData))]
    SendRrData(RRPacketData),
}

// ======= Start of CommandSpecificData impl ========

impl CommandSpecificData {
    pub fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: 1,
            option_flags: 0,
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

    pub fn new_request<T>(interface_handle: CipUdint, timeout: CipUint, request_size: usize) -> Self
    where
        T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
    {
        Self::SendRrData(RRPacketData {
            interface_handle,
            timeout,
            item_count: request_size.try_into().unwrap(),
            cip_data_packets: generate_packet_descriptors(request_size),
        })
    }
}

// ======= Start of CommandSpecificData impl ========




#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct EncapsulationHeader {
    pub command: EnIpCommand,
    pub length: CipUint,
    pub session_handle: CipUdint,
    pub status_code: EncapsStatusCode,
    pub sender_context: [CipByte; eip_constants::SENDER_CONTEXT_SIZE],
    pub options: CipUdint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EnIpPacketDescription {
    pub header: EncapsulationHeader,

    #[br(args(header.command,))]
    pub command_specific_data: CommandSpecificData,
    /* Passes the command field of the header to the command_specific_data field for binary reading/writing */
}


// ======= Start of EnIpPacketDescription impl ========

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
                sender_context: [0x00; eip_constants::SENDER_CONTEXT_SIZE],
                options: 0x00,
            },
            command_specific_data,
        }
    }

    pub fn new_registration_description() -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::RegisterSession,
            0,
            CommandSpecificData::RegisterSession(RegisterData {
                protocol_version: 1,
                option_flags: 0,
            }),
        )
    }

    pub fn new_cip_description<T>(
        session_handle: CipUdint,
        timeout: CipUint,
        message_router: &MessageRouter<T>,
    ) -> Self
    where
        T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
    {
        let package_descriptors = generate_packet_descriptors(message_router.byte_size());

        EnIpPacketDescription::new(
            EnIpCommand::SendRrData,
            session_handle,
            CommandSpecificData::SendRrData(RRPacketData {
                interface_handle: 0,
                timeout,
                item_count: package_descriptors.len() as CipUint,
                cip_data_packets: package_descriptors,
            }),
        )
    }
}

// ======= Start of EnIpPacketDescription impl ========
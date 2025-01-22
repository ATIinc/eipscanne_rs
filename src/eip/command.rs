use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // trait for reading
    BinWrite, // trait for writing
};

use crate::cip::types::{CipUdint, CipUint};

use super::description::{CommonPacketDescriptor, CommonPacketItemId};

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
#[bw(import(provided_packet_length: u16))]
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    #[bw(calc = 2)]
    pub _item_count: CipUint,
    pub empty_data_packet: CommonPacketDescriptor,

    #[bw(args(Some(provided_packet_length)))]
    pub unconnected_data_packet: CommonPacketDescriptor,
}

// ======= Start of RRPacketData impl ========

impl RRPacketData {
    /// WARNING: Exposed only for testing. All normal declarations should be made with Self::new(...)
    pub fn test_with_size(
        interface_handle: CipUdint,
        timeout: CipUint,
        unconnected_length: Option<u16>,
    ) -> Self {
        RRPacketData {
            interface_handle,
            timeout,
            empty_data_packet: CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: Some(0),
            },
            unconnected_data_packet: CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: unconnected_length,
            },
        }
    }

    pub fn new(interface_handle: CipUdint, timeout: CipUint) -> Self {
        Self::test_with_size(interface_handle, timeout, None)
    }
}

// ^^^^^^^^ End of RRPacketData impl ^^^^^^^^

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
#[br(import(command_type: EnIpCommand))]
#[bw(import(provided_packet_length: u16))]
pub enum CommandSpecificData {
    #[br(pre_assert(command_type == EnIpCommand::UnRegisterSession))]
    UnregisterSession,

    #[br(pre_assert(command_type == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    #[br(pre_assert(command_type == EnIpCommand::SendRrData))]
    SendRrData(#[bw(args(provided_packet_length))] RRPacketData),
    /*  When reading -- make sure the provided command_type matches.
    When writing -- make sure the packet length is passed on */
}

// ======= Start of CommandSpecificData impl ========

impl CommandSpecificData {
    pub fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: 1,
            option_flags: 0,
        })
    }

    pub fn new_request(interface_handle: CipUdint, timeout: CipUint) -> Self {
        Self::SendRrData(RRPacketData::new(interface_handle, timeout))
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

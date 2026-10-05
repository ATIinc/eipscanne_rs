use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    binrw,    // #[binrw] attribute
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

/// Number of Common Packet Format items that every SendRRData packet carries: the address item and the data item
pub const BASE_ITEM_COUNT: CipUint = 2;

/// Values that are only known once the data following the command specific data has been serialized:
/// `(unconnected_data_length, trailing_items_length, item_count)`
///
/// * `unconnected_data_length`: length of the Unconnected Data Item (the CIP message)
/// * `trailing_items_length`: length of every item written after the CIP message
/// * `item_count`: total number of items in the Common Packet Format
pub type PacketWriteArgs = (u16, u16, u16);

#[binrw::writer(writer: writer, endian)]
fn item_count_writer(obj: &CipUint, provided_item_count: u16) -> binrw::BinResult<()> {
    // Without a provided count (e.g. when the struct is written on its own) keep the field value
    if provided_item_count == 0 {
        return obj.write_options(writer, endian, ());
    }

    provided_item_count.write_options(writer, endian, ())
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[bw(import(unconnected_data_length: u16, _trailing_items_length: u16, provided_item_count: u16))]
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    // Read from the wire; written from the number of items actually serialized
    #[bw(args(provided_item_count), write_with = item_count_writer)]
    pub item_count: CipUint,
    pub empty_data_packet: CommonPacketDescriptor,

    #[bw(args(Some(unconnected_data_length)))]
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
            item_count: BASE_ITEM_COUNT,
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

    /// Number of items that follow the data item
    pub fn additional_item_count(&self) -> usize {
        self.item_count.saturating_sub(BASE_ITEM_COUNT) as usize
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
#[bw(import(unconnected_data_length: u16, trailing_items_length: u16, item_count: u16))]
pub enum CommandSpecificData {
    #[br(pre_assert(command_type == EnIpCommand::UnRegisterSession))]
    UnregisterSession,

    #[br(pre_assert(command_type == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    #[br(pre_assert(command_type == EnIpCommand::SendRrData))]
    SendRrData(
        #[bw(args(unconnected_data_length, trailing_items_length, item_count))] RRPacketData,
    ),
    /*  When reading -- make sure the provided command_type matches.
    When writing -- make sure the packet lengths are passed on */
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

    /// Number of Common Packet Format items that follow the data item (0 for commands without one)
    pub fn additional_item_count(&self) -> usize {
        match self {
            CommandSpecificData::SendRrData(rr_data) => rr_data.additional_item_count(),
            _ => 0,
        }
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

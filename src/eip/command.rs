use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    binrw,    // #[binrw] attribute
};

use crate::cip::types::{CipUdint, CipUint};

use crate::cip::message::CipMessage;

use super::constants as eip_constants;
use super::description::{CommonPacketDescriptor, CommonPacketItemId, serialized_length};
use super::sockaddr::SockaddrInfoItems;

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUint)]
#[bw(little, repr = CipUint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EnIpCommand {
    NOP = 0,
    ListServices = 0x0004,
    ListIdentity = 0x0063,
    ListInterfaces = 0x0064,
    RegisterSession = 0x0065,
    UnRegisterSession = 0x0066,
    /// Send Request/Reply Data: an unconnected request, answered by a reply, see [`RRPacketData`]
    SendRrData = 0x006F,
    /// Send Unit Data: a connected message, sent without a reply
    SendUnitData = 0x0070,
    IndicateStatus = 0x0072,
    Cancel = 0x0073,
}

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUdint)]
#[bw(little, repr = CipUdint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EncapsStatusCode {
    Success = 0x0000,
    UnsupportedCommand = 0x0001,
    InsufficientMemory = 0x0002,
    InvalidFormatOrData = 0x0003,
    InvalidSessionHandle = 0x0064,
    UnsupportedProtocolVersion = 0x0069,
}

/// Command specific data of a Send RR Data packet (Wireshark: "Command Specific Data").
///
/// "RR" stands for Request/Reply: Send RR Data (command 0x006F) carries one unconnected request
/// from the originator (this scanner) to the target, and the target answers with a Send RR Data
/// packet of the same layout carrying the reply. With CIP, the request is a Message Router request
/// (an unconnected explicit message, e.g. Get Attribute Single or Forward_Open) and the reply is
/// the matching Message Router response. Connected messages use Send Unit Data (0x0070) instead,
/// which gets no reply.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    // The Null Address Item, the Unconnected Data Item, then the Sockaddr Info items
    #[br(temp, assert(
        item_count >= eip_constants::SEND_RR_DATA_REQUIRED_ITEM_COUNT,
        "a Send RR Data packet has at least 2 items"
    ))]
    #[bw(calc = eip_constants::SEND_RR_DATA_REQUIRED_ITEM_COUNT + sockaddr_info_items.count())]
    item_count: CipUint,

    #[br(assert(
        null_address_item.type_id == CommonPacketItemId::NullAddr,
        "expected a Null Address Item"
    ))]
    pub null_address_item: CommonPacketDescriptor,

    #[br(assert(
        unconnected_data_item.type_id == CommonPacketItemId::UnconnectedMessage,
        "expected an Unconnected Data Item"
    ))]
    #[bw(args { data_length: serialized_length(cip_message)? })]
    pub unconnected_data_item: CommonPacketDescriptor,

    #[br(args(unconnected_data_item.packet_length.unwrap_or_default()))]
    pub cip_message: CipMessage,

    #[br(args(item_count - eip_constants::SEND_RR_DATA_REQUIRED_ITEM_COUNT))]
    pub sockaddr_info_items: SockaddrInfoItems,
}

// ======= Start of RRPacketData impl ========

impl RRPacketData {
    /// An unconnected message: the Null Address Item followed by the Unconnected Data Item
    pub fn new_unconnected(
        interface_handle: CipUdint,
        timeout: CipUint,
        message: impl Into<CipMessage>,
    ) -> Self {
        RRPacketData {
            interface_handle,
            timeout,
            null_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: Some(0),
            },
            unconnected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: None,
            },
            cip_message: message.into(),
            sockaddr_info_items: SockaddrInfoItems::empty(),
        }
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
pub enum CommandSpecificData {
    #[br(pre_assert(command_type == EnIpCommand::UnRegisterSession))]
    UnregisterSession,

    #[br(pre_assert(command_type == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    /// Send RR Data: an unconnected request or its reply, see [`RRPacketData`]
    #[br(pre_assert(command_type == EnIpCommand::SendRrData))]
    SendRrData(RRPacketData),
    /*  When reading -- make sure the provided command_type matches */
}

// ======= Start of CommandSpecificData impl ========

impl CommandSpecificData {
    pub(crate) fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: eip_constants::ENCAPSULATION_PROTOCOL_VERSION,
            option_flags: eip_constants::REGISTER_SESSION_OPTION_FLAGS,
        })
    }

    pub(crate) fn new_request(
        interface_handle: CipUdint,
        timeout: CipUint,
        message: impl Into<CipMessage>,
    ) -> Self {
        Self::SendRrData(RRPacketData::new_unconnected(
            interface_handle,
            timeout,
            message,
        ))
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

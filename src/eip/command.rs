use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    binrw,    // #[binrw] attribute
};

use crate::cip::types::{CipUdint, CipUint};

use crate::cip::message::CipMessage;

use super::constants as eip_constants;
use super::description::{CommonPacketDescriptor, CommonPacketItemId, serialized_length};
use super::socket_addr::SocketAddrInfoItems;

/// Length of the data carried by a Connected Address Item: the connection ID
const CONNECTED_ADDRESS_LENGTH: CipUint = 4;
/// Length of the CIP Sequence Count in front of a connected message
const CIP_SEQUENCE_COUNT_LENGTH: CipUint = 2;

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
    /// Send Unit Data: a connected request or its reply, see [`UnitPacketData`]
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
/// see [`UnitPacketData`].
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    // The Null Address Item, the Unconnected Data Item, then the Socket Address Info items
    #[br(temp, assert(
        item_count >= eip_constants::SEND_RR_DATA_REQUIRED_ITEM_COUNT,
        "a Send RR Data packet has at least 2 items"
    ))]
    #[bw(calc = eip_constants::SEND_RR_DATA_REQUIRED_ITEM_COUNT + socket_addr_info_items.count())]
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
    pub socket_addr_info_items: SocketAddrInfoItems,
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
            socket_addr_info_items: SocketAddrInfoItems::empty(),
        }
    }
}

// ^^^^^^^^ End of RRPacketData impl ^^^^^^^^

/// Command specific data of a Send Unit Data packet (Wireshark: "Command Specific Data").
///
/// Send Unit Data (command 0x0070) carries one message over a class 3 connection: the originator
/// sends a Message Router request on the O->T connection ID, and the target sends the response in
/// a Send Unit Data packet of its own on the T->O connection ID. The response repeats the
/// request's CIP Sequence Count; a request sent again keeps its count.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct UnitPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    // The Connected Address Item, then the Connected Data Item
    #[br(temp, assert(
        item_count == eip_constants::SEND_UNIT_DATA_ITEM_COUNT,
        "a Send Unit Data packet has exactly 2 items"
    ))]
    #[bw(calc = eip_constants::SEND_UNIT_DATA_ITEM_COUNT)]
    item_count: CipUint,

    #[br(assert(
        connected_address_item.type_id == CommonPacketItemId::ConnectionAddressItem
            && connected_address_item.packet_length == Some(CONNECTED_ADDRESS_LENGTH),
        "expected a Connected Address Item with a Length of 4"
    ))]
    pub connected_address_item: CommonPacketDescriptor,

    pub connection_id: CipUdint,

    #[br(assert(
        connected_data_item.type_id == CommonPacketItemId::ConnectedTransportPacket,
        "expected a Connected Data Item"
    ))]
    #[bw(args { data_length: CIP_SEQUENCE_COUNT_LENGTH + serialized_length(cip_message)? })]
    pub connected_data_item: CommonPacketDescriptor,

    pub cip_sequence_count: CipUint,

    #[br(args(connected_data_item
        .packet_length
        .unwrap_or_default()
        .saturating_sub(CIP_SEQUENCE_COUNT_LENGTH)))]
    pub cip_message: CipMessage,
}

// ======= Start of UnitPacketData impl ========

impl UnitPacketData {
    /// A connected message on the connection `connection_id`: the Connected Address Item followed
    /// by the Connected Data Item
    pub fn new_connected(
        connection_id: CipUdint,
        cip_sequence_count: CipUint,
        message: impl Into<CipMessage>,
    ) -> Self {
        UnitPacketData {
            interface_handle: eip_constants::CIP_INTERFACE_HANDLE,
            timeout: eip_constants::NO_ENCAPSULATION_TIMEOUT,
            connected_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectionAddressItem,
                packet_length: Some(CONNECTED_ADDRESS_LENGTH),
            },
            connection_id,
            connected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectedTransportPacket,
                packet_length: None,
            },
            cip_sequence_count,
            cip_message: message.into(),
        }
    }
}

// ^^^^^^^^ End of UnitPacketData impl ^^^^^^^^

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

    /// Send Unit Data: a connected request or its reply, see [`UnitPacketData`]
    #[br(pre_assert(command_type == EnIpCommand::SendUnitData))]
    SendUnitData(UnitPacketData),
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

    /// The data of a Send RR Data command, `None` for any other command
    pub fn as_send_rr_data(&self) -> Option<&RRPacketData> {
        match self {
            Self::SendRrData(rr_data) => Some(rr_data),
            _ => None,
        }
    }

    /// The data of a Send Unit Data command, `None` for any other command
    pub fn as_send_unit_data(&self) -> Option<&UnitPacketData> {
        match self {
            Self::SendUnitData(unit_data) => Some(unit_data),
            _ => None,
        }
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

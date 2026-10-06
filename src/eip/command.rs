use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    binrw,    // #[binrw] attribute
};

use crate::cip::types::{CipUdint, CipUint};

use crate::cip::message::CipMessage;

use super::constants as eip_constants;
use super::description::CommonPacketItem;

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
    // Needs to be of type CipUdint (u32)
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
///
/// * `interface_handle`: the communications interface the request is for; always 0 for CIP.
/// * `timeout`: an encapsulation-level timeout in seconds, 0 meaning "rely on the encapsulated
///   protocol's own timeout". It is always 0 for CIP and ignored by the target; the reply has the
///   field too but does not use it.
/// * `items`: the Common Packet Format items: the Null Address Item, the Unconnected Data Item
///   holding the CIP message, then, for Forward_Open, optional Socket Address Info items.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    // Read from the wire; written from the number of items actually serialized
    #[br(temp)]
    #[bw(calc = items.len() as CipUint)]
    item_count: CipUint,

    #[br(count = item_count)]
    pub items: Vec<CommonPacketItem>,
}

// ======= Start of RRPacketData impl ========

impl RRPacketData {
    fn new(interface_handle: CipUdint, timeout: CipUint, items: Vec<CommonPacketItem>) -> Self {
        RRPacketData {
            interface_handle,
            timeout,
            items,
        }
    }

    /// An unconnected message: the Null Address Item followed by the Unconnected Data Item
    pub fn new_unconnected(
        interface_handle: CipUdint,
        timeout: CipUint,
        message: impl Into<CipMessage>,
    ) -> Self {
        Self::new(
            interface_handle,
            timeout,
            vec![
                CommonPacketItem::NullAddressItem,
                CommonPacketItem::UnconnectedDataItem(message.into()),
            ],
        )
    }

    /// The CIP message carried by the Unconnected Data Item, if any
    pub(crate) fn cip_message(&self) -> Option<&CipMessage> {
        self.items.iter().find_map(|item| match item {
            CommonPacketItem::UnconnectedDataItem(message) => Some(message),
            _ => None,
        })
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

    /// The Common Packet Format items (empty for commands without them)
    pub(crate) fn items(&self) -> &[CommonPacketItem] {
        match self {
            CommandSpecificData::SendRrData(rr_data) => &rr_data.items,
            _ => &[],
        }
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

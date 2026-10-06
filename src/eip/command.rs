use binrw::{
    BinRead,  // trait for reading
    BinWrite, // trait for writing
    binrw,    // #[binrw] attribute
};

use crate::cip::types::{CipUdint, CipUint};

use super::description::{CipMessage, CommonPacketItem};

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

/// Command specific data of a SendRRData packet: the interface handle, the timeout and the Common
/// Packet Format (the item count followed by the items).
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RRPacketData<M: CipMessage> {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    // Read from the wire; written from the number of items actually serialized
    #[br(temp)]
    #[bw(calc = items.len() as CipUint)]
    item_count: CipUint,

    #[br(count = item_count)]
    pub items: Vec<CommonPacketItem<M>>,
}

// ======= Start of RRPacketData impl ========

impl<M: CipMessage> RRPacketData<M> {
    pub fn new(
        interface_handle: CipUdint,
        timeout: CipUint,
        items: Vec<CommonPacketItem<M>>,
    ) -> Self {
        RRPacketData {
            interface_handle,
            timeout,
            items,
        }
    }

    /// An unconnected message: the Null Address Item followed by the Unconnected Data Item
    pub fn new_unconnected(interface_handle: CipUdint, timeout: CipUint, message: M) -> Self {
        Self::new(
            interface_handle,
            timeout,
            vec![
                CommonPacketItem::NullAddress,
                CommonPacketItem::UnconnectedData(message),
            ],
        )
    }

    /// The CIP message carried by the Unconnected Data Item, if any
    pub fn cip_message(&self) -> Option<&M> {
        self.items.iter().find_map(|item| match item {
            CommonPacketItem::UnconnectedData(message) => Some(message),
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
pub enum CommandSpecificData<M: CipMessage> {
    #[br(pre_assert(command_type == EnIpCommand::UnRegisterSession))]
    UnregisterSession,

    #[br(pre_assert(command_type == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    #[br(pre_assert(command_type == EnIpCommand::SendRrData))]
    SendRrData(RRPacketData<M>),
    /*  When reading -- make sure the provided command_type matches */
}

// ======= Start of CommandSpecificData impl ========

impl<M: CipMessage> CommandSpecificData<M> {
    pub fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: 1,
            option_flags: 0,
        })
    }

    pub fn new_request(interface_handle: CipUdint, timeout: CipUint, message: M) -> Self {
        Self::SendRrData(RRPacketData::new_unconnected(
            interface_handle,
            timeout,
            message,
        ))
    }

    /// The Common Packet Format items (empty for commands without them)
    pub fn items(&self) -> &[CommonPacketItem<M>] {
        match self {
            CommandSpecificData::SendRrData(rr_data) => &rr_data.items,
            _ => &[],
        }
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

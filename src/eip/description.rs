use binrw::{
    BinWrite, // trait for writing
    binrw,    // #[binrw] attribute
};

use crate::cip::types::{CipUint, CipUsint};

use super::sockaddr::{SOCKADDR_INFO_LENGTH, SockaddrInfo};

/// Type ID of a Common Packet Format item.
///
/// Unknown values are kept as-is so an unexpected item can be skipped using its length instead of
/// failing the whole packet.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CommonPacketItemId {
    #[brw(magic = 0x0000u16)]
    NullAddr,
    #[brw(magic = 0x000Cu16)]
    ListIdentity,
    #[brw(magic = 0x00A1u16)]
    ConnectionAddressItem,
    #[brw(magic = 0x00B1u16)]
    ConnectedTransportPacket,
    #[brw(magic = 0x00B2u16)]
    UnconnectedMessage,
    #[brw(magic = 0x8000u16)]
    O2TSockAddrInfo,
    #[brw(magic = 0x8001u16)]
    T2OSockAddrInfo,
    #[brw(magic = 0x8002u16)]
    SequencedAddressItem,
    Unknown(CipUint),
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
#[bw(import(provided_packet_length: Option<u16>))]
pub struct CommonPacketDescriptor {
    pub type_id: CommonPacketItemId,

    #[bw(args(provided_packet_length), write_with = descripter_length_writer)]
    pub packet_length: Option<CipUint>,
}

// ======= Start of CommonPacketDescriptor impl ========

#[binrw::writer(writer: writer, endian)]
fn descripter_length_writer(obj: &Option<CipUint>, arg0: Option<u16>) -> binrw::BinResult<()> {
    let write_value = arg0.unwrap_or(0);

    // If there isn't an input argument size, then just write 0
    if obj.is_some() && arg0 == Some(0) {
        return obj.write_options(writer, endian, ());
    }

    // let write_value = arg0.unwrap_or(0);
    write_value.write_options(writer, endian, ())
}

// ^^^^^^^^ End of CommonPacketDescriptor impl ^^^^^^^^

/// The data of a Common Packet Format item, selected by the Type ID of its descriptor.
///
/// Only the items that may follow the address and data items of a packet are modelled; the
/// address and data items themselves are handled by the command specific data and the object
/// assemblies.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(type_id: CommonPacketItemId, packet_length: u16))]
pub enum CommonPacketItemData {
    #[br(pre_assert(
        type_id == CommonPacketItemId::O2TSockAddrInfo && packet_length == SOCKADDR_INFO_LENGTH
    ))]
    O2TSockAddrInfo(SockaddrInfo),

    #[br(pre_assert(
        type_id == CommonPacketItemId::T2OSockAddrInfo && packet_length == SOCKADDR_INFO_LENGTH
    ))]
    T2OSockAddrInfo(SockaddrInfo),

    /// Any other item: the raw data is kept so the packet can be re-serialized unchanged
    Unknown(#[br(count = packet_length)] Vec<CipUsint>),
}

// ======= Start of CommonPacketItemData impl ========

impl CommonPacketItemData {
    /// Number of data bytes of the item (the Length field of its descriptor)
    pub fn byte_len(&self) -> u16 {
        match self {
            CommonPacketItemData::O2TSockAddrInfo(_) | CommonPacketItemData::T2OSockAddrInfo(_) => {
                SOCKADDR_INFO_LENGTH
            }
            CommonPacketItemData::Unknown(data) => data.len() as u16,
        }
    }
}

// ^^^^^^^^ End of CommonPacketItemData impl ^^^^^^^^

/// A complete Common Packet Format item (descriptor + data) that follows the address and data
/// items of a packet, e.g. the Sockaddr Info items of a Forward_Open exchange.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct CommonPacketItem {
    #[bw(args(Some(data.byte_len())))]
    pub descriptor: CommonPacketDescriptor,

    #[br(args(descriptor.type_id, descriptor.packet_length.unwrap_or(0)))]
    pub data: CommonPacketItemData,
}

// ======= Start of CommonPacketItem impl ========

impl CommonPacketItem {
    pub fn new(type_id: CommonPacketItemId, data: CommonPacketItemData) -> Self {
        CommonPacketItem {
            descriptor: CommonPacketDescriptor {
                type_id,
                packet_length: Some(data.byte_len()),
            },
            data,
        }
    }
}

// ^^^^^^^^ End of CommonPacketItem impl ^^^^^^^^

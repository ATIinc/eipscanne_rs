use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{
    binrw,    // #[binrw] attribute
    BinWrite, // trait for writing
};

use crate::cip::types::{CipInt, CipUdint, CipUint, CipUsint};

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

/// Length of the data carried by a Sockaddr Info item
pub const SOCKADDR_INFO_LENGTH: u16 = 16;
/// Only IPv4 socket addresses are allowed
pub const SOCKADDR_FAMILY_INET: CipInt = 2;

/// Socket address information exchanged while opening an I/O connection (Sockaddr Info in Wireshark).
///
/// Unlike the rest of the protocol, the family, port and address are sent in big endian order.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct SockaddrInfo {
    #[brw(big)]
    pub sin_family: CipInt,
    #[brw(big)]
    pub sin_port: CipUint,
    #[brw(big)]
    pub sin_addr: CipUdint,
    pub sin_zero: [CipUsint; 8],
}

// ======= Start of SockaddrInfo impl ========

impl SockaddrInfo {
    pub fn new(address: SocketAddrV4) -> Self {
        SockaddrInfo {
            sin_family: SOCKADDR_FAMILY_INET,
            sin_port: address.port(),
            sin_addr: u32::from(*address.ip()),
            sin_zero: [0; 8],
        }
    }

    pub fn socket_address(&self) -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::from(self.sin_addr), self.sin_port)
    }
}

impl From<SocketAddrV4> for SockaddrInfo {
    fn from(address: SocketAddrV4) -> Self {
        SockaddrInfo::new(address)
    }
}

impl From<SockaddrInfo> for SocketAddrV4 {
    fn from(info: SockaddrInfo) -> Self {
        info.socket_address()
    }
}

// ^^^^^^^^ End of SockaddrInfo impl ^^^^^^^^

/// The data of a Common Packet Format item that follows the address and data items of a packet.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(type_id: CommonPacketItemId, packet_length: u16))]
pub enum AdditionalItem {
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

// ======= Start of AdditionalItem impl ========

impl AdditionalItem {
    /// Number of data bytes of the item (the Length field of its descriptor)
    pub fn byte_len(&self) -> u16 {
        match self {
            AdditionalItem::O2TSockAddrInfo(_) | AdditionalItem::T2OSockAddrInfo(_) => {
                SOCKADDR_INFO_LENGTH
            }
            AdditionalItem::Unknown(data) => data.len() as u16,
        }
    }
}

// ^^^^^^^^ End of AdditionalItem impl ^^^^^^^^

/// A complete Common Packet Format item (descriptor + data) that is not the address or data item
/// of the packet, e.g. the Sockaddr Info items of a Forward_Open exchange.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct CommonPacketItem {
    #[bw(args(Some(item.byte_len())))]
    pub descriptor: CommonPacketDescriptor,

    #[br(args(descriptor.type_id, descriptor.packet_length.unwrap_or(0)))]
    pub item: AdditionalItem,
}

// ======= Start of CommonPacketItem impl ========

impl CommonPacketItem {
    fn new(type_id: CommonPacketItemId, item: AdditionalItem) -> Self {
        CommonPacketItem {
            descriptor: CommonPacketDescriptor {
                type_id,
                packet_length: Some(item.byte_len()),
            },
            item,
        }
    }

    /// Sockaddr Info describing where originator-to-target I/O data must be sent
    pub fn new_o2t_sockaddr_info(address: SocketAddrV4) -> Self {
        Self::new(
            CommonPacketItemId::O2TSockAddrInfo,
            AdditionalItem::O2TSockAddrInfo(SockaddrInfo::new(address)),
        )
    }

    /// Sockaddr Info describing where target-to-originator I/O data must be sent
    pub fn new_t2o_sockaddr_info(address: SocketAddrV4) -> Self {
        Self::new(
            CommonPacketItemId::T2OSockAddrInfo,
            AdditionalItem::T2OSockAddrInfo(SockaddrInfo::new(address)),
        )
    }

    pub fn sockaddr_info(&self) -> Option<&SockaddrInfo> {
        match &self.item {
            AdditionalItem::O2TSockAddrInfo(info) | AdditionalItem::T2OSockAddrInfo(info) => {
                Some(info)
            }
            AdditionalItem::Unknown(_) => None,
        }
    }
}

// ^^^^^^^^ End of CommonPacketItem impl ^^^^^^^^

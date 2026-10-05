//! Socket address information exchanged while opening an I/O connection (implicit messaging).

use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::binrw;

use crate::cip::types::{CipInt, CipUdint, CipUint, CipUsint};

use super::description::{CommonPacketItem, CommonPacketItemData, CommonPacketItemId};

/// Length of the data carried by a Sockaddr Info item
pub const SOCKADDR_INFO_LENGTH: u16 = 16;
/// Only IPv4 socket addresses are allowed
pub const SOCKADDR_FAMILY_INET: CipInt = 2;

/// Socket address information (Sockaddr Info in Wireshark).
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

// ======= Start of CommonPacketItem sockaddr impl ========

impl CommonPacketItem {
    /// Sockaddr Info describing where originator-to-target I/O data must be sent
    pub fn new_o2t_sockaddr_info(address: SocketAddrV4) -> Self {
        Self::new(
            CommonPacketItemId::O2TSockAddrInfo,
            CommonPacketItemData::O2TSockAddrInfo(SockaddrInfo::new(address)),
        )
    }

    /// Sockaddr Info describing where target-to-originator I/O data must be sent
    pub fn new_t2o_sockaddr_info(address: SocketAddrV4) -> Self {
        Self::new(
            CommonPacketItemId::T2OSockAddrInfo,
            CommonPacketItemData::T2OSockAddrInfo(SockaddrInfo::new(address)),
        )
    }

    pub fn sockaddr_info(&self) -> Option<&SockaddrInfo> {
        match &self.data {
            CommonPacketItemData::O2TSockAddrInfo(info)
            | CommonPacketItemData::T2OSockAddrInfo(info) => Some(info),
            CommonPacketItemData::Unknown(_) => None,
        }
    }
}

// ^^^^^^^^ End of CommonPacketItem sockaddr impl ^^^^^^^^

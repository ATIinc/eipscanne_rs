//! Socket address information exchanged while opening an I/O connection (implicit messaging).

use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::binrw;

use crate::cip::types::{CipInt, CipUdint, CipUint, CipUsint};

use super::description::CommonPacketItem;

/// Length of the data carried by a Sockaddr Info item
pub(crate) const SOCKADDR_INFO_LENGTH: u16 = 16;
/// Only IPv4 socket addresses are allowed
const SOCKADDR_FAMILY_INET: CipInt = 2;

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
    fn new(address: SocketAddrV4) -> Self {
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
    pub fn sockaddr_info(&self) -> Option<&SockaddrInfo> {
        match self {
            CommonPacketItem::O2TSockAddrInfo(info) | CommonPacketItem::T2OSockAddrInfo(info) => {
                Some(info)
            }
            _ => None,
        }
    }
}

// ^^^^^^^^ End of CommonPacketItem sockaddr impl ^^^^^^^^

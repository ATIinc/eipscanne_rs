//! Socket address information exchanged while opening an I/O connection (implicit messaging).

use std::io::{Read, Seek, Write};
use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::meta::{EndianKind, ReadEndian, WriteEndian};
use binrw::{
    BinRead, // trait for reading
    BinResult,
    BinWrite, // trait for writing
    Endian,
    binrw, // #[binrw] attribute
};

use crate::cip::types::{CipInt, CipUdint, CipUint, CipUsint};

use super::description::{CommonPacketDescriptor, CommonPacketItemId};

/// Length of the data carried by a Sockaddr Info item
const SOCKADDR_INFO_LENGTH: CipUint = 16;
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

/// The Socket Address Info items of a Forward_Open request or reply, after its Unconnected Data
/// Item. Each one is optional, and they may come in either order: the Type ID of an item says
/// which one it is. They are written O->T first.
#[derive(Debug, PartialEq, Default)]
pub struct SockaddrInfoItems {
    /// Socket Address Info O->T (0x8000): where the scanner sends its I/O data
    pub o2t: Option<SockaddrInfo>,
    /// Socket Address Info T->O (0x8001): where the adapter sends its I/O data
    pub t2o: Option<SockaddrInfo>,
}

// ======= Start of SockaddrInfoItems impl ========

impl SockaddrInfoItems {
    /// The number of items present
    pub(crate) fn count(&self) -> CipUint {
        self.o2t.is_some() as CipUint + self.t2o.is_some() as CipUint
    }
}

impl ReadEndian for SockaddrInfoItems {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl WriteEndian for SockaddrInfoItems {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl BinRead for SockaddrInfoItems {
    // The number of items to read
    type Args<'a> = (CipUint,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        (count,): Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut items = SockaddrInfoItems::default();

        for _ in 0..count {
            let pos = reader.stream_position()?;
            let descriptor = CommonPacketDescriptor::read_options(reader, endian, ())?;
            let type_id = descriptor.type_id;

            let item = match type_id {
                CommonPacketItemId::O2TSockAddrInfo => &mut items.o2t,
                CommonPacketItemId::T2OSockAddrInfo => &mut items.t2o,
                _ => {
                    return Err(binrw::Error::AssertFail {
                        pos,
                        message: format!("expected a Sockaddr Info item, found a {type_id:?}"),
                    });
                }
            };
            if item.is_some() || descriptor.packet_length != Some(SOCKADDR_INFO_LENGTH) {
                return Err(binrw::Error::AssertFail {
                    pos,
                    message: format!("a second or malformed {type_id:?}"),
                });
            }

            *item = Some(SockaddrInfo::read_options(reader, endian, ())?);
        }

        Ok(items)
    }
}

impl BinWrite for SockaddrInfoItems {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        let items = [
            (CommonPacketItemId::O2TSockAddrInfo, &self.o2t),
            (CommonPacketItemId::T2OSockAddrInfo, &self.t2o),
        ];
        for (type_id, info) in items {
            if let Some(info) = info {
                let descriptor = CommonPacketDescriptor {
                    type_id,
                    packet_length: Some(SOCKADDR_INFO_LENGTH),
                };
                descriptor.write_options(writer, endian, Default::default())?;
                info.write_options(writer, endian, ())?;
            }
        }

        Ok(())
    }
}

// ^^^^^^^^ End of SockaddrInfoItems impl ^^^^^^^^

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

/// Length of the data carried by a Socket Address Info item
const SOCKET_ADDR_INFO_LENGTH: CipUint = 16;
/// Only IPv4 socket addresses are allowed
const SOCKET_ADDR_FAMILY_INET: CipInt = 2;

/// Socket address information (Socket Address Info in Wireshark).
///
/// Unlike the rest of the protocol, the family, port and address are sent in big endian order.
/// The field names are those of a BSD `sockaddr_in` ("socket address, internet").
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct SocketAddrInfo {
    /// Address family: 2 (AF_INET, IPv4)
    #[brw(big)]
    pub sin_family: CipInt,
    /// Port number
    #[brw(big)]
    pub sin_port: CipUint,
    /// IPv4 address
    #[brw(big)]
    pub sin_addr: CipUdint,
    /// Zero padding
    pub sin_zero: [CipUsint; 8],
}

// ======= Start of SocketAddrInfo impl ========

impl SocketAddrInfo {
    fn new(address: SocketAddrV4) -> Self {
        SocketAddrInfo {
            sin_family: SOCKET_ADDR_FAMILY_INET,
            sin_port: address.port(),
            sin_addr: u32::from(*address.ip()),
            sin_zero: [0; 8],
        }
    }

    pub fn socket_address(&self) -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::from(self.sin_addr), self.sin_port)
    }
}

impl From<SocketAddrV4> for SocketAddrInfo {
    fn from(address: SocketAddrV4) -> Self {
        SocketAddrInfo::new(address)
    }
}

impl From<SocketAddrInfo> for SocketAddrV4 {
    fn from(info: SocketAddrInfo) -> Self {
        info.socket_address()
    }
}

// ^^^^^^^^ End of SocketAddrInfo impl ^^^^^^^^

/// The Socket Address Info items of a Forward_Open request or reply, after its Unconnected Data
/// Item. Each one is optional, and they may come in either order: the Type ID of an item says
/// which one it is. They are written O->T first.
#[derive(Debug, PartialEq)]
pub struct SocketAddrInfoItems {
    /// Socket Address Info O->T (0x8000): where the scanner sends its I/O data
    pub o2t: Option<SocketAddrInfo>,
    /// Socket Address Info T->O (0x8001): where the adapter sends its I/O data
    pub t2o: Option<SocketAddrInfo>,
}

// ======= Start of SocketAddrInfoItems impl ========

impl SocketAddrInfoItems {
    /// No Socket Address Info items
    pub const fn empty() -> Self {
        SocketAddrInfoItems {
            o2t: None,
            t2o: None,
        }
    }

    /// The number of items present
    pub(crate) fn count(&self) -> CipUint {
        self.o2t.is_some() as CipUint + self.t2o.is_some() as CipUint
    }
}

impl ReadEndian for SocketAddrInfoItems {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl WriteEndian for SocketAddrInfoItems {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl BinRead for SocketAddrInfoItems {
    // The number of items to read
    type Args<'a> = (CipUint,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        (count,): Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut items = SocketAddrInfoItems::empty();

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
                        message: format!(
                            "expected a Socket Address Info item, found a {type_id:?}"
                        ),
                    });
                }
            };
            if item.is_some() || descriptor.packet_length != Some(SOCKET_ADDR_INFO_LENGTH) {
                return Err(binrw::Error::AssertFail {
                    pos,
                    message: format!("a second or malformed {type_id:?}"),
                });
            }

            *item = Some(SocketAddrInfo::read_options(reader, endian, ())?);
        }

        Ok(items)
    }
}

impl BinWrite for SocketAddrInfoItems {
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
                    packet_length: Some(SOCKET_ADDR_INFO_LENGTH),
                };
                descriptor.write_options(writer, endian, Default::default())?;
                info.write_options(writer, endian, ())?;
            }
        }

        Ok(())
    }
}

// ^^^^^^^^ End of SocketAddrInfoItems impl ^^^^^^^^

use std::io::{Cursor, Read, Seek, Write};

use binrw::meta::{EndianKind, ReadEndian, WriteEndian};
use binrw::{
    BinRead, // trait for reading
    BinResult,
    BinWrite, // trait for writing
    Endian,
    binrw, // #[binrw] attribute
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

/// A message carried by an Unconnected Data Item, e.g. a Message Router request or response.
///
/// Reading it takes the length of the item, since the message does not encode its own length.
pub trait CipMessage:
    'static + for<'a> BinRead<Args<'a> = (u16,)> + for<'a> BinWrite<Args<'a> = ()>
{
}

impl<T> CipMessage for T where
    T: 'static + for<'a> BinRead<Args<'a> = (u16,)> + for<'a> BinWrite<Args<'a> = ()>
{
}

/// A Common Packet Format item: Type ID, Length and the data selected by the Type ID.
///
/// The Type ID and the Length are derived from the variant on write, so an item can never be built
/// with a Type ID or Length that does not match its data. On read, an item whose data does not fit
/// its variant (wrong length, unparsable message) is kept as `Unknown` with its raw data.
#[derive(Debug, PartialEq, Clone)]
pub enum CommonPacketItem<M: CipMessage> {
    /// Null Address Item: no data, used for unconnected messages
    NullAddress,

    /// Unconnected Data Item: the CIP message of a SendRRData packet
    UnconnectedData(M),

    O2TSockAddrInfo(SockaddrInfo),

    T2OSockAddrInfo(SockaddrInfo),

    /// Any other item: the raw data is kept so the packet can be re-serialized unchanged
    Unknown {
        type_id: CommonPacketItemId,
        data: Vec<CipUsint>,
    },
}

// ======= Start of CommonPacketItem impl ========

impl<M: CipMessage> CommonPacketItem<M> {
    pub fn type_id(&self) -> CommonPacketItemId {
        match self {
            CommonPacketItem::NullAddress => CommonPacketItemId::NullAddr,
            CommonPacketItem::UnconnectedData(_) => CommonPacketItemId::UnconnectedMessage,
            CommonPacketItem::O2TSockAddrInfo(_) => CommonPacketItemId::O2TSockAddrInfo,
            CommonPacketItem::T2OSockAddrInfo(_) => CommonPacketItemId::T2OSockAddrInfo,
            CommonPacketItem::Unknown { type_id, .. } => *type_id,
        }
    }

    /// Parses the data of an item into the variant selected by its Type ID
    fn parse_data(type_id: CommonPacketItemId, data: &[CipUsint], endian: Endian) -> Option<Self> {
        let packet_length = data.len() as CipUint;
        let mut data_reader = Cursor::new(data);

        match type_id {
            CommonPacketItemId::NullAddr if data.is_empty() => Some(CommonPacketItem::NullAddress),
            CommonPacketItemId::UnconnectedMessage => {
                M::read_options(&mut data_reader, endian, (packet_length,))
                    .ok()
                    .map(CommonPacketItem::UnconnectedData)
            }
            CommonPacketItemId::O2TSockAddrInfo if packet_length == SOCKADDR_INFO_LENGTH => {
                SockaddrInfo::read_options(&mut data_reader, endian, ())
                    .ok()
                    .map(CommonPacketItem::O2TSockAddrInfo)
            }
            CommonPacketItemId::T2OSockAddrInfo if packet_length == SOCKADDR_INFO_LENGTH => {
                SockaddrInfo::read_options(&mut data_reader, endian, ())
                    .ok()
                    .map(CommonPacketItem::T2OSockAddrInfo)
            }
            _ => None,
        }
    }
}

impl<M: CipMessage> ReadEndian for CommonPacketItem<M> {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl<M: CipMessage> WriteEndian for CommonPacketItem<M> {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl<M: CipMessage> BinRead for CommonPacketItem<M> {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let type_id = CommonPacketItemId::read_options(reader, endian, ())?;
        let packet_length = CipUint::read_options(reader, endian, ())?;

        let mut data = vec![0; packet_length as usize];
        reader.read_exact(&mut data)?;

        Ok(Self::parse_data(type_id, &data, endian)
            .unwrap_or(CommonPacketItem::Unknown { type_id, data }))
    }
}

impl<M: CipMessage> BinWrite for CommonPacketItem<M> {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Serialize the data first, its size is the Length of the item
        let mut data = Vec::new();
        let mut data_writer = Cursor::new(&mut data);

        match self {
            CommonPacketItem::NullAddress => {}
            CommonPacketItem::UnconnectedData(message) => {
                message.write_options(&mut data_writer, endian, ())?
            }
            CommonPacketItem::O2TSockAddrInfo(info) | CommonPacketItem::T2OSockAddrInfo(info) => {
                info.write_options(&mut data_writer, endian, ())?
            }
            CommonPacketItem::Unknown { data: raw_data, .. } => data_writer.write_all(raw_data)?,
        }

        self.type_id().write_options(writer, endian, ())?;
        (data.len() as CipUint).write_options(writer, endian, ())?;
        writer.write_all(&data)?;

        Ok(())
    }
}

// ^^^^^^^^ End of CommonPacketItem impl ^^^^^^^^

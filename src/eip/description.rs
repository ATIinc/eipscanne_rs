use std::io::{Cursor, Read, Seek, Write};

use binrw::meta::{EndianKind, ReadEndian, WriteEndian};
use binrw::{
    BinRead, // trait for reading
    BinResult,
    BinWrite, // trait for writing
    Endian,
    binrw, // #[binrw] attribute
};

use crate::cip::message::CipMessage;
use crate::cip::message::data::CipDataOpt;
use crate::cip::types::{CipUint, CipUsint};

use super::io_packet::{SEQUENCED_ADDRESS_LENGTH, SequencedAddress};
use super::sockaddr::{SOCKADDR_INFO_LENGTH, SockaddrInfo};

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CommonPacketItemId {
    #[brw(magic = 0x0000u16)]
    NullAddressItem,
    #[brw(magic = 0x000Cu16)]
    ListIdentityResponse,
    #[brw(magic = 0x00A1u16)]
    ConnectedAddressItem,
    #[brw(magic = 0x00B1u16)]
    ConnectedDataItem,
    #[brw(magic = 0x00B2u16)]
    UnconnectedDataItem,
    /// Socket Address Info O->T (originator to target: where the scanner sends I/O data)
    #[brw(magic = 0x8000u16)]
    O2TSockAddrInfo,
    /// Socket Address Info T->O (target to originator: where the adapter sends I/O data)
    #[brw(magic = 0x8001u16)]
    T2OSockAddrInfo,
    #[brw(magic = 0x8002u16)]
    SequencedAddressItem,
    Unknown(CipUint),
}

#[derive(Debug, PartialEq)]
pub enum CommonPacketItem {
    /// Null Address Item: no data, used for unconnected messages
    NullAddressItem,

    /// Unconnected Data Item: the CIP message of a SendRRData packet
    UnconnectedDataItem(CipMessage),

    /// Socket Address Info O->T (originator to target): the address the scanner must send its
    /// I/O data to
    O2TSockAddrInfo(SockaddrInfo),

    /// Socket Address Info T->O (target to originator): the address the adapter must send its
    /// I/O data to
    T2OSockAddrInfo(SockaddrInfo),

    /// Sequenced Address Item: the connection an I/O packet belongs to and the packet's number
    /// on that connection
    SequencedAddressItem(SequencedAddress),

    /// Connected Data Item: the data of an I/O packet. Always raw when read, because its layout
    /// (sequence count, run/idle header, application data) depends on the connection it belongs
    /// to; `IoData` decodes it once the connection is known
    ConnectedDataItem(CipDataOpt),

    /// Any other item: the raw data is kept so the packet can be re-serialized unchanged
    Unknown {
        type_id: CommonPacketItemId,
        data: Vec<CipUsint>,
    },
}

// ======= Start of CommonPacketItem impl ========

impl CommonPacketItem {
    fn type_id(&self) -> CommonPacketItemId {
        match self {
            CommonPacketItem::NullAddressItem => CommonPacketItemId::NullAddressItem,
            CommonPacketItem::UnconnectedDataItem(_) => CommonPacketItemId::UnconnectedDataItem,
            CommonPacketItem::O2TSockAddrInfo(_) => CommonPacketItemId::O2TSockAddrInfo,
            CommonPacketItem::T2OSockAddrInfo(_) => CommonPacketItemId::T2OSockAddrInfo,
            CommonPacketItem::SequencedAddressItem(_) => CommonPacketItemId::SequencedAddressItem,
            CommonPacketItem::ConnectedDataItem(_) => CommonPacketItemId::ConnectedDataItem,
            CommonPacketItem::Unknown { type_id, .. } => *type_id,
        }
    }

    fn parse_data(type_id: CommonPacketItemId, data: &[CipUsint], endian: Endian) -> Option<Self> {
        let packet_length = data.len() as CipUint;
        let mut data_reader = Cursor::new(data);

        match type_id {
            CommonPacketItemId::NullAddressItem if data.is_empty() => {
                Some(CommonPacketItem::NullAddressItem)
            }
            CommonPacketItemId::UnconnectedDataItem => {
                CipMessage::read_options(&mut data_reader, endian, (packet_length,))
                    .ok()
                    .map(CommonPacketItem::UnconnectedDataItem)
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
            CommonPacketItemId::SequencedAddressItem
                if packet_length == SEQUENCED_ADDRESS_LENGTH =>
            {
                SequencedAddress::read_options(&mut data_reader, endian, ())
                    .ok()
                    .map(CommonPacketItem::SequencedAddressItem)
            }
            // Always raw: how the data is laid out is only known once the connection is looked up
            CommonPacketItemId::ConnectedDataItem => {
                CipDataOpt::read_options(&mut data_reader, endian, (packet_length,))
                    .ok()
                    .map(CommonPacketItem::ConnectedDataItem)
            }
            _ => None,
        }
    }
}

impl ReadEndian for CommonPacketItem {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl WriteEndian for CommonPacketItem {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Little);
}

impl BinRead for CommonPacketItem {
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

impl BinWrite for CommonPacketItem {
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
            CommonPacketItem::NullAddressItem => {}
            CommonPacketItem::UnconnectedDataItem(message) => {
                message.write_options(&mut data_writer, endian, ())?
            }
            CommonPacketItem::O2TSockAddrInfo(info) | CommonPacketItem::T2OSockAddrInfo(info) => {
                info.write_options(&mut data_writer, endian, ())?
            }
            CommonPacketItem::SequencedAddressItem(address) => {
                address.write_options(&mut data_writer, endian, ())?
            }
            // The write side of `CipDataOpt` ignores its length argument
            CommonPacketItem::ConnectedDataItem(item_data) => {
                item_data.write_options(&mut data_writer, endian, (0,))?
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

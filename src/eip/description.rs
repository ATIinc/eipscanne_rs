use std::io::Cursor;

use binrw::{
    BinRead, // trait for reading
    BinResult,
    BinWrite, // trait for writing
    Endian,
    binrw, // #[binrw] attribute
};

use crate::cip::types::CipUint;

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUint)]
#[bw(little, repr = CipUint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CommonPacketItemId {
    NullAddr = 0x0000,
    ListIdentity = 0x000C,
    ConnectionAddressItem = 0x00A1,
    ConnectedTransportPacket = 0x00B1,
    UnconnectedMessage = 0x00B2,
    O2TSockAddrInfo = 0x8000,
    T2OSockAddrInfo = 0x8001,
    SequencedAddressItem = 0x8002,
}

/// The header of a Common Packet Format item: its Type ID and the Length of its data
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
#[bw(import { data_length: CipUint = 0 })]
pub struct CommonPacketDescriptor {
    pub type_id: CommonPacketItemId,

    // Written as stored, or as `data_length` (the size of the item's data) when None
    #[bw(map = |length: &Option<CipUint>| length.unwrap_or(data_length))]
    pub packet_length: Option<CipUint>,
}

/// The number of bytes `data` serializes to: the Length of an item carrying it
pub(crate) fn serialized_length<T>(data: &T) -> BinResult<CipUint>
where
    T: BinWrite,
    for<'a> T::Args<'a>: Default,
{
    let mut buffer = Vec::new();
    data.write_options(
        &mut Cursor::new(&mut buffer),
        Endian::Little,
        Default::default(),
    )?;
    Ok(buffer.len() as CipUint)
}

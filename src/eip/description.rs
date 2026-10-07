use std::io::{Cursor, Read, Seek};

use binrw::{
    BinRead, // trait for reading
    BinResult,
    BinWrite, // trait for writing
    Endian,
};

use crate::cip::types::{CipUint, CipUsint};

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

/// Reads an item: its Type ID, then Length bytes of data
pub(crate) fn read_any_item<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
) -> BinResult<(CommonPacketItemId, Vec<CipUsint>)> {
    let type_id = CommonPacketItemId::read_options(reader, endian, ())?;
    let length = CipUint::read_options(reader, endian, ())?;

    let mut data = vec![0; length as usize];
    reader.read_exact(&mut data)?;

    Ok((type_id, data))
}

/// Reads an item that must have the Type ID `expected`, and its data as a `T` of Length bytes
#[binrw::parser(reader, endian)]
pub(crate) fn read_item<T>(expected: CommonPacketItemId) -> BinResult<T>
where
    T: for<'a> BinRead<Args<'a> = (CipUint,)>,
{
    let pos = reader.stream_position()?;
    let (type_id, data) = read_any_item(reader, endian)?;

    if type_id != expected {
        return Err(binrw::Error::AssertFail {
            pos,
            message: format!("expected a {expected:?}, found a {type_id:?}"),
        });
    }

    T::read_options(&mut Cursor::new(&data), endian, (data.len() as CipUint,))
}

/// Writes an item: `type_id`, the Length of `data` once serialized, then `data`
#[binrw::writer(writer, endian)]
pub(crate) fn write_item<T>(data: &T, type_id: CommonPacketItemId) -> BinResult<()>
where
    T: BinWrite,
    for<'a> T::Args<'a>: Default,
{
    let mut buffer = Vec::new();
    data.write_options(&mut Cursor::new(&mut buffer), endian, Default::default())?;

    type_id.write_options(writer, endian, ())?;
    (buffer.len() as CipUint).write_options(writer, endian, ())?;
    writer.write_all(&buffer)?;

    Ok(())
}

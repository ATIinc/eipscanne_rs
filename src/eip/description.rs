use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // trait for reading
    BinWrite, // trait for writing
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

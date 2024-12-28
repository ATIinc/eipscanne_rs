use crate::cip::types::{CipByte, CipUint};
use serde::{Deserialize, Serialize};

// Convert FROM number to enum: https://enodev.fr/posts/rusticity-convert-an-integer-to-an-enum.html

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[repr(u16)]
pub enum CommonPacketItemId {
    // Needs to be of type CipUint (u16)
    NullAddr = 0x0000,
    ListIdentity = 0x000C,
    ConnectionAddressItem = 0x00A1,
    ConnectedTransportPacket = 0x00B1,
    UnconnectedMessage = 0x00B2,
    O2TSockAddrInfo = 0x8000,
    T2OSockAddrInfo = 0x8001,
    SequencedAddressItem = 0x8002,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CommonPacketItem {
    pub type_id: CommonPacketItemId,
    pub length: CipUint,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CommonPacket {
    pub items: Vec<CommonPacketItem>,
}

impl CommonPacket {
    pub fn new(packet_contents: Vec<CipByte>) -> Self {
        // always start with an empty packet
        let empty_packet = CommonPacketItem {
            type_id: CommonPacketItemId::NullAddr,
            length: 0,
            data: Vec::new(),
        };

        // follow it with a packet that contains the data
        let data_packet = CommonPacketItem {
            type_id: CommonPacketItemId::UnconnectedMessage,
            length: packet_contents.len() as CipUint,
            data: packet_contents,
        };

        CommonPacket {
            items: vec![empty_packet, data_packet],
        }
    }
}

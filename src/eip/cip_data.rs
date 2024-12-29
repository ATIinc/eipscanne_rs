use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{mem, vec};

use crate::cip::types::{CipByte, CipUint};

// Enum definition with `Serialize` and `Deserialize` traits.
#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq, Copy, Clone)]
#[repr(u16)]
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

// Modify the struct to require both `Serialize` and `Deserialize` for the generic `T`
#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct CommonPacketItem<T> {
    pub type_id: CommonPacketItemId,
    pub length: CipUint,
    pub data: Option<T>,
}

// Modify the struct to require both `Serialize` and `Deserialize` for the generic `T`
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct CipDataPacket<T> {
    pub items: Vec<CommonPacketItem<T>>,
}

impl<T> CipDataPacket<T>
where
    T: Serialize + DeserializeOwned,
{
    // Method to get the size of the contained value in the enum
    pub fn get_size(&self) -> usize {
        self.items.iter().fold(0, |acc, item| {
            acc + mem::size_of_val(&item.type_id) + mem::size_of_val(&item.length)
        })
    }
}

// Implementing the `new` function for `CipDataPacket`
impl<T> CipDataPacket<T> {
    pub fn new(cip_object: T) -> Self {
        // always start with an empty packet
        let empty_packet = CommonPacketItem {
            type_id: CommonPacketItemId::NullAddr,
            length: 0,
            data: None,
        };

        let cip_object_byte_length: CipUint = mem::size_of_val(&cip_object) as CipUint;

        // follow it with a packet that contains the data
        let data_packet = CommonPacketItem {
            type_id: CommonPacketItemId::UnconnectedMessage,
            length: cip_object_byte_length,
            data: if cip_object_byte_length > 0 {
                Some(cip_object)
            } else {
                None
            },
        };

        CipDataPacket {
            items: vec![empty_packet, data_packet],
        }
    }
}

impl CipDataPacket<CipByte> {
    pub fn new_empty() -> Self {
        CipDataPacket { items: vec![] }
    }
}

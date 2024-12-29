use std::fmt;
use std::mem;

use serde::de::{Deserializer, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

//  Tried to use bilge: https://github.com/hecatia-elegua/bilge
//      * Apparently Deku may be slower because there are a LOT of transforms
use deku::prelude::*;

use crate::cip::types::CipByte;

// The whole CipPath is a CipUint (16 bit number)
#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct CipPath {
    #[deku(bits = 3)]
    pub logical_segment: u8,
    #[deku(bits = 3)]
    pub class_id: u8,
    #[deku(bits = 2)]
    pub instance_id: u8,
    pub attribute_id: u8,
}

impl Serialize for CipPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let serialized_byte_vec = self.to_bytes().unwrap();

        const U16_BYTE_ARRAY_SIZE: usize = mem::size_of::<u16>();

        if serialized_byte_vec.len() != U16_BYTE_ARRAY_SIZE {
            panic!(
                "CipPath serialization error: serialized bytes vector length is not {}",
                U16_BYTE_ARRAY_SIZE
            );
        }

        let serialized_byte_array: [u8; U16_BYTE_ARRAY_SIZE] =
            serialized_byte_vec.try_into().unwrap();

        // Serialize the u16 as little endian
        serializer.serialize_u16(u16::from_le_bytes(serialized_byte_array))
    }
}

struct CipPathVisitor;

impl<'de> Visitor<'de> for CipPathVisitor {
    type Value = CipPath;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer between -2^31 and 2^31")
    }

    fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let byte_offset = 0;
        // NOTE: This is a bit inneficient, but it takes advantage of the existing bytes method
        let converted_bytes: Vec<u8> = value.to_le_bytes().to_vec();

        let ((_remaining_bytes, _remaining_byte_size), deserialized_path) =
            CipPath::from_bytes((converted_bytes.as_ref(), byte_offset)).unwrap();

        Ok(deserialized_path)
    }
}

impl<'de> Deserialize<'de> for CipPath {
    fn deserialize<D>(deserializer: D) -> Result<CipPath, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u16(CipPathVisitor)
    }
}

#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq)]
#[repr(u8)]
pub enum ServiceCode {
    None = 0x00,
    /* Start CIP common services */
    GetAttributeAll = 0x01,
    SetAttributeAll = 0x02,
    GetAttributeList = 0x03,
    SetAttributeList = 0x04,
    Reset = 0x05,
    Start = 0x06,
    Stop = 0x07,
    CreateObjectInstance = 0x08,
    DeleteObjectInstance = 0x09,
    MultipleServicePacket = 0x0A,
    ApplyAttributes = 0x0D,
    GetAttributeSingle = 0x0E,
    SetAttributeSingle = 0x10,
    FindNextObjectInstance = 0x11,
    ErrorResponse = 0x14, //DeviceNet only
    Restore = 0x15,
    Save = 0x16,
    GetMember = 0x18,
    NoOperation = 0x17,
    SetMember = 0x19,
    InsertMember = 0x1A,
    RemoveMember = 0x1B,
    GroupSync = 0x1C, /* End CIP common services */
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct MessageRouterRequest {
    pub service_code: ServiceCode,
    pub path: CipPath,
    pub data: Vec<CipByte>,
    pub use_8_bit_path_segments: bool,
}

impl Serialize for MessageRouterRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let total_data_length = std::mem::size_of::<ServiceCode>() + self.data.len() as usize;

        let mut ser = serializer.serialize_seq(Some(total_data_length))?;
        ser.serialize_element(&self.service_code)?;
        ser.serialize_element(&self.data)?;
        ser.end()
    }
}

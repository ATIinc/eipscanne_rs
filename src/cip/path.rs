use std::fmt;
use std::mem;

use serde::de::{Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

//  Tried to use bilge: https://github.com/hecatia-elegua/bilge
//      * Apparently Deku may be slower because there are a LOT of transforms
use deku::prelude::*;

#[repr(u8)]
#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(id_type = "u8")]
pub enum SegmentType {
    LogicalSegment = 0x01,
}

// The whole CipPath is a CipUint (16 bit number)
#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct LogicalPathSegment {
    #[deku(bits = 3)]
    pub segment_type: SegmentType,
    #[deku(bits = 3)]
    pub logical_segment_type: u8,
    #[deku(bits = 2)]
    pub logical_segment_format: u8,
}

impl Serialize for LogicalPathSegment {
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

struct LogicalPathSegmentVisitor;

impl<'de> Visitor<'de> for LogicalPathSegmentVisitor {
    type Value = LogicalPathSegment;

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
            LogicalPathSegment::from_bytes((converted_bytes.as_ref(), byte_offset)).unwrap();

        Ok(deserialized_path)
    }
}

impl<'de> Deserialize<'de> for LogicalPathSegment {
    fn deserialize<D>(deserializer: D) -> Result<LogicalPathSegment, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u16(LogicalPathSegmentVisitor)
    }
}

pub struct CipPath {
    pub class_id: LogicalPathSegment,
    pub instance_id: LogicalPathSegment,
    pub attribute_id: LogicalPathSegment,
}

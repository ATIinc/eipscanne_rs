use std::fmt;

use serde::de::{Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

//  Tried to use Deku but that didn't support nested structs: https://github.com/sharksforarms/deku
use bilge::prelude::{bitsize, u2, u3, Bitsized, DebugBits, Number, TryFromBits};

#[bitsize(3)]
#[derive(Debug, Clone, TryFromBits, PartialEq)]
#[non_exhaustive]
pub enum SegmentType {
    LogicalSegment = 0x01,
}

#[bitsize(3)]
#[derive(Debug, Clone, TryFromBits, PartialEq)]
#[non_exhaustive]
pub enum LogicalSegmentType {
    ClassId = 0x00,
    InstanceId = 0x01,
    Sample = 0x05,
}

#[bitsize(2)]
#[derive(Debug, Clone, TryFromBits, PartialEq)]
#[non_exhaustive]
pub enum LogicalSegmentFormat {
    FormatAsU16 = 0x01,
    FormatAsUWhat = 0x03,
}

// The whole CipPath is a CipUint (16 bit number)
#[bitsize(32)]
#[derive(TryFromBits, PartialEq, DebugBits)]
pub struct LogicalPathSegment {
    // For some reason, the segment sections need to be inverted... Should be u3, u3, u2
    pub logical_segment_format: LogicalSegmentFormat,
    pub logical_segment_type: LogicalSegmentType,
    pub segment_type: SegmentType,
    pub padded_bytes: u8,
    pub data: u16,
}

impl Serialize for LogicalPathSegment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the u32 as little endian
        serializer.serialize_u32(self.value.to_le())
    }
}

struct LogicalPathSegmentVisitor;

impl<'de> Visitor<'de> for LogicalPathSegmentVisitor {
    type Value = LogicalPathSegment;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer between 0 and 2^32")
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let logical_segment = LogicalPathSegment::try_from(value).unwrap();
        Ok(logical_segment)
    }
}

impl<'de> Deserialize<'de> for LogicalPathSegment {
    fn deserialize<D>(deserializer: D) -> Result<LogicalPathSegment, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(LogicalPathSegmentVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CipPath {
    pub class_id_segment: LogicalPathSegment,
    pub instance_id_segment: LogicalPathSegment,
}

impl CipPath {
    pub fn new(class_id: u16, instance_id: u16) -> CipPath {
        CipPath {
            class_id_segment: LogicalPathSegment::new(
                LogicalSegmentFormat::FormatAsU16,
                LogicalSegmentType::ClassId,
                SegmentType::LogicalSegment,
                0x0,
                class_id,
            ),
            instance_id_segment: LogicalPathSegment::new(
                LogicalSegmentFormat::FormatAsU16,
                LogicalSegmentType::InstanceId,
                SegmentType::LogicalSegment,
                0x0,
                instance_id,
            ),
        }
    }
}

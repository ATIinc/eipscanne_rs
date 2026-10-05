use std::io::{Seek, Write};

use binrw::{
    BinRead,
    BinResult,
    BinWrite, // #[binrw] attribute
    Endian,
    binrw,
};

//  Tried to use Deku but that didn't support nested structs: https://github.com/sharksforarms/deku
use bilge::prelude::{BuilderBits, DebugBits, FromBits, bitsize, u2, u3};

use crate::cip::types::CipUsint;

pub const BYTES_PER_PATH_WORD: usize = 2;

#[bitsize(3)]
#[derive(Debug, Clone, Copy, FromBits, PartialEq)]
#[repr(u8)]
pub enum SegmentType {
    PortSegment = 0x00,
    LogicalSegment = 0x01,
    NetworkSegment = 0x02,
    SymbolicSegment = 0x03,
    DataSegment = 0x04,

    #[fallback]
    Unknown(u3),
}

#[bitsize(3)]
#[derive(Debug, Clone, Copy, FromBits, PartialEq)]
#[repr(u8)]
pub enum LogicalSegmentType {
    ClassId = 0x00,
    InstanceId = 0x01,
    MemberId = 0x02,
    ConnectionPoint = 0x03,
    AttributeId = 0x04,
    Special = 0x05,
    ServiceId = 0x06,
    Reserved = 0x07,
}

#[bitsize(2)]
#[derive(Debug, FromBits, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum LogicalSegmentFormat {
    FormatAsU8 = 0x00,
    FormatAsU16 = 0x01,

    #[fallback]
    Unknown(u2),
}

#[bitsize(8)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits)]
#[br(map = u8::into)]
#[bw(map = |&x| u8::from(x))]
pub struct LogicalPathDefinition {
    // For some reason, the segment sections need to be inverted... Should be u3, u3, u2
    pub logical_segment_format: LogicalSegmentFormat,
    pub logical_segment_type: LogicalSegmentType,
    pub segment_type: SegmentType,
}

// NOTE: Could also investigate doing something that explicitly converts from and to a u32
// #[bitsize(32)]
// #[derive(DebugBits, FromBits, BinRead, BinWrite, PartialEq, Clone, Copy)]
// #[br(map = u32::into)]
// #[bw(map = |&x| u32::from(x))]

#[binrw]
#[derive(Debug, PartialEq, Clone, Copy)]
#[br(import(segment_format: LogicalSegmentFormat))]
pub enum PathData {
    #[br(pre_assert(segment_format == LogicalSegmentFormat::FormatAsU8))]
    FormatAsU8(u8),

    #[br(pre_assert(segment_format == LogicalSegmentFormat::FormatAsU16))]
    FormatAsU16(u16),
}

impl Into<u16> for PathData {
    /// Converts the path data to a u16, regardless of its underlying format.
    fn into(self) -> u16 {
        match self {
            PathData::FormatAsU8(data) => data as u16,
            PathData::FormatAsU16(data) => data,
        }
    }
}

impl<'a> Into<u16> for &'a PathData {
    fn into(self) -> u16 {
        match self {
            PathData::FormatAsU8(data) => *data as u16,
            PathData::FormatAsU16(data) => *data,
        }
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct LogicalPathSegment {
    // Only logical segments are described by this struct; refuse to interpret any other segment type
    #[br(assert(path_definition.segment_type() == SegmentType::LogicalSegment))]
    pub path_definition: LogicalPathDefinition,

    #[br(if (path_definition.logical_segment_format() == LogicalSegmentFormat::FormatAsU16))]
    pub u16_padding: Option<u8>,

    #[br(args(path_definition.logical_segment_format(),))]
    pub data: PathData,
}

// ======= Start of LogicalPathSegment impl ========

impl LogicalPathSegment {
    pub fn new_u8(logical_segment_type: LogicalSegmentType, data: u8) -> Self {
        LogicalPathSegment {
            path_definition: LogicalPathDefinition::builder()
                .logical_segment_format(LogicalSegmentFormat::FormatAsU8)
                .logical_segment_type(logical_segment_type)
                .segment_type(SegmentType::LogicalSegment)
                .build(),
            u16_padding: None,
            data: PathData::FormatAsU8(data),
        }
    }

    pub fn new_u16(logical_segment_type: LogicalSegmentType, data: u16) -> Self {
        LogicalPathSegment {
            path_definition: LogicalPathDefinition::builder()
                .logical_segment_format(LogicalSegmentFormat::FormatAsU16)
                .logical_segment_type(logical_segment_type)
                .segment_type(SegmentType::LogicalSegment)
                .build(),
            u16_padding: Some(0x0),
            data: PathData::FormatAsU16(data),
        }
    }

    /// Number of bytes this segment occupies on the wire
    pub fn byte_len(&self) -> usize {
        match self.data {
            PathData::FormatAsU8(_) => 2,
            PathData::FormatAsU16(_) => 4,
        }
    }
}

// ^^^^^^^^ End of LogicalPathSegment impl ^^^^^^^^

#[binrw::parser(reader, endian)]
fn parse_segments_until(word_len: u8) -> BinResult<Vec<LogicalPathSegment>> {
    let start_position = reader.stream_position()?;
    let end_position = start_position + (word_len as usize * BYTES_PER_PATH_WORD) as u64;

    let mut segments = Vec::new();
    while reader.stream_position()? < end_position {
        segments.push(LogicalPathSegment::read_options(reader, endian, ())?);
    }

    let final_position = reader.stream_position()?;
    if final_position != end_position {
        return Err(binrw::Error::AssertFail {
            pos: final_position,
            message: format!(
                "path segments overran the declared path length by {} bytes",
                final_position - end_position
            ),
        });
    }

    Ok(segments)
}

/// A padded path made of logical segments (Request Path / Connection Path in Wireshark).
///
/// Only logical segments are modelled: every path this library builds or parses consists of
/// class, instance, attribute and connection point segments. Reading takes the path size in
/// 16-bit words, which is how every packet carries it (Request Path Size, Connection Path Size).
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Default)]
#[br(import(path_word_size: u8))]
pub struct CipPath {
    #[br(parse_with = parse_segments_until, args(path_word_size))]
    pub segments: Vec<LogicalPathSegment>,
}

// ======= Start of CipPath impl ========

impl CipPath {
    pub const ASSEMBLY_CLASS_ID: u8 = 0x04;

    pub fn from_segments(segments: Vec<LogicalPathSegment>) -> Self {
        CipPath { segments }
    }

    /// `[class, instance]` using 16-bit logical segments
    pub fn new(class_id: u16, instance_id: u16) -> Self {
        Self::from_segments(vec![
            LogicalPathSegment::new_u16(LogicalSegmentType::ClassId, class_id),
            LogicalPathSegment::new_u16(LogicalSegmentType::InstanceId, instance_id),
        ])
    }

    /// `[class, instance, attribute]` using 8-bit logical segments
    pub fn new_full(class_id: u8, instance_id: u8, attribute_id: u8) -> Self {
        Self::from_segments(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, class_id),
            LogicalPathSegment::new_u8(LogicalSegmentType::InstanceId, instance_id),
            LogicalPathSegment::new_u8(LogicalSegmentType::AttributeId, attribute_id),
        ])
    }

    /// The usual I/O connection path to the Assembly object:
    /// configuration instance, then the O->T and T->O connection points, all as 8-bit segments
    pub fn new_assembly_connection(
        configuration_instance: u8,
        o2t_connection_point: u8,
        t2o_connection_point: u8,
    ) -> Self {
        Self::from_segments(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, Self::ASSEMBLY_CLASS_ID),
            LogicalPathSegment::new_u8(LogicalSegmentType::InstanceId, configuration_instance),
            LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, o2t_connection_point),
            LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, t2o_connection_point),
        ])
    }

    /// Number of bytes the path occupies on the wire
    pub fn byte_len(&self) -> usize {
        self.segments.iter().map(LogicalPathSegment::byte_len).sum()
    }

    /// Number of 16-bit words the path occupies on the wire (Request Path Size / Connection Path Size)
    pub fn word_len(&self) -> usize {
        self.byte_len().div_ceil(BYTES_PER_PATH_WORD)
    }

    /// Value of the first logical segment of the given type, regardless of its 8/16-bit format
    fn logical_value(&self, logical_segment_type: LogicalSegmentType) -> Option<u16> {
        self.segments
            .iter()
            .find(|segment| segment.path_definition.logical_segment_type() == logical_segment_type)
            .map(|segment| (&segment.data).into())
    }

    pub fn class_id(&self) -> Option<u16> {
        self.logical_value(LogicalSegmentType::ClassId)
    }

    pub fn instance_id(&self) -> Option<u16> {
        self.logical_value(LogicalSegmentType::InstanceId)
    }

    pub fn attribute_id(&self) -> Option<u16> {
        self.logical_value(LogicalSegmentType::AttributeId)
    }
}

// ^^^^^^^^ End of CipPath impl ^^^^^^^^

/// Writes a path preceded by its size in 16-bit words (Request Path Size / Connection Path Size).
///
/// Usable as a `write_with` function for a `CipPath` field.
pub fn write_path_with_word_size<W>(
    path: &CipPath,
    writer: &mut W,
    endian: Endian,
    _args: (),
) -> BinResult<()>
where
    W: Write + Seek,
{
    writer.write_all(&[path.word_len() as CipUsint])?;
    path.write_options(writer, endian, ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_data_into_u16_u8_variant() {
        let data = PathData::FormatAsU8(42);
        let value: u16 = data.into();
        assert_eq!(value, 42u16);
    }

    #[test]
    fn test_path_data_into_u16_u16_variant() {
        let data = PathData::FormatAsU16(4242);
        let value: u16 = data.into();
        assert_eq!(value, 4242u16);
    }

    #[test]
    fn test_path_data_ref_into_u16_u8_variant() {
        let data = PathData::FormatAsU8(42);
        let value: u16 = (&data).into();
        assert_eq!(value, 42u16);
    }

    #[test]
    fn test_path_data_ref_into_u16_u16_variant() {
        let data = PathData::FormatAsU16(4242);
        let value: u16 = (&data).into();
        assert_eq!(value, 4242u16);
    }

    #[test]
    fn test_cip_path_accessors_and_sizes() {
        let full_path = CipPath::new_full(0x04, 0x96, 0x03);
        assert_eq!(full_path.class_id(), Some(0x04));
        assert_eq!(full_path.instance_id(), Some(0x96));
        assert_eq!(full_path.attribute_id(), Some(0x03));
        assert_eq!(full_path.byte_len(), 6);
        assert_eq!(full_path.word_len(), 3);

        let class_instance = CipPath::new(0x0001, 0x0001);
        assert_eq!(class_instance.class_id(), Some(0x0001));
        assert_eq!(class_instance.attribute_id(), None);
        assert_eq!(class_instance.segments.len(), 2);
        assert_eq!(class_instance.word_len(), 4);

        let connection_path = CipPath::new_assembly_connection(0x97, 0x96, 0x64);
        assert_eq!(connection_path.byte_len(), 8);
        assert_eq!(connection_path.word_len(), 4);

        let mixed_path = CipPath::from_segments(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, 0x04),
            LogicalPathSegment::new_u16(LogicalSegmentType::InstanceId, 0x0096),
        ]);
        assert_eq!(mixed_path.byte_len(), 6);
        assert_eq!(mixed_path.word_len(), 3);
    }
}

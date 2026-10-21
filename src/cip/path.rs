use std::io::{Seek, Write};

use binrw::{BinRead, BinResult, BinWrite, Endian, binrw};
//  Tried to use Deku but that didn't support nested structs: https://github.com/sharksforarms/deku
use bilge::prelude::{BuilderBits, DebugBits, FromBits, bitsize, u2, u3, u7};

use crate::cip::object_ids::ASSEMBLY_CLASS_ID;
use crate::cip::types::{CipByte, CipUint, CipUsint};

const BYTES_PER_PATH_WORD: usize = 2;

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
    // Least significant bits first: format (bits 0-1), logical type (2-4), segment type (5-7)
    pub logical_segment_format: LogicalSegmentFormat,
    pub logical_segment_type: LogicalSegmentType,
    pub segment_type: SegmentType,
}

impl LogicalPathDefinition {
    /// Electronic Key Segment (0x34): the special logical type's first format
    pub fn electronic_key() -> Self {
        LogicalPathDefinition::builder()
            .logical_segment_format(LogicalSegmentFormat::FormatAsU8)
            .logical_segment_type(LogicalSegmentType::Special)
            .segment_type(SegmentType::LogicalSegment)
            .build()
    }
}

/// Major Revision (bits 0-6) and Compatibility (bit 7) of an electronic key. With Compatibility
/// clear the device must match the key exactly; set, it may accept any key it can emulate.
#[bitsize(8, new = pub)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits)]
#[br(map = CipByte::into)]
#[bw(map = |&x| CipByte::from(x))]
pub struct MajorRevisionCompatibility {
    pub major_revision: u7,
    pub compatibility: bool,
}

/// The data of an Electronic Key Segment: the device a connection path is meant for, checked
/// against its Identity object before the connection opens.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ElectronicKey {
    // Key Format 4, the only one defined
    #[br(temp, assert(key_format == 4))]
    #[bw(calc = 4)]
    key_format: CipUsint,

    pub vendor_id: CipUint,
    pub device_type: CipUint,
    pub product_code: CipUint,
    pub major_revision_compatibility: MajorRevisionCompatibility,
    pub minor_revision: CipUsint,
}

#[binrw]
#[derive(Debug, PartialEq, Clone, Copy)]
#[br(import(path_definition: LogicalPathDefinition))]
pub enum PathData {
    // Order matters: the electronic key's segment byte has the 8-bit format bits so this must come first
    #[br(pre_assert(path_definition == LogicalPathDefinition::electronic_key()))]
    ElectronicKey(ElectronicKey),

    #[br(pre_assert(path_definition.logical_segment_format() == LogicalSegmentFormat::FormatAsU8))]
    FormatAsU8(CipUsint),

    #[br(pre_assert(path_definition.logical_segment_format() == LogicalSegmentFormat::FormatAsU16))]
    FormatAsU16(CipUint),
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

    #[br(args(path_definition,))]
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

    /// Electronic Key Segment (0x34)
    pub fn new_electronic_key(electronic_key: ElectronicKey) -> Self {
        LogicalPathSegment {
            path_definition: LogicalPathDefinition::electronic_key(),
            u16_padding: None,
            data: PathData::ElectronicKey(electronic_key),
        }
    }

    /// Number of bytes this segment occupies on the wire
    fn byte_len(&self) -> usize {
        match self.data {
            PathData::FormatAsU8(_) => 2,
            PathData::FormatAsU16(_) => 4,
            PathData::ElectronicKey(_) => 10,
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
/// class, instance, attribute, connection point and electronic key segments. Reading takes the
/// path size in 16-bit words, which is how every packet carries it (Request Path Size,
/// Connection Path Size).
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

    /// `[class, instance]` using 8-bit logical segments. Both widths are valid for a value that
    /// fits, but 8-bit is what adapters expect: the Teknic IO-HUB refuses a Forward_Open whose
    /// request path uses 16-bit segments with a path segment error.
    pub fn new_u8(class_id: u8, instance_id: u8) -> Self {
        Self::from_segments(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, class_id),
            LogicalPathSegment::new_u8(LogicalSegmentType::InstanceId, instance_id),
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

    /// The usual I/O connection path to the Assembly object: configuration instance, then the
    /// originator to target and target to originator connection points, all as 8-bit segments
    pub fn new_assembly_connection(
        configuration_instance: u8,
        o2t_connection_point: u8,
        t2o_connection_point: u8,
    ) -> Self {
        Self::from_segments(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, ASSEMBLY_CLASS_ID),
            LogicalPathSegment::new_u8(LogicalSegmentType::InstanceId, configuration_instance),
            LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, o2t_connection_point),
            LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, t2o_connection_point),
        ])
    }

    /// Number of bytes the path occupies on the wire
    fn byte_len(&self) -> usize {
        self.segments.iter().map(LogicalPathSegment::byte_len).sum()
    }

    /// Number of 16-bit words the path occupies on the wire (Request Path Size / Connection Path Size)
    pub fn word_len(&self) -> usize {
        self.byte_len().div_ceil(BYTES_PER_PATH_WORD)
    }
}

// ^^^^^^^^ End of CipPath impl ^^^^^^^^

/// Writes a path preceded by its size in 16-bit words (Request Path Size / Connection Path Size).
///
/// Usable as a `write_with` function for a `CipPath` field.
pub(crate) fn write_path_with_word_size<W>(
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
    use crate::cip::object_ids::{
        ASSEMBLY_DATA_ATTRIBUTE_ID, IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID,
    };

    use super::*;

    #[test]
    fn test_cip_path_accessors_and_sizes() {
        let full_path = CipPath::new_full(ASSEMBLY_CLASS_ID, 0x96, ASSEMBLY_DATA_ATTRIBUTE_ID);
        let [class, instance, attribute] = full_path.segments.as_slice() else {
            panic!("expected class, instance and attribute segments");
        };
        assert_eq!(
            class.path_definition.logical_segment_type(),
            LogicalSegmentType::ClassId
        );
        assert_eq!(class.data, PathData::FormatAsU8(ASSEMBLY_CLASS_ID));
        assert_eq!(
            instance.path_definition.logical_segment_type(),
            LogicalSegmentType::InstanceId
        );
        assert_eq!(instance.data, PathData::FormatAsU8(0x96));
        assert_eq!(
            attribute.path_definition.logical_segment_type(),
            LogicalSegmentType::AttributeId
        );
        assert_eq!(
            attribute.data,
            PathData::FormatAsU8(ASSEMBLY_DATA_ATTRIBUTE_ID)
        );
        assert_eq!(full_path.byte_len(), 6);
        assert_eq!(full_path.word_len(), 3);

        let class_instance = CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID);
        let [class, instance] = class_instance.segments.as_slice() else {
            panic!("expected class and instance segments");
        };
        assert_eq!(
            class.path_definition.logical_segment_type(),
            LogicalSegmentType::ClassId
        );
        assert_eq!(class.data, PathData::FormatAsU16(IDENTITY_CLASS_ID));
        assert_eq!(
            instance.path_definition.logical_segment_type(),
            LogicalSegmentType::InstanceId
        );
        assert_eq!(class_instance.word_len(), 4);

        let connection_path = CipPath::new_assembly_connection(0x97, 0x96, 0x64);
        assert_eq!(connection_path.byte_len(), 8);
        assert_eq!(connection_path.word_len(), 4);

        let mixed_path = CipPath::from_segments(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, ASSEMBLY_CLASS_ID),
            LogicalPathSegment::new_u16(LogicalSegmentType::InstanceId, 0x0096),
        ]);
        assert_eq!(mixed_path.byte_len(), 6);
        assert_eq!(mixed_path.word_len(), 3);
    }
}

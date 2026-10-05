use std::io::{Cursor, Seek, Write};

use binrw::{
    binrw,
    BinRead,
    BinResult,
    BinWrite, // #[binrw] attribute
    Endian,
};

//  Tried to use Deku but that didn't support nested structs: https://github.com/sharksforarms/deku
use bilge::prelude::{bitsize, u2, u3, DebugBits, FromBits};

use crate::cip::types::{CipUsint, CipWord};

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

#[bitsize(8, new = pub)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
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
            path_definition: LogicalPathDefinition::new(
                LogicalSegmentFormat::FormatAsU8,
                logical_segment_type,
                SegmentType::LogicalSegment,
            ),
            u16_padding: None,
            data: PathData::FormatAsU8(data),
        }
    }

    pub fn new_u16(logical_segment_type: LogicalSegmentType, data: u16) -> Self {
        LogicalPathSegment {
            path_definition: LogicalPathDefinition::new(
                LogicalSegmentFormat::FormatAsU16,
                logical_segment_type,
                SegmentType::LogicalSegment,
            ),
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

/// A Simple Data Segment: application data (such as configuration) carried inside a path.
///
/// The segment type byte (0x80) is followed by the data size in 16-bit words and the data words.
#[binrw]
#[brw(little, magic = 0x80u8)]
#[derive(Debug, PartialEq, Clone)]
pub struct SimpleDataSegment {
    pub data_size: CipUsint,

    #[br(count = data_size)]
    pub data: Vec<CipWord>,
}

// ======= Start of SimpleDataSegment impl ========

impl SimpleDataSegment {
    pub fn new(data: Vec<CipWord>) -> Self {
        SimpleDataSegment {
            data_size: data.len() as CipUsint,
            data,
        }
    }

    /// Number of bytes this segment occupies on the wire
    pub fn byte_len(&self) -> usize {
        // segment type byte + data size byte + the data words
        2 + BYTES_PER_PATH_WORD * self.data.len()
    }
}

// ^^^^^^^^ End of SimpleDataSegment impl ^^^^^^^^

/// One segment of a padded EPATH.
///
/// The data segment is tried first because its segment type byte is a fixed value; a logical
/// segment accepts any byte whose segment type bits identify a logical segment.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub enum PathSegment {
    Data(SimpleDataSegment),
    Logical(LogicalPathSegment),
}

// ======= Start of PathSegment impl ========

impl PathSegment {
    /// Number of bytes this segment occupies on the wire
    pub fn byte_len(&self) -> usize {
        match self {
            PathSegment::Data(segment) => segment.byte_len(),
            PathSegment::Logical(segment) => segment.byte_len(),
        }
    }
}

impl From<LogicalPathSegment> for PathSegment {
    fn from(segment: LogicalPathSegment) -> Self {
        PathSegment::Logical(segment)
    }
}

impl From<SimpleDataSegment> for PathSegment {
    fn from(segment: SimpleDataSegment) -> Self {
        PathSegment::Data(segment)
    }
}

// ^^^^^^^^ End of PathSegment impl ^^^^^^^^

#[binrw::parser(reader, endian)]
fn parse_segments_until(byte_len: u16) -> BinResult<Vec<PathSegment>> {
    let start_position = reader.stream_position()?;
    let end_position = start_position + byte_len as u64;

    let mut segments = Vec::new();
    while reader.stream_position()? < end_position {
        segments.push(PathSegment::read_options(reader, endian, ())?);
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

/// A padded EPATH made of an arbitrary list of segments (Connection Path in Wireshark).
///
/// Reading needs the byte length of the path, which the surrounding packet always provides as a
/// word count.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Default)]
#[br(import(byte_len: u16))]
pub struct EPath {
    #[br(parse_with = parse_segments_until, args(byte_len))]
    pub segments: Vec<PathSegment>,
}

// ======= Start of EPath impl ========

impl EPath {
    pub const ASSEMBLY_CLASS_ID: u8 = 0x04;

    pub fn new(segments: Vec<PathSegment>) -> Self {
        EPath { segments }
    }

    /// `[class, instance]` using 16-bit logical segments (same layout as `CipPath::new`)
    pub fn new_class_instance(class_id: u16, instance_id: u16) -> Self {
        EPath::new(vec![
            LogicalPathSegment::new_u16(LogicalSegmentType::ClassId, class_id).into(),
            LogicalPathSegment::new_u16(LogicalSegmentType::InstanceId, instance_id).into(),
        ])
    }

    /// The usual I/O connection path to the Assembly object:
    /// configuration instance, then the O->T and T->O connection points, all as 8-bit segments
    pub fn new_assembly_connection(
        configuration_instance: u8,
        o2t_connection_point: u8,
        t2o_connection_point: u8,
    ) -> Self {
        EPath::new(vec![
            LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, Self::ASSEMBLY_CLASS_ID).into(),
            LogicalPathSegment::new_u8(LogicalSegmentType::InstanceId, configuration_instance)
                .into(),
            LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, o2t_connection_point)
                .into(),
            LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, t2o_connection_point)
                .into(),
        ])
    }

    pub fn push(&mut self, segment: impl Into<PathSegment>) {
        self.segments.push(segment.into());
    }

    /// Number of bytes the path occupies on the wire
    pub fn byte_len(&self) -> usize {
        self.segments.iter().map(PathSegment::byte_len).sum()
    }

    /// Number of 16-bit words the path occupies on the wire (Connection Path Size / Request Path Size)
    pub fn word_len(&self) -> usize {
        self.byte_len().div_ceil(BYTES_PER_PATH_WORD)
    }
}

// ^^^^^^^^ End of EPath impl ^^^^^^^^

/// Writes a path preceded by its size in 16-bit words.
///
/// Usable as a `write_with` function for any padded path type (`CipPath`, `EPath`, ...).
pub fn write_path_with_word_size<W, P>(
    path: &P,
    writer: &mut W,
    endian: Endian,
    _args: (),
) -> BinResult<()>
where
    W: Write + Seek,
    P: for<'a> BinWrite<Args<'a> = ()>,
{
    // Step 1: Write the path into a temporary buffer
    let mut temp_buffer = Vec::new();
    let mut temp_writer = Cursor::new(&mut temp_buffer);

    path.write_options(&mut temp_writer, endian, ())?;

    // Step 2: Calculate the path word size
    if temp_buffer.len() % BYTES_PER_PATH_WORD != 0 {
        return Err(binrw::Error::AssertFail {
            pos: writer.stream_position()?,
            message: format!(
                "a padded path must be word aligned but it is {} bytes long",
                temp_buffer.len()
            ),
        });
    }
    let path_word_size = temp_buffer.len() / BYTES_PER_PATH_WORD;

    // Step 3: Write the size followed by the path
    writer.write_all(&[path_word_size as CipUsint])?;
    writer.write_all(&temp_buffer)?;

    Ok(())
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(path_length: u8))]
pub struct CipPath {
    pub class_id_segment: LogicalPathSegment,
    pub instance_id_segment: LogicalPathSegment,

    #[br(if(path_length == 3))]
    pub attribute_id_segment: Option<LogicalPathSegment>,
}

// ======= Start of CipPath impl ========

impl CipPath {
    pub fn new(class_id: u16, instance_id: u16) -> Self {
        CipPath {
            class_id_segment: LogicalPathSegment::new_u16(LogicalSegmentType::ClassId, class_id),
            instance_id_segment: LogicalPathSegment::new_u16(
                LogicalSegmentType::InstanceId,
                instance_id,
            ),
            attribute_id_segment: None,
        }
    }

    pub fn new_full(class_id: u8, instance_id: u8, attribute_id: u8) -> Self {
        CipPath {
            class_id_segment: LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, class_id),
            instance_id_segment: LogicalPathSegment::new_u8(
                LogicalSegmentType::InstanceId,
                instance_id,
            ),
            attribute_id_segment: Some(LogicalPathSegment::new_u8(
                LogicalSegmentType::AttributeId,
                attribute_id,
            )),
        }
    }
}

// ^^^^^^^^ End of CipPath impl ^^^^^^^^

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
    fn test_epath_byte_and_word_len() {
        let mut path = EPath::new_assembly_connection(0x97, 0x96, 0x64);
        assert_eq!(path.byte_len(), 8);
        assert_eq!(path.word_len(), 4);

        path.push(SimpleDataSegment::new(vec![0x0001, 0x0002]));
        assert_eq!(path.byte_len(), 14);
        assert_eq!(path.word_len(), 7);

        let class_instance = EPath::new_class_instance(0x06, 0x01);
        assert_eq!(class_instance.byte_len(), 8);
    }
}

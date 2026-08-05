use binrw::{
    binrw,
    BinRead,
    BinWrite, // #[binrw] attribute
              // BinRead,  // trait for reading
              // BinWrite, // trait for writing
};

//  Tried to use Deku but that didn't support nested structs: https://github.com/sharksforarms/deku
use bilge::prelude::{bitsize, u2, u3, Bitsized, DebugBits, FromBits, Number};

#[bitsize(3)]
#[derive(Debug, Clone, FromBits, PartialEq)]
#[repr(u8)]
pub enum SegmentType {
    LogicalSegment = 0x01,

    #[fallback]
    Unknown(u3),
}

#[bitsize(3)]
#[derive(Debug, Clone, FromBits, PartialEq)]
#[repr(u8)]
pub enum LogicalSegmentType {
    ClassId = 0x00,
    InstanceId = 0x01,
    AttributeId = 0x04,

    #[fallback]
    Unknown(u3),
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
pub struct LogicalPathSegment {
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
}

// ^^^^^^^^ End of CipPath impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
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
}
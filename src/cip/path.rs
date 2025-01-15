use binrw::{
    binrw, // #[binrw] attribute
           // BinRead,  // trait for reading
           // BinWrite, // trait for writing
};

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
pub struct LogicalPathSegmentBits {
    // For some reason, the segment sections need to be inverted... Should be u3, u3, u2
    pub logical_segment_format: LogicalSegmentFormat,
    pub logical_segment_type: LogicalSegmentType,
    pub segment_type: SegmentType,
    pub padded_bytes: u8,
    pub data: u16,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct LogicalPathSegment {
    segment_representation: u32,
}

// ======= Start of LogicalPathSegment impl ========

impl From<LogicalPathSegment> for LogicalPathSegmentBits {
    fn from(segment: LogicalPathSegment) -> Self {
        LogicalPathSegmentBits::try_from(segment.segment_representation).unwrap()
    }
}

impl From<LogicalPathSegmentBits> for LogicalPathSegment {
    fn from(segment: LogicalPathSegmentBits) -> Self {
        LogicalPathSegment {
            segment_representation: segment.value,
        }
    }
}

// ^^^^^^^^ End of LogicalPathSegment impl ^^^^^^^^

#[derive(Debug, PartialEq)]
pub struct CipPathBits {
    pub class_id_segment: LogicalPathSegmentBits,
    pub instance_id_segment: LogicalPathSegmentBits,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct CipPath {
    pub class_id_segment: LogicalPathSegment,
    pub instance_id_segment: LogicalPathSegment,
}

// ======= Start of CipPath impl ========

impl CipPath {
    pub fn new(class_id: u16, instance_id: u16) -> CipPath {
        CipPath {
            class_id_segment: LogicalPathSegmentBits::new(
                LogicalSegmentFormat::FormatAsU16,
                LogicalSegmentType::ClassId,
                SegmentType::LogicalSegment,
                0x0,
                class_id,
            )
            .into(),
            instance_id_segment: LogicalPathSegmentBits::new(
                LogicalSegmentFormat::FormatAsU16,
                LogicalSegmentType::InstanceId,
                SegmentType::LogicalSegment,
                0x0,
                instance_id,
            )
            .into(),
        }
    }
}

impl From<CipPath> for CipPathBits {
    fn from(segment: CipPath) -> Self {
        CipPathBits {
            class_id_segment: LogicalPathSegmentBits::from(segment.class_id_segment),
            instance_id_segment: LogicalPathSegmentBits::from(segment.instance_id_segment),
        }
    }
}

impl From<CipPathBits> for CipPath {
    fn from(segment: CipPathBits) -> Self {
        CipPath {
            class_id_segment: LogicalPathSegment::from(segment.class_id_segment),
            instance_id_segment: LogicalPathSegment::from(segment.instance_id_segment),
        }
    }
}

// ^^^^^^^^ End of CipPath impl ^^^^^^^^
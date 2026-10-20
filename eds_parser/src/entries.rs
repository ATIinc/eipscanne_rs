//! The entries this crate reads, each a struct whose fields are the entry's fields in file order. `ConnectionEntry::deserialize(&entry)` fills one; an entry that does not
//! fit its struct fails with the field number.

use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::field_values::{
    DataFormat, MemberReference, NumberOrParam, ParamValue, PathTokens, QuotedString,
};
use crate::masks::{CipDataType, ConnectionParameters, ParamDescriptor, TriggerAndTransport};

/// `ParamN` of `[Params]`:
///
/// ```text
/// Param1 =
///     0,                          $ reserved
///     ,,                          $ link path size, link path
///     0x0004,                     $ descriptor
///     0xC8,                       $ data type: UDINT
///     4,                          $ data size in bytes
///     "RPI Range",                $ name
///     "us",                       $ units
///     "Requested packet interval",    $ help
///     1000,1000000,1000000,       $ minimum, maximum, default
///     1,1000,1,0,                 $ scaling: multiplier, divider, base, offset
///     ,,,,                        $ scaling links
///     1;                          $ decimal precision
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ParamEntry {
    reserved: IgnoredAny,
    link_path_size: IgnoredAny,
    link_path: IgnoredAny,
    /// `0x0004`: supports scaling
    pub descriptor: ParamDescriptor,
    /// `0xC8`: UDINT
    pub data_type: CipDataType,
    /// Bytes the value occupies on the wire
    pub data_size: u8,
    pub name: QuotedString,
    pub units: QuotedString,
    pub help: QuotedString,
    pub minimum: Option<ParamValue>,
    pub maximum: Option<ParamValue>,
    pub default: Option<ParamValue>,
    scaling_multiplier: IgnoredAny,
    scaling_divider: IgnoredAny,
    scaling_base: IgnoredAny,
    scaling_offset: IgnoredAny,
    multiplier_link: IgnoredAny,
    divisor_link: IgnoredAny,
    base_link: IgnoredAny,
    offset_link: IgnoredAny,
    decimal_precision: IgnoredAny,
    international_name: IgnoredAny,
    international_units: IgnoredAny,
    international_help: IgnoredAny,
}

/// `AssemN` of `[Assembly]`: six fields, then `member size, member reference` pairs up to the last
/// non-empty field:
///
/// ```text
/// Assem100 =
///     "Input Assembly",           $ name
///     "20 04 24 64 30 03",        $ path
///     5,                          $ size in bytes
///     0x0000,                     $ descriptor
///     ,,                          $ two reserved fields
///     16,Param1,                  $ members: size in bits, reference
///     16,Param2,
///     8,;
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssemblyEntry {
    pub name: Option<QuotedString>,
    pub path: Option<PathTokens>,
    /// Bytes of data
    pub size: Option<NumberOrParam>,
    descriptor: IgnoredAny,
    reserved_5: IgnoredAny,
    reserved_6: IgnoredAny,
    /// The pairs after field 6: `16,Param1, 16,Param2, 8,`
    pub members: Vec<AssemblyMember>,
}

/// One `member size, member reference` pair of an `AssemN` entry: `16,Param1,` (16 bits holding
/// Param1), `8,` (8 bits of opaque data), or `,` (neither)
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssemblyMember {
    pub size_bits: Option<u16>,
    pub reference: Option<MemberReference>,
}

/// `ConnectionN` of `[Connection Manager]`:
///
/// ```text
/// Connection1 =
///     0x04010002,                 $ trigger and transport
///     0x44440405,                 $ connection parameters
///     Param1,32,Assem150,         $ O->T RPI, size, format
///     Param1,,Assem100,           $ T->O RPI, size (empty: from Assem100), format
///     ,,                          $ configuration #1 size, format
///     0,Assem151,                 $ configuration #2 size, format
///     "Exclusive Owner",          $ name
///     "",                         $ help
///     "20 04 24 [Param2] 2C 96 2C 64";    $ path
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ConnectionEntry {
    pub trigger_and_transport: TriggerAndTransport,
    pub connection_parameters: ConnectionParameters,
    /// Microseconds
    pub o2t_rpi: Option<NumberOrParam>,
    /// Bytes of data, without the sequence count and real-time header
    pub o2t_size: Option<NumberOrParam>,
    pub o2t_format: Option<DataFormat>,
    pub t2o_rpi: Option<NumberOrParam>,
    pub t2o_size: Option<NumberOrParam>,
    pub t2o_format: Option<DataFormat>,
    config_1_size: IgnoredAny,
    config_1_format: IgnoredAny,
    config_2_size: IgnoredAny,
    config_2_format: IgnoredAny,
    pub name: QuotedString,
    pub help: QuotedString,
    pub path: PathTokens,
    safety_async: IgnoredAny,
    safety_max_consumer_number: IgnoredAny,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_values::PathToken;
    use crate::file::{self, Entry};

    fn entry(text: &str) -> Entry {
        file::sections(&format!("[S]\n{text}\n")).unwrap()[0].entries[0].clone()
    }

    #[test]
    fn a_param_reads_its_descriptor_type_size_and_values() {
        let param = ParamEntry::deserialize(&entry(
            "Param1 = 0,,,0x0014,0xC8,4,\"RPI Range\",\"us\",\"\",1000,1000000,10000,1,1000,1,0,,,,,1;",
        ))
        .unwrap();

        assert!(param.descriptor.supports_scaling());
        assert!(param.descriptor.read_only());
        assert!(!param.descriptor.monitor());
        assert_eq!(param.data_type, CipDataType::Udint);
        assert_eq!(param.data_size, 4);
        assert_eq!(param.name, QuotedString("RPI Range".to_string()));
        assert_eq!(param.minimum, Some(ParamValue::Integer(1000)));
        assert_eq!(param.default, Some(ParamValue::Integer(10_000)));
        assert_eq!(CipDataType::from(0xA0), CipDataType::Unknown(0xA0));
    }

    #[test]
    fn an_assembly_reads_its_members_in_pairs() {
        let assembly = AssemblyEntry::deserialize(&entry(
            "Assem100 = \"In\", \"20 04 24 64 30 03\", 5, 0x0000, , , 16,Param1, ,, 8,, ,;",
        ))
        .unwrap();

        assert_eq!(assembly.size, Some(NumberOrParam::Number(5)));
        assert_eq!(assembly.path.unwrap().0[3], PathToken::Byte(0x64));
        assert_eq!(
            assembly.members,
            vec![
                AssemblyMember {
                    size_bits: Some(16),
                    reference: Some(MemberReference::Param(1))
                },
                AssemblyMember {
                    size_bits: None,
                    reference: None
                },
                AssemblyMember {
                    size_bits: Some(8),
                    reference: None
                },
            ]
        );
        assert_eq!(
            AssemblyEntry::deserialize(&entry("Assem1 = \"a\", \"\", , 0, , , x,Param1;"))
                .unwrap_err()
                .to_string(),
            "Assem1: field 7: expected a number, found \"x\""
        );
    }

    #[test]
    fn a_connection_that_does_not_fit_its_struct_fails_with_the_field() {
        let error = |fields: &str| {
            ConnectionEntry::deserialize(&entry(&format!("Connection1 = {fields};")))
                .unwrap_err()
                .to_string()
        };

        assert_eq!(
            error("0x04010002, 0x44440405, x"),
            "Connection1: field 3: expected a number or a ParamN, found \"x\""
        );
        assert_eq!(
            error(
                "0x04010002, 0x44440405, 1, 32, , 1, 32, , , , , , \"n\", \"\", \"20 04\", , , 9"
            ),
            "Connection1: field 18: is past the last field of this kind of entry"
        );
    }
}

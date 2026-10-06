//! Checking a caller's assembly struct against the layout an EDS gives for it.
//!
//! Assemblies are plain `binrw` structs written by hand (from the layout `Assembly` prints), so
//! they keep their groups, named bits and helpers. A struct cannot be inspected field by field,
//! so the check probes it through its bytes, in three steps:
//!
//! ```text
//! Step      What is done                                        What it catches
//! --------  --------------------------------------------------  ---------------------------------
//! size      read `size` zero bytes, write them back             a missing, extra or wrong-width
//!                                                               field
//! coverage  per member: only its bits set, read, write back,    padding over bytes the EDS calls
//!           compare                                             data
//! values    per number member: a distinctive value, read, look  a field of the wrong width or
//!           for it in the struct's `Debug` output               signedness, reordered fields
//! ```
//!
//! Members whose param name starts with "Reserved" may be padding. Bit strings (BYTE, WORD,
//! DWORD, LWORD) are usually bitfields, so they only get the coverage step. A member whose probe
//! the struct refuses to read (an enum without that value) is reported as not checked rather
//! than as a mismatch. Two adjacent fields of the same type that are swapped pass every step:
//! only a test against captured data catches that.

use std::fmt;
use std::io::Cursor;

use binrw::{BinRead, BinWrite};

use crate::assembly::{Assembly, Member};
use crate::params::DataType;

/// One thing the check found about a member, or about the assembly as a whole
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// The assembly has no size to check against
    NoSize,
    /// The struct did not read from zero bytes
    ZerosRejected(String),
    /// The struct read a different number of bytes than the assembly has
    ReadSize { read: u64, size: u16 },
    /// The struct wrote a different number of bytes than the assembly has
    WriteSize { written: usize, size: u16 },
    /// The struct does not carry the member's bits through a read and a write: it pads over
    /// them
    Dropped { member: String },
    /// No field of the struct shows the value put in the member, but other numbers appeared
    WrongValue {
        member: String,
        expected: String,
        found: Vec<String>,
    },
    /// The member could not be probed; this is not a mismatch
    NotChecked { member: String, reason: String },
}

/// A struct that does not match its EDS assembly, with everything the check found
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyMismatch {
    /// `AssemN` of the assembly
    pub assembly: String,
    pub findings: Vec<Finding>,
}

/// Checks that `T` has the layout of `assembly`. On success, returns the members that could
/// not be probed (`Finding::NotChecked`), for the caller to look at; on failure, every finding.
pub fn check_assembly<T>(assembly: &Assembly) -> Result<Vec<Finding>, AssemblyMismatch>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + fmt::Debug,
{
    let mismatch = |findings| AssemblyMismatch {
        assembly: assembly.keyword.clone(),
        findings,
    };

    // ========= Size ============
    let Some(size) = assembly.size else {
        return Err(mismatch(vec![Finding::NoSize]));
    };
    // Room for a struct that is too long, so it reads and its length can be told
    let zeros = vec![0u8; usize::from(size) * 2 + 64];
    let (baseline, read) = match read::<T>(&zeros) {
        Ok(read) => read,
        Err(error) => return Err(mismatch(vec![Finding::ZerosRejected(error.to_string())])),
    };
    let mut size_findings = Vec::new();
    if read != u64::from(size) {
        size_findings.push(Finding::ReadSize { read, size });
    }
    match write(&baseline) {
        Ok(written) if written.len() == usize::from(size) => {}
        Ok(written) => size_findings.push(Finding::WriteSize {
            written: written.len(),
            size,
        }),
        Err(error) => size_findings.push(Finding::ZerosRejected(error.to_string())),
    }
    if !size_findings.is_empty() {
        return Err(mismatch(size_findings));
    }
    let baseline_numbers = numbers(&format!("{baseline:?}"));

    // ========= Coverage and values, member by member ============
    let mut findings = Vec::new();
    for member in &assembly.members {
        if let Some(finding) = probe_member::<T>(member, usize::from(size), &baseline_numbers) {
            findings.push(finding);
        }
    }

    let mismatched = findings
        .iter()
        .any(|finding| !matches!(finding, Finding::NotChecked { .. }));
    if mismatched {
        Err(mismatch(findings))
    } else {
        Ok(findings)
    }
}

/// The coverage step, then the values step, for one member: what went wrong, if anything
fn probe_member<T>(member: &Member, size: usize, baseline_numbers: &[String]) -> Option<Finding>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + fmt::Debug,
{
    let not_checked = |reason: String| {
        Some(Finding::NotChecked {
            member: member.describe(),
            reason,
        })
    };
    let reserved = member.param.as_ref().is_some_and(|param| {
        param
            .name
            .get(..8)
            .is_some_and(|start| start.eq_ignore_ascii_case("reserved"))
    });

    // ========= Coverage: every bit of the member set ============
    let mut bytes = vec![0u8; size];
    let first_bit = member.offset_bits as usize;
    for bit in first_bit..first_bit + usize::from(member.size_bits) {
        bytes[bit / 8] |= 1 << (bit % 8);
    }
    let value = match read::<T>(&bytes) {
        Ok((value, _)) => value,
        Err(_) => return not_checked("the struct refuses all ones (an enum?)".to_string()),
    };
    if write(&value).ok().as_ref() != Some(&bytes) {
        return if reserved {
            None
        } else {
            Some(Finding::Dropped {
                member: member.describe(),
            })
        };
    }

    // ========= Values: a distinctive number in the member ============
    let Some(param) = &member.param else {
        return None;
    };
    if reserved {
        return None;
    }
    let Some((probe, expected)) = probe_value(param.data_type) else {
        // Bit strings, BOOL and the other types are not numbers to look for
        return None;
    };
    if !member.offset_bits.is_multiple_of(8) || usize::from(member.size_bits) != probe.len() * 8 {
        return not_checked(format!(
            "{} bits at bit {} do not hold a whole {}",
            member.size_bits,
            member.offset_bits,
            param.data_type.name()
        ));
    }
    let mut bytes = vec![0u8; size];
    bytes[member.byte_range()].copy_from_slice(&probe);
    let value = match read::<T>(&bytes) {
        Ok((value, _)) => value,
        Err(_) => return not_checked(format!("the struct refuses {expected} (an enum?)")),
    };
    let shown: Vec<String> = numbers(&format!("{value:?}"))
        .into_iter()
        .filter(|number| !baseline_numbers.contains(number))
        .collect();
    if shown.contains(&expected) {
        None
    } else if shown.is_empty() {
        not_checked(format!(
            "no field shows {expected} as a number (an enum or a bitfield?)"
        ))
    } else {
        Some(Finding::WrongValue {
            member: member.describe(),
            expected: format!("{expected} ({})", param.data_type.name()),
            found: shown,
        })
    }
}

/// The bytes of a distinctive value of a number type, and how `Debug` shows it; `None` for the
/// types that are not numbers
fn probe_value(data_type: DataType) -> Option<(Vec<u8>, String)> {
    fn probe<V: fmt::Debug>(value: V, bytes: &[u8]) -> Option<(Vec<u8>, String)> {
        Some((bytes.to_vec(), format!("{value:?}")))
    }
    // Every byte non-zero and different from its neighbours, so a field of the wrong width
    // shows another number
    match data_type {
        DataType::Sint => probe(-86i8, &(-86i8).to_le_bytes()),
        DataType::Int => probe(-12_345i16, &(-12_345i16).to_le_bytes()),
        DataType::Dint => probe(-123_456_789i32, &(-123_456_789i32).to_le_bytes()),
        DataType::Lint => probe(
            -1_234_567_890_123_456_789i64,
            &(-1_234_567_890_123_456_789i64).to_le_bytes(),
        ),
        DataType::Usint => probe(0xA5u8, &0xA5u8.to_le_bytes()),
        DataType::Uint => probe(0xABCDu16, &0xABCDu16.to_le_bytes()),
        DataType::Udint => probe(0xABCD_EF01u32, &0xABCD_EF01u32.to_le_bytes()),
        DataType::Ulint => probe(
            0xABCD_EF01_2345_6789u64,
            &0xABCD_EF01_2345_6789u64.to_le_bytes(),
        ),
        DataType::Real => probe(1234.5f32, &1234.5f32.to_le_bytes()),
        DataType::Lreal => probe(98_765.432_1_f64, &98_765.432_1_f64.to_le_bytes()),
        DataType::Bool
        | DataType::Byte
        | DataType::Word
        | DataType::Dword
        | DataType::Lword
        | DataType::Other(_) => None,
    }
}

/// `T` read from the start of `bytes`, and how many bytes it took
fn read<T>(bytes: &[u8]) -> binrw::BinResult<(T, u64)>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut cursor = Cursor::new(bytes);
    let value = T::read_le(&mut cursor)?;
    Ok((value, cursor.position()))
}

fn write<T>(value: &T) -> binrw::BinResult<Vec<u8>>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    let mut cursor = Cursor::new(Vec::new());
    value.write_le(&mut cursor)?;
    Ok(cursor.into_inner())
}

/// Every number standing on its own in a `Debug` output (`-86`, `1234.5`), not the digits of a
/// name such as `motor0`
fn numbers(debug: &str) -> Vec<String> {
    let bytes = debug.as_bytes();
    let is_name = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut numbers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let negative = bytes[index] == b'-' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit);
        if !(negative || bytes[index].is_ascii_digit())
            || (start > 0 && (is_name(bytes[start - 1]) || bytes[start - 1] == b'.'))
        {
            index += 1;
            continue;
        }
        index += usize::from(negative);
        while index < bytes.len()
            && (bytes[index].is_ascii_digit()
                || (bytes[index] == b'.' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)))
        {
            index += 1;
        }
        if index == bytes.len() || !is_name(bytes[index]) {
            numbers.push(debug[start..index].to_string());
        }
    }
    numbers
}

// ======= Start of Finding impl ========

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::NoSize => write!(f, "the assembly gives no size"),
            Finding::ZerosRejected(error) => {
                write!(f, "the struct does not read from zero bytes: {error}")
            }
            Finding::ReadSize { read, size } => {
                write!(f, "the struct reads {read} bytes, the assembly has {size}")
            }
            Finding::WriteSize { written, size } => {
                write!(
                    f,
                    "the struct writes {written} bytes, the assembly has {size}"
                )
            }
            Finding::Dropped { member } => {
                write!(f, "{member}: the struct pads over it instead of keeping it")
            }
            Finding::WrongValue {
                member,
                expected,
                found,
            } => write!(
                f,
                "{member}: put in {expected}, the struct shows {}",
                found.join(", ")
            ),
            Finding::NotChecked { member, reason } => {
                write!(f, "{member}: not checked, {reason}")
            }
        }
    }
}

// ^^^^^^^^ End of Finding impl ^^^^^^^^

// ======= Start of AssemblyMismatch impl ========

impl fmt::Display for AssemblyMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the struct does not match {}:", self.assembly)?;
        for finding in &self.findings {
            write!(f, "\n  {finding}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AssemblyMismatch {}

// ^^^^^^^^ End of AssemblyMismatch impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use binrw::binrw;

    use crate::document::Document;
    use crate::params::Param;

    use super::*;

    /// Status (WORD), position (DINT), speed (UINT), a reserved byte, mode (USINT)
    const EDS: &str = r#"
[Params]
    Param1 = 0,,,0,0xD2,2,"Status","","",,,0,,,,,,,,,;
    Param2 = 0,,,0,0xC4,4,"Position","counts","",,,0,,,,,,,,,;
    Param3 = 0,,,0,0xC7,2,"Speed","rpm","",,,0,,,,,,,,,;
    Param4 = 0,,,0,0xD1,1,"Reserved Byte","","",,,0,,,,,,,,,;
    Param5 = 0,,,0,0xC6,1,"Mode","","",,,0,,,,,,,,,;
[Assembly]
    Assem100 = "Inputs", "20 04 24 64 30 03", 10, 0x0000, , ,
        16,Param1, 32,Param2, 16,Param3, 8,Param4, 8,Param5;
"#;

    fn assembly() -> Assembly {
        let document = Document::parse(EDS).unwrap();
        let params = Param::all(&document).unwrap();
        Assembly::all(&document, &params).unwrap().remove(0)
    }

    fn findings<T>() -> Vec<Finding>
    where
        T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + fmt::Debug,
    {
        match check_assembly::<T>(&assembly()) {
            Ok(not_checked) => not_checked,
            Err(mismatch) => mismatch.findings,
        }
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct Correct {
        status: u16,
        position: i32,
        speed: u16,
        _reserved: u8,
        mode: u8,
    }

    #[test]
    fn a_matching_struct_passes() {
        assert_eq!(check_assembly::<Correct>(&assembly()), Ok(vec![]));
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct ReservedAsPadding {
        status: u16,
        position: i32,
        #[brw(pad_after = 1)]
        speed: u16,
        mode: u8,
    }

    #[test]
    fn a_reserved_member_may_be_padding() {
        assert_eq!(check_assembly::<ReservedAsPadding>(&assembly()), Ok(vec![]));
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct MissingMode {
        status: u16,
        position: i32,
        speed: u16,
        _reserved: u8,
    }

    #[test]
    fn a_missing_field_is_the_wrong_size() {
        assert_eq!(
            findings::<MissingMode>(),
            vec![
                Finding::ReadSize { read: 9, size: 10 },
                Finding::WriteSize {
                    written: 9,
                    size: 10
                }
            ]
        );
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct PositionAsPadding {
        #[brw(pad_after = 4)]
        status: u16,
        speed: u16,
        _reserved: u8,
        mode: u8,
    }

    #[test]
    fn padding_over_data_is_dropped_data() {
        assert_eq!(
            findings::<PositionAsPadding>(),
            vec![Finding::Dropped {
                member: "Param2 \"Position\" at byte 2".to_string()
            }]
        );
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct PositionInHalves {
        status: u16,
        position_low: u16,
        position_high: u16,
        speed: u16,
        _reserved: u8,
        mode: u8,
    }

    #[test]
    fn two_halves_of_a_dint_show_other_numbers() {
        // -123456789 is 0xF8A432EB: halves 0x32EB and 0xF8A4
        assert_eq!(
            findings::<PositionInHalves>(),
            vec![Finding::WrongValue {
                member: "Param2 \"Position\" at byte 2".to_string(),
                expected: "-123456789 (DINT)".to_string(),
                found: vec!["13035".to_string(), "63652".to_string()],
            }]
        );
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct SpeedBeforePosition {
        status: u16,
        speed: u16,
        position: i32,
        _reserved: u8,
        mode: u8,
    }

    #[test]
    fn reordered_fields_of_different_types_show_other_numbers() {
        let findings = findings::<SpeedBeforePosition>();

        let wrong: Vec<&str> = findings
            .iter()
            .filter_map(|finding| match finding {
                Finding::WrongValue { member, .. } => Some(member.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            wrong,
            vec![
                "Param2 \"Position\" at byte 2",
                "Param3 \"Speed\" at byte 6"
            ]
        );
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct SignedSpeed {
        status: u16,
        position: i32,
        speed: i16,
        _reserved: u8,
        mode: u8,
    }

    #[test]
    fn a_signed_field_for_an_unsigned_member_shows_another_number() {
        assert_eq!(
            findings::<SignedSpeed>(),
            vec![Finding::WrongValue {
                member: "Param3 \"Speed\" at byte 6".to_string(),
                expected: "43981 (UINT)".to_string(),
                found: vec!["-21555".to_string()],
            }]
        );
    }

    #[binrw]
    #[brw(little, repr = u8)]
    #[derive(Debug)]
    enum Mode {
        Off = 0,
        On = 1,
    }

    #[binrw]
    #[brw(little)]
    #[derive(Debug)]
    struct ModeAsEnum {
        status: u16,
        position: i32,
        speed: u16,
        _reserved: u8,
        mode: Mode,
    }

    #[test]
    fn an_enum_that_refuses_the_probe_is_not_checked() {
        let not_checked = check_assembly::<ModeAsEnum>(&assembly()).unwrap();

        assert!(matches!(
            not_checked.as_slice(),
            [Finding::NotChecked { member, .. }] if member == "Param5 \"Mode\" at byte 9"
        ));
    }

    #[test]
    fn an_assembly_without_a_size_cannot_be_checked() {
        let mut assembly = assembly();
        assembly.size = None;

        assert_eq!(
            check_assembly::<Correct>(&assembly).unwrap_err().findings,
            vec![Finding::NoSize]
        );
    }

    #[test]
    fn numbers_stand_on_their_own() {
        assert_eq!(
            numbers("Motor { motor0: -86, io12: [1234.5, 7], name: x9, at: 3.0e5 }"),
            vec!["-86", "1234.5", "7"]
        );
    }

    #[test]
    fn a_mismatch_lists_every_finding() {
        let mismatch = check_assembly::<SignedSpeed>(&assembly()).unwrap_err();

        assert_eq!(
            mismatch.to_string(),
            "the struct does not match Assem100:\n  Param3 \"Speed\" at byte 6: put in 43981 (UINT), the struct shows -21555"
        );
    }
}

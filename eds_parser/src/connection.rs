//! Step 3c: the `[Connection Manager]` section. A `ConnectionN` entry says what the device
//! supports for one connection and where its data lives; its references to params and
//! assemblies are resolved here, so the bridge only maps values.

use bilge::prelude::{DebugBits, FromBits, bitsize, u2, u3, u5};
use pest::Parser;

use crate::assembly::{self, Assembly};
use crate::document::{Document, EdsParser, Entry, Field, Rule, syntax_error};
use crate::error::EdsError;
use crate::params::{self, Param, integer_field, is_param_reference, optional_integer};

/// The first mask word of a connection: which transport classes, triggers and application types
/// the device supports on it
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone)]
pub struct TriggerAndTransport {
    /// Bit n set: transport class n is supported
    pub transport_classes: u16,
    pub cyclic: bool,
    pub change_of_state: bool,
    pub application: bool,
    reserved_19_23: u5,
    pub listen_only: bool,
    pub input_only: bool,
    pub exclusive_owner: bool,
    pub redundant_owner: bool,
    reserved_28_30: u3,
    /// Ignored by the bridge: vendors disagree on it, and a Forward_Open's originator is
    /// always the client
    pub server: bool,
}

/// The second mask word of a connection: sizes, real-time formats, connection types and
/// priorities the device supports, per direction
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone)]
pub struct ConnectionParameters {
    pub o2t_fixed_size: bool,
    pub o2t_variable_size: bool,
    pub t2o_fixed_size: bool,
    pub t2o_variable_size: bool,
    obsolete_o2t_bytes_per_slot: u2,
    obsolete_t2o_bytes_per_slot: u2,
    /// 0 modeless, 1 zero length, 2 (reserved), 3 heartbeat, 4 32-bit header, 5-7 others
    pub o2t_real_time_format: u3,
    reserved_11: bool,
    pub t2o_real_time_format: u3,
    reserved_15: bool,
    pub o2t_null: bool,
    pub o2t_multicast: bool,
    pub o2t_point_to_point: bool,
    reserved_19: bool,
    pub t2o_null: bool,
    pub t2o_multicast: bool,
    pub t2o_point_to_point: bool,
    reserved_23: bool,
    pub o2t_low_priority: bool,
    pub o2t_high_priority: bool,
    pub o2t_scheduled_priority: bool,
    reserved_27: bool,
    pub t2o_low_priority: bool,
    pub t2o_high_priority: bool,
    pub t2o_scheduled_priority: bool,
    reserved_31: bool,
}

/// One direction of a connection, with its references resolved
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionSpec {
    /// Requested packet interval in microseconds; `None` when the EDS gives none
    pub requested_packet_interval: Option<u32>,
    /// Bytes of application data, without the sequence count and real-time header
    pub size: u16,
    /// The `AssemN` the data has the layout of, when given
    pub format: Option<String>,
}

/// Configuration data the connection declares: read and kept, never sent
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfigurationData {
    pub proxy_size: Option<u16>,
    pub proxy_format: Option<String>,
    pub target_size: Option<u16>,
    pub target_format: Option<String>,
}

/// A `ConnectionN` entry
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    /// `ConnectionN`, as written in the file
    pub keyword: String,
    pub name: String,
    pub trigger_and_transport: TriggerAndTransport,
    pub connection_parameters: ConnectionParameters,
    pub o2t: DirectionSpec,
    pub t2o: DirectionSpec,
    pub configuration: ConfigurationData,
    /// The connection path bytes, with every `[ParamN]` replaced by the param's default
    pub path: Vec<u8>,
}

/// Field positions of a `ConnectionN` entry
const TRIGGER_AND_TRANSPORT: usize = 0;
const CONNECTION_PARAMETERS: usize = 1;
const O2T_RPI: usize = 2;
const O2T_SIZE: usize = 3;
const O2T_FORMAT: usize = 4;
const T2O_RPI: usize = 5;
const T2O_SIZE: usize = 6;
const T2O_FORMAT: usize = 7;
const PROXY_CONFIG_SIZE: usize = 8;
const PROXY_CONFIG_FORMAT: usize = 9;
const TARGET_CONFIG_SIZE: usize = 10;
const TARGET_CONFIG_FORMAT: usize = 11;
const NAME: usize = 12;
const PATH: usize = 14;

// ======= Start of Connection impl ========

impl Connection {
    /// Every `ConnectionN` entry of the `[Connection Manager]` section, in file order
    pub fn all(
        document: &Document,
        params: &[Param],
        assemblies: &[Assembly],
    ) -> Result<Vec<Connection>, EdsError> {
        let section = document
            .section("Connection Manager")
            .ok_or_else(|| EdsError::MissingSection("Connection Manager".to_string()))?;
        section
            .numbered_entries("Connection")
            .map(|entry| Connection::from_entry(entry, params, assemblies))
            .collect()
    }

    fn from_entry(
        entry: &Entry,
        params: &[Param],
        assemblies: &[Assembly],
    ) -> Result<Connection, EdsError> {
        let trigger_and_transport: u32 = integer_field(
            entry,
            TRIGGER_AND_TRANSPORT,
            "the trigger and transport mask",
        )?;
        let connection_parameters: u32 = integer_field(
            entry,
            CONNECTION_PARAMETERS,
            "the connection parameters mask",
        )?;

        let o2t = direction(
            entry, O2T_RPI, O2T_SIZE, O2T_FORMAT, "O->T", params, assemblies,
        )?;
        let t2o = direction(
            entry, T2O_RPI, T2O_SIZE, T2O_FORMAT, "T->O", params, assemblies,
        )?;

        let configuration = ConfigurationData {
            proxy_size: optional_size(entry, PROXY_CONFIG_SIZE, params)?,
            proxy_format: entry
                .field(PROXY_CONFIG_FORMAT)
                .as_word()
                .map(str::to_string),
            target_size: optional_size(entry, TARGET_CONFIG_SIZE, params)?,
            target_format: entry
                .field(TARGET_CONFIG_FORMAT)
                .as_word()
                .map(str::to_string),
        };

        let name = entry.field(NAME).as_text().unwrap_or("").to_string();
        let path_text = entry
            .field(PATH)
            .as_text()
            .ok_or_else(|| EdsError::BadField {
                entry: entry.keyword.clone(),
                index: PATH,
                expected: "the connection path as a quoted string",
                found: entry.field(PATH).describe(),
            })?;
        let path = resolve_path(path_text, params, &entry.keyword)?;

        Ok(Connection {
            keyword: entry.keyword.clone(),
            name,
            trigger_and_transport: TriggerAndTransport::from(trigger_and_transport),
            connection_parameters: ConnectionParameters::from(connection_parameters),
            o2t,
            t2o,
            configuration,
            path,
        })
    }
}

// ^^^^^^^^ End of Connection impl ^^^^^^^^

/// One direction's RPI, size and format fields. The RPI is a number or a `ParamN` default; the
/// size is a number, a `ParamN` default, or, when empty, the size of the format's `AssemN`.
fn direction(
    entry: &Entry,
    rpi_index: usize,
    size_index: usize,
    format_index: usize,
    direction_name: &str,
    params: &[Param],
    assemblies: &[Assembly],
) -> Result<DirectionSpec, EdsError> {
    let requested_packet_interval = match value_or_param(entry, rpi_index, params)? {
        None => None,
        Some(value) => Some(u32::try_from(value).map_err(|_| EdsError::BadField {
            entry: entry.keyword.clone(),
            index: rpi_index,
            expected: "a requested packet interval in microseconds",
            found: entry.field(rpi_index).describe(),
        })?),
    };

    let format = entry.field(format_index).as_word().map(str::to_string);

    let size = match optional_size(entry, size_index, params)? {
        Some(size) => size,
        None => {
            let Some(format) = &format else {
                return Err(EdsError::BadField {
                    entry: entry.keyword.clone(),
                    index: size_index,
                    expected: "a size, or a format to take the size from",
                    found: format!("{direction_name} size and format are both empty"),
                });
            };
            assembly::lookup(assemblies, format, &entry.keyword)?
                .size
                .ok_or_else(|| EdsError::BadField {
                    entry: entry.keyword.clone(),
                    index: size_index,
                    expected: "a size, or a format whose assembly has one",
                    found: format!("{direction_name} size empty and {format} has no size"),
                })?
        }
    };

    Ok(DirectionSpec {
        requested_packet_interval,
        size,
        format,
    })
}

/// A size field: a number, a `ParamN` default, or empty
fn optional_size(entry: &Entry, index: usize, params: &[Param]) -> Result<Option<u16>, EdsError> {
    match value_or_param(entry, index, params)? {
        None => Ok(None),
        Some(value) => u16::try_from(value)
            .map(Some)
            .map_err(|_| EdsError::BadField {
                entry: entry.keyword.clone(),
                index,
                expected: "a size in bytes",
                found: entry.field(index).describe(),
            }),
    }
}

/// A field that is a number, a `ParamN` whose default is used, or empty
fn value_or_param(entry: &Entry, index: usize, params: &[Param]) -> Result<Option<i64>, EdsError> {
    match entry.field(index) {
        Field::Word(word) if is_param_reference(word) => {
            let param = params::lookup(params, word, &entry.keyword)?;
            param.default.map(Some).ok_or_else(|| EdsError::BadField {
                entry: entry.keyword.clone(),
                index,
                expected: "a param with a default value",
                found: format!("{word}, which has no default"),
            })
        }
        Field::Word(word) => Err(EdsError::BadField {
            entry: entry.keyword.clone(),
            index,
            expected: "a number, a ParamN or nothing",
            found: format!("the word {word}"),
        }),
        _ => optional_integer(entry, index),
    }
}

/// The bytes of a path string, each `[ParamN]` replaced by the param's default in as many
/// little-endian bytes as its data size
pub(crate) fn resolve_path(text: &str, params: &[Param], entry: &str) -> Result<Vec<u8>, EdsError> {
    let path = EdsParser::parse(Rule::path, text)
        .map_err(syntax_error)?
        .next()
        .expect("the path rule matches once");

    let mut bytes = Vec::new();
    for pair in path.into_inner() {
        match pair.as_rule() {
            Rule::hex_byte => {
                bytes.push(u8::from_str_radix(pair.as_str(), 16).expect("two hex digits"));
            }
            Rule::param_ref => {
                let keyword = pair.as_str().trim_matches(['[', ']']);
                let param = params::lookup(params, keyword, entry)?;
                let default = param.default.ok_or_else(|| EdsError::BadField {
                    entry: entry.to_string(),
                    index: PATH,
                    expected: "a param with a default value in the path",
                    found: format!("{keyword}, which has no default"),
                })?;
                bytes.extend_from_slice(&default.to_le_bytes()[..usize::from(param.data_size)]);
            }
            _ => {}
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(keyword: &str, data_size: u8, default: i64) -> Param {
        Param {
            keyword: keyword.to_string(),
            name: String::new(),
            data_size,
            min: None,
            max: None,
            default: Some(default),
        }
    }

    fn assembly(keyword: &str, size: Option<u16>) -> Assembly {
        Assembly {
            keyword: keyword.to_string(),
            name: String::new(),
            size,
        }
    }

    /// The IO-HUB style entry: RPI from a param, sizes given, formats as assemblies
    const CONNECTION: &str = r#"
[Connection Manager]
    Connection1 =
        0x04010002,             $ class 1, cyclic, exclusive owner
        0x77750405,             $ fixed sizes, O->T 32-bit header, T->O modeless, P2P, all priorities
        Param999,148,Assem101,  $ O->T RPI, size, format
        Param999,,Assem100,     $ T->O RPI, size (from Assem100), format
        ,,                      $ proxy config size, format
        0,,                     $ target config size, format
        "Four motors",          $ Connection Name
        "",                     $ help string
        "20 04 24 [Param3] 2C 65 2C 64";    $ Path
"#;

    #[test]
    fn a_connection_resolves_its_params_and_assemblies() {
        let document = Document::parse(CONNECTION).unwrap();
        let params = [param("Param999", 4, 10_000), param("Param3", 1, 1)];
        let assemblies = [
            assembly("Assem100", Some(228)),
            assembly("Assem101", Some(148)),
        ];

        let connections = Connection::all(&document, &params, &assemblies).unwrap();

        let connection = &connections[0];
        assert_eq!(connection.keyword, "Connection1");
        assert_eq!(connection.name, "Four motors");
        assert_eq!(connection.trigger_and_transport.transport_classes(), 0x0002);
        assert!(connection.trigger_and_transport.cyclic());
        assert!(connection.trigger_and_transport.exclusive_owner());
        assert!(!connection.trigger_and_transport.input_only());
        assert_eq!(
            connection.connection_parameters.o2t_real_time_format(),
            u3::new(4)
        );
        assert_eq!(
            connection.connection_parameters.t2o_real_time_format(),
            u3::new(0)
        );
        assert!(connection.connection_parameters.o2t_point_to_point());
        assert!(connection.connection_parameters.t2o_scheduled_priority());
        assert_eq!(
            connection.o2t,
            DirectionSpec {
                requested_packet_interval: Some(10_000),
                size: 148,
                format: Some("Assem101".to_string()),
            }
        );
        assert_eq!(connection.t2o.size, 228, "taken from Assem100");
        assert_eq!(
            connection.configuration,
            ConfigurationData {
                target_size: Some(0),
                ..ConfigurationData::default()
            }
        );
        assert_eq!(
            connection.path,
            vec![0x20, 0x04, 0x24, 0x01, 0x2C, 0x65, 0x2C, 0x64]
        );
    }

    #[test]
    fn path_params_are_substituted_in_their_data_size() {
        let params = [param("Param1", 1, 0x97), param("Param2", 2, 0x0102)];

        let path = resolve_path(
            "20 04 24 [Param1] 2C [Param2] 2C 64",
            &params,
            "Connection1",
        )
        .unwrap();

        assert_eq!(
            path,
            vec![0x20, 0x04, 0x24, 0x97, 0x2C, 0x02, 0x01, 0x2C, 0x64]
        );
    }

    #[test]
    fn an_unknown_path_param_names_the_connection() {
        assert_eq!(
            resolve_path("20 04 24 [Param7]", &[], "Connection2").unwrap_err(),
            EdsError::UnknownReference {
                entry: "Connection2".to_string(),
                reference: "Param7".to_string(),
            }
        );
    }

    #[test]
    fn a_direction_without_size_or_format_is_an_error() {
        let document = Document::parse(
            "[Connection Manager]\nConnection1 = 0x04010002, 0x77750405, 1000, , , 1000, 32, Assem100, ,, ,, \"n\", \"\", \"20 04 24 01 2C 65 2C 64\";\n",
        )
        .unwrap();

        let error = Connection::all(&document, &[], &[assembly("Assem100", Some(32))]).unwrap_err();

        assert!(
            matches!(&error, EdsError::BadField { entry, index: 3, found, .. }
                if entry == "Connection1" && found.contains("O->T size and format are both empty")),
            "{error:?}"
        );
    }

    #[test]
    fn a_missing_connection_manager_section_is_an_error() {
        let document = Document::parse("[File]\nRevision = 1.0;\n").unwrap();
        assert_eq!(
            Connection::all(&document, &[], &[]).unwrap_err(),
            EdsError::MissingSection("Connection Manager".to_string())
        );
    }
}

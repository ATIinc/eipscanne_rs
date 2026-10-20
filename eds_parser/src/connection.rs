//! One `ConnectionEntry` as the values of the Forward_Open that opens it, or the reason this
//! scanner cannot open it: it opens class 1, cyclic, exclusive-owner connections, point-to-point
//! both ways.

use std::io::Cursor;

use bilge::prelude::u9;
use binrw::BinRead;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, ProductionTrigger,
    RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters, TransportClass,
    TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;

use crate::Eds;
use crate::entries::ConnectionEntry;
use crate::error::{Error, Result};
use crate::field_values::DataFormat;
use crate::masks::real_time_format;

/// One connection of an EDS file, as the Forward_Open values it decides. The caller writes the
/// `ForwardOpenRequest` from these and from what an EDS does not describe: tick time, timeouts,
/// connection IDs and the connection triad.
///
/// This entry, with `Param1` defaulting to 1000000, `Param2` to 0x97 and `Assem100` 32 bytes:
///
/// ```text
/// Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, Param1,,Assem100, ,, 0,Assem151,
///               "Exclusive Owner", "", "20 04 24 [Param2] 2C 96 2C 64";
/// ```
///
/// is the connection:
///
/// ```text
/// name: "Exclusive Owner"
/// transport_type_trigger: class 1, cyclic, client
/// O->T: every 1000000 us, 32 bytes, Header32Bit; parameters: 38 bytes (32 + 2 sequence count
///       + 4 run/idle header), fixed, scheduled, point-to-point, exclusive owner
/// T->O: every 1000000 us, 32 bytes (Assem100's), Modeless; parameters: 34 bytes, fixed,
///       scheduled, point-to-point, exclusive owner
/// connection_path: 20 04 24 97 2C 96 2C 64 (class 4, instance 0x97, connection points 0x96
///       and 0x64)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    /// `Exclusive Owner`
    pub name: String,
    pub transport_type_trigger: TransportTypeTrigger,
    pub o2t_requested_packet_interval: CipUdint,
    pub o2t_network_connection_parameters: NetworkConnectionParameters,
    /// How the outputs signal run/idle; the Forward_Open does not carry it
    pub o2t_real_time_format: RealTimeFormat,
    /// Bytes of output data, without the sequence count and real-time header
    pub o2t_size: u16,
    pub t2o_requested_packet_interval: CipUdint,
    pub t2o_network_connection_parameters: NetworkConnectionParameters,
    /// How the inputs signal run/idle; the Forward_Open does not carry it
    pub t2o_real_time_format: RealTimeFormat,
    /// Bytes of input data, without the sequence count and real-time header
    pub t2o_size: u16,
    pub connection_path: CipPath,
}

/// The largest connection size the 16-bit parameters of a Forward_Open hold
const MAX_STANDARD_CONNECTION_SIZE: u16 = 511;

// ======= Start of Connection impl ========

impl Connection {
    /// The Forward_Open values of a `ConnectionN` entry, or the reason this scanner cannot open
    /// it, with what the connection offers
    pub(crate) fn new(
        entry: &ConnectionEntry,
        eds: &Eds,
        large: Option<bool>,
    ) -> Result<Connection> {
        let trigger_and_transport = entry.trigger_and_transport;
        let connection_parameters = entry.connection_parameters;

        // 0x04010002: class 1 (bit 1), cyclic (bit 16), exclusive owner (bit 26)
        if !(trigger_and_transport.class_1()
            && trigger_and_transport.cyclic()
            && trigger_and_transport.exclusive_owner())
        {
            return Err(Error::Refused(format!(
                "this scanner opens class 1, cyclic, exclusive-owner connections; this one offers {trigger_and_transport:?}"
            )));
        }
        // 0x44440405: point-to-point O->T (bit 18) and T->O (bit 22), scheduled priority O->T
        // (bit 26) and T->O (bit 30)
        if !(connection_parameters.o2t_point_to_point()
            && connection_parameters.t2o_point_to_point())
        {
            return Err(Error::Refused(format!(
                "this scanner opens point-to-point connections both ways; this one offers {connection_parameters:?}"
            )));
        }
        let priority: ConnectionPriority =
            connection_parameters.shared_priority().ok_or_else(|| {
                Error::Refused(format!(
                    "no priority is supported both ways; this one offers {connection_parameters:?}"
                ))
            })?;

        // 0x44440405: O->T format 4 (bits 8-10, 32-bit header), T->O format 0 (bits 12-14,
        // modeless)
        let o2t_code = connection_parameters.o2t_real_time_format();
        let o2t_real_time_format: RealTimeFormat = real_time_format(o2t_code).ok_or_else(|| {
            Error::Refused(format!("O->T real-time format {o2t_code} is not supported"))
        })?;
        let t2o_code = connection_parameters.t2o_real_time_format();
        let t2o_real_time_format: RealTimeFormat = real_time_format(t2o_code).ok_or_else(|| {
            Error::Refused(format!("T->O real-time format {t2o_code} is not supported"))
        })?;

        // Param1,32,Assem150, (fields 3-5): O->T every Param1's default microseconds, 32 bytes
        let o2t_requested_packet_interval: CipUdint = match entry.o2t_rpi {
            Some(rpi) => eds.value(rpi)?,
            None => {
                return Err(Error::Refused(
                    "gives no O->T requested packet interval".into(),
                ));
            }
        };
        let o2t_size: u16 = match (entry.o2t_size, entry.o2t_format) {
            (Some(size), _) => eds.value(size)?,
            (None, Some(DataFormat::Assem(number))) => eds.assembly_size(number)?,
            (None, _) => {
                return Err(Error::Refused(
                    "O->T has no size, and no AssemN format that gives one".into(),
                ));
            }
        };
        // Param1,,Assem100, (fields 6-8): T->O every Param1's default microseconds, as many
        // bytes as Assem100
        let t2o_requested_packet_interval: CipUdint = match entry.t2o_rpi {
            Some(rpi) => eds.value(rpi)?,
            None => {
                return Err(Error::Refused(
                    "gives no T->O requested packet interval".into(),
                ));
            }
        };
        let t2o_size: u16 = match (entry.t2o_size, entry.t2o_format) {
            (Some(size), _) => eds.value(size)?,
            (None, Some(DataFormat::Assem(number))) => eds.assembly_size(number)?,
            (None, _) => {
                return Err(Error::Refused(
                    "T->O has no size, and no AssemN format that gives one".into(),
                ));
            }
        };

        // Sizes on the wire: the data plus the sequence count and the real-time header. O->T in
        // the example: 32 + 2 + 4 = 38; T->O: 32 + 2 = 34
        let o2t_connection_size: u16 =
            connection_size(o2t_size, TransportClass::Class1, o2t_real_time_format);
        let t2o_connection_size: u16 =
            connection_size(t2o_size, TransportClass::Class1, t2o_real_time_format);
        let large: bool = large
            .unwrap_or(o2t_connection_size.max(t2o_connection_size) > MAX_STANDARD_CONNECTION_SIZE);
        let o2t_network_connection_parameters = network_connection_parameters(
            o2t_connection_size,
            connection_parameters.o2t_fixed_size(),
            priority,
            large,
        )
        .ok_or_else(|| {
            Error::Refused(format!(
                "the O->T connection size of {o2t_connection_size} bytes needs a Large_Forward_Open"
            ))
        })?;
        let t2o_network_connection_parameters = network_connection_parameters(
            t2o_connection_size,
            connection_parameters.t2o_fixed_size(),
            priority,
            large,
        )
        .ok_or_else(|| {
            Error::Refused(format!(
                "the T->O connection size of {t2o_connection_size} bytes needs a Large_Forward_Open"
            ))
        })?;

        // "20 04 24 [Param2] 2C 96 2C 64": Param2's default in place of [Param2]
        let path: Vec<u8> = eds.path(&entry.path)?;
        let connection_path: CipPath = cip_path(&path).ok_or_else(|| {
            Error::Refused(format!(
                "its path {path:02X?} is not made of logical segments only"
            ))
        })?;

        Ok(Connection {
            name: entry.name.0.clone(),
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(TransportClass::Class1)
                .production_trigger(ProductionTrigger::Cyclic)
                .direction(Direction::Client)
                .build(),
            o2t_requested_packet_interval,
            o2t_network_connection_parameters,
            o2t_real_time_format,
            o2t_size,
            t2o_requested_packet_interval,
            t2o_network_connection_parameters,
            t2o_real_time_format,
            t2o_size,
            connection_path,
        })
    }
}

// ^^^^^^^^ End of Connection impl ^^^^^^^^

/// A point-to-point, exclusive-owner parameter word: 32-bit when `large`, else 16-bit, which
/// `None` means `connection_size` does not fit. The size is fixed when the direction supports it,
/// else variable. `(38, true, Scheduled, false)` is the 16-bit word for 38 bytes, fixed,
/// scheduled, point-to-point, exclusive owner.
fn network_connection_parameters(
    connection_size: u16,
    fixed_size_supported: bool,
    priority: ConnectionPriority,
    large: bool,
) -> Option<NetworkConnectionParameters> {
    let connection_size_type = match fixed_size_supported {
        true => ConnectionSizeType::Fixed,
        false => ConnectionSizeType::Variable,
    };
    if large {
        return Some(NetworkConnectionParameters::Large(
            LargeNetworkConnectionParameters::builder()
                .connection_size(connection_size)
                .connection_size_type(connection_size_type)
                .priority(priority)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        ));
    }
    Some(NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(u9::try_new(connection_size).ok()?)
            .connection_size_type(connection_size_type)
            .priority(priority)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    ))
}

/// Path bytes as a `CipPath`; `None` unless they are whole 16-bit words of logical segments.
/// `20 04 24 97 2C 96 2C 64` is class 4, instance 0x97, connection points 0x96 and 0x64.
fn cip_path(bytes: &[u8]) -> Option<CipPath> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let words = u8::try_from(bytes.len() / 2).ok()?;
    CipPath::read_args(&mut Cursor::new(bytes), (words,)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Class 1, cyclic, exclusive owner
    const TRIGGER_AND_TRANSPORT: u32 = 0x0401_0002;
    /// Fixed sizes both ways, O->T 32-bit header, T->O modeless, point-to-point both ways, every
    /// priority both ways
    const CONNECTION_PARAMETERS: u32 = 0x7775_0405;

    /// The IO-HUB style entry: RPI from a param, the T->O size from its format, a param in the
    /// path
    const IO_HUB: &str = r#"
[Params]
    Param3 = 0,,,0,0xC6,1,"","","",,,1;
    Param999 = 0,,,0,0xC8,4,"RPI","","",,,10000;
[Assembly]
    Assem100 = "Inputs", "", 228, 0, , ;
    Assem101 = "Outputs", "", 148, 0, , ;
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

    /// `Connection1` with the two masks and 32 bytes each way
    fn read(trigger_and_transport: u32, connection_parameters: u32) -> Result<Connection> {
        let eds = format!(
            "[Connection Manager]\nConnection1 = {trigger_and_transport:#x}, {connection_parameters:#x}, \
             1000000, 32, , 1000000, 32, , , , , , \"Exclusive owner\", \"\", \"20 04 24 97 2C 96 2C 64\";\n"
        );
        Eds::parse(&eds).unwrap().connection(1, None)
    }

    fn refusal<T: std::fmt::Debug>(result: Result<T>) -> String {
        result.unwrap_err().to_string()
    }

    fn standard(
        connection_size: u16,
        connection_size_type: ConnectionSizeType,
        priority: ConnectionPriority,
    ) -> NetworkConnectionParameters {
        NetworkConnectionParameters::Standard(
            StandardNetworkConnectionParameters::builder()
                .connection_size(u9::new(connection_size))
                .connection_size_type(connection_size_type)
                .priority(priority)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        )
    }

    #[test]
    fn a_connection_reads_into_forward_open_values() {
        let eds = Eds::parse(IO_HUB).unwrap();
        let connection = eds.connection(1, None).unwrap();

        assert_eq!(connection.name, "Four motors");
        assert_eq!(
            connection.o2t_requested_packet_interval, 10_000,
            "from Param999"
        );
        assert_eq!(connection.t2o_requested_packet_interval, 10_000);
        assert_eq!(connection.o2t_size, 148);
        assert_eq!(connection.t2o_size, 228, "from Assem100");
        assert_eq!(connection.o2t_real_time_format, RealTimeFormat::Header32Bit);
        assert_eq!(connection.t2o_real_time_format, RealTimeFormat::Modeless);
        assert_eq!(
            connection.o2t_network_connection_parameters,
            standard(
                154,
                ConnectionSizeType::Fixed,
                ConnectionPriority::Scheduled
            ),
            "148 bytes, the sequence count and the 32-bit header"
        );
        assert_eq!(
            connection.t2o_network_connection_parameters,
            standard(
                230,
                ConnectionSizeType::Fixed,
                ConnectionPriority::Scheduled
            )
        );
        assert_eq!(
            connection.connection_path,
            CipPath::new_assembly_connection(1, 0x65, 0x64),
            "[Param3] replaced by its default"
        );
    }

    #[test]
    fn a_refusal_shows_what_the_connection_offers() {
        let input_only = refusal(read(0x0201_0002, CONNECTION_PARAMETERS));
        assert!(
            input_only.starts_with(
                "Connection1: this scanner opens class 1, cyclic, exclusive-owner connections; \
                 this one offers TriggerAndTransport {"
            ),
            "{input_only}"
        );
        assert!(input_only.contains("input_only: true"), "{input_only}");

        assert!(refusal(read(0x0401_0001, CONNECTION_PARAMETERS)).contains("class_1: false"));
        assert!(refusal(read(0x0402_0002, CONNECTION_PARAMETERS)).contains("cyclic: false"));
        // bit 18 (O->T) and bit 22 (T->O) off in turn
        assert!(
            refusal(read(TRIGGER_AND_TRANSPORT, 0x7771_0405)).contains("o2t_point_to_point: false")
        );
        assert!(
            refusal(read(TRIGGER_AND_TRANSPORT, 0x7735_0405)).contains("t2o_point_to_point: false")
        );
        assert!(refusal(read(TRIGGER_AND_TRANSPORT, 0x1275_0405)).contains("no priority"));
        assert_eq!(
            refusal(read(TRIGGER_AND_TRANSPORT, 0x7775_0205)),
            "Connection1: O->T real-time format 2 is not supported"
        );
    }

    #[test]
    fn the_client_server_bit_is_ignored() {
        assert!(read(TRIGGER_AND_TRANSPORT | 0x8000_0000, CONNECTION_PARAMETERS).is_ok());
    }

    #[test]
    fn size_type_is_fixed_when_supported_else_variable() {
        let connection = read(TRIGGER_AND_TRANSPORT, 0x7775_040A).unwrap(); // variable only
        assert_eq!(
            connection.o2t_network_connection_parameters,
            standard(
                38,
                ConnectionSizeType::Variable,
                ConnectionPriority::Scheduled
            )
        );
        assert_eq!(
            connection.t2o_network_connection_parameters,
            standard(
                34,
                ConnectionSizeType::Variable,
                ConnectionPriority::Scheduled
            )
        );
    }

    #[test]
    fn a_direction_without_rpi_or_size_is_refused() {
        let eds = Eds::parse("[Connection Manager]\nConnection1 = 0x04010002, 0x77750405, 1000, 32, , , 32, , , , , , \"n\", \"\", \"20 04 24 01 2C 65 2C 64\";\n").unwrap();
        assert_eq!(
            refusal(eds.first_connection(None)),
            "[Connection Manager]: no connection this scanner can open:\n  \
             Connection1: gives no T->O requested packet interval"
        );

        let eds = Eds::parse("[Connection Manager]\nConnection1 = 0x04010002, 0x77750405, 1000, , , 1000, 32, , , , , , \"n\", \"\", \"20 04 24 01 2C 65 2C 64\";\n").unwrap();
        assert_eq!(
            refusal(eds.connection(1, None)),
            "Connection1: O->T has no size, and no AssemN format that gives one"
        );
    }

    #[test]
    fn a_large_forward_open_is_picked_when_a_size_needs_it_or_when_asked() {
        let eds = |t2o_size: u16| {
            Eds::parse(&format!(
                "[Connection Manager]\nConnection1 = 0x04010002, 0x77750405, 1000, 32, , 1000, {t2o_size}, , , , , , \"n\", \"\", \"20 04 24 01 2C 65 2C 64\";\n"
            ))
            .unwrap()
        };
        let t2o = |eds: Eds, large: Option<bool>| {
            eds.connection(1, large)
                .map(|connection| connection.t2o_network_connection_parameters)
        };

        // 510 bytes and the sequence count is 512, over the 511 of a Forward_Open
        assert!(matches!(
            t2o(eds(510), None),
            Ok(NetworkConnectionParameters::Large(parameters)) if parameters.connection_size() == 512
        ));
        assert!(matches!(
            t2o(eds(509), None),
            Ok(NetworkConnectionParameters::Standard(_))
        ));
        assert!(matches!(
            t2o(eds(32), Some(true)),
            Ok(NetworkConnectionParameters::Large(_))
        ));
        assert_eq!(
            refusal(eds(510).connection(1, Some(false))),
            "Connection1: the T->O connection size of 512 bytes needs a Large_Forward_Open"
        );
    }

    #[test]
    fn the_first_connection_is_the_first_one_this_scanner_can_open() {
        let eds = Eds::parse("[Connection Manager]
Connection1 = 0x02010002, 0x44440305, 1000, 0, , 1000, 32, , , , , , \"Input Only\", \"\", \"20 04 24 97 2C FE 2C 64\";
Connection2 = 0x04010002, 0x44440405, 1000, 32, , 1000, 32, , , , , , \"Exclusive Owner\", \"\", \"20 04 24 97 2C 96 2C 64\";
").unwrap();

        assert_eq!(eds.first_connection(None).unwrap().0, 2);
        assert_eq!(
            refusal(eds.connection(3, None)),
            "Connection3: the EDS has no such connection"
        );
        assert_eq!(
            refusal(
                Eds::parse("[File]\nRevision = 1.0;\n")
                    .unwrap()
                    .first_connection(None)
            ),
            "[Connection Manager]: the EDS has no ConnectionN entries"
        );
    }
}

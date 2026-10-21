//! EtherNet/IP encapsulated packets.
//!
//! How a SendRRData packet, as Wireshark shows it, maps onto the Rust types:
//!
//! ```text
//! Wireshark                                      Rust
//! ---------------------------------------------  --------------------------------------------------
//! EtherNet/IP (Industrial Protocol)              EnIpPacket
//!     Encapsulation Header                         .header: EncapsulationHeader
//!         Command                                    .command: EnIpCommand::SendRrData
//!         Length                                     .length (computed on write when None)
//!         Session Handle                             .session_handle
//!         Status                                     .status_code
//!         Sender Context                             .sender_context
//!         Options                                    .options
//!     Command Specific Data                        .command_specific_data: CommandSpecificData::SendRrData
//!         Interface Handle                           .interface_handle
//!         Timeout                                    .timeout
//!         Item Count                                 (not stored: 2 + the Socket Address Info items)
//!             Type ID: Null Address Item (0x0000)    .null_address_item.type_id
//!                 Length                               .null_address_item.packet_length
//!             Type ID: Unconnected Data Item (0x00b2)
//!                                                    .unconnected_data_item.type_id
//!                 Length                               .unconnected_data_item.packet_length
//!                                                      (computed on write when None)
//!             Type ID: Socket Address Info O->T (0x8000)
//!                                                    .socket_addr_info_items.o2t: Some(SocketAddrInfo)
//!             Type ID: Socket Address Info T->O (0x8001)
//!                                                    .socket_addr_info_items.t2o: Some(SocketAddrInfo)
//! Common Industrial Protocol                     .cip_message: CipMessage::Request / ::Response
//! ```
//!
//! The one structural difference: Wireshark shows the CIP message as its own top-level tree, while
//! here it is the data of the Unconnected Data Item, which is where its bytes are on the wire.
//!
//! A Send Unit Data packet (a class 3 connected message) has the same header and differs in its
//! command specific data:
//!
//! ```text
//! Wireshark                                      Rust
//! ---------------------------------------------  --------------------------------------------------
//!     Command Specific Data                        .command_specific_data: CommandSpecificData::SendUnitData
//!         Interface Handle                           .interface_handle
//!         Timeout                                    .timeout
//!         Item Count                                 (not stored: always 2)
//!             Type ID: Connected Address Item (0x00a1)
//!                                                    .connected_address_item.type_id
//!                 Length                               .connected_address_item.packet_length (4)
//!                 Connection ID                      .connection_id
//!             Type ID: Connected Data Item (0x00b1)  .connected_data_item.type_id
//!                 Length                               .connected_data_item.packet_length
//!                                                      (computed on write when None)
//!                 CIP Sequence Count                 .cip_sequence_count
//! Common Industrial Protocol                     .cip_message: CipMessage::Request / ::Response
//! ```
//! Register Session and Unregister Session packets have no items; their command specific data is
//! `CommandSpecificData::RegisterSession` (Protocol Version, Option Flags) or nothing.

use binrw::meta::WriteEndian;
use binrw::{
    BinWrite, // trait for writing
    binread,
    binwrite,
};

use crate::cip::message::CipMessage;
use crate::cip::message::response::MessageRouterResponse;
use crate::cip::types::{CipByte, CipUdint, CipUint};

use super::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode, UnitPacketData};
use super::constants as eip_constants;

#[binwrite]
#[binread]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
#[bw(import(packet_length: CipUint))]
pub struct EncapsulationHeader {
    pub command: EnIpCommand,
    #[bw(args(packet_length), write_with = header_length_writer)]
    pub length: Option<CipUint>,
    pub session_handle: CipUdint,
    pub status_code: EncapsStatusCode,
    pub sender_context: [CipByte; eip_constants::SENDER_CONTEXT_SIZE],
    pub options: CipUdint,
}

// ======= Start of EncapsulationHeader impl ========

#[binrw::writer(writer: writer, endian)]
fn header_length_writer(obj: &Option<CipUint>, arg0: u16) -> binrw::BinResult<()> {
    if obj.is_some() {
        let existing_value = obj.unwrap();
        return existing_value.write_options(writer, endian, ());
    }

    arg0.write_options(writer, endian, ())
}

// ^^^^^^^^ End of EncapsulationHeader impl ^^^^^^^^

/// A complete encapsulated packet: the encapsulation header followed by the command specific data
/// (which, for SendRRData, carries the Common Packet Format items and with them the CIP message).
#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EnIpPacket {
    pub header: EncapsulationHeader,

    #[br(args(header.command))]
    pub command_specific_data: CommandSpecificData,
    /* Passes the command field of the header to the command_specific_data field for binary reading */
}

// ======= Start of EnIpPacket impl ========

impl EnIpPacket {
    fn new(
        command: EnIpCommand,
        session_handle: CipUdint,
        command_specific_data: CommandSpecificData,
    ) -> Self {
        EnIpPacket {
            header: EncapsulationHeader {
                command,
                // will be calculated when serialized
                length: None,
                session_handle,
                status_code: EncapsStatusCode::Success,
                sender_context: eip_constants::EMPTY_SENDER_CONTEXT,
                options: eip_constants::DEFAULT_ENCAPSULATION_OPTIONS,
            },
            command_specific_data,
        }
    }

    pub fn new_registration() -> Self {
        EnIpPacket::new(
            EnIpCommand::RegisterSession,
            eip_constants::UNREGISTERED_SESSION_HANDLE,
            CommandSpecificData::new_registration(),
        )
    }

    pub fn new_unregistration(session_handle: CipUdint) -> Self {
        EnIpPacket::new(
            EnIpCommand::UnRegisterSession,
            session_handle,
            CommandSpecificData::UnregisterSession,
        )
    }

    /// A SendRRData packet carrying `message` as an unconnected message
    pub fn new_send_rr_data(
        session_handle: CipUdint,
        timeout: CipUint,
        message: impl Into<CipMessage>,
    ) -> Self {
        EnIpPacket::new(
            EnIpCommand::SendRrData,
            session_handle,
            CommandSpecificData::new_request(eip_constants::CIP_INTERFACE_HANDLE, timeout, message),
        )
    }

    /// A Send Unit Data packet carrying `message` on the connection `connection_id`
    pub fn new_send_unit_data(
        session_handle: CipUdint,
        connection_id: CipUdint,
        cip_sequence_count: CipUint,
        message: impl Into<CipMessage>,
    ) -> Self {
        EnIpPacket::new(
            EnIpCommand::SendUnitData,
            session_handle,
            CommandSpecificData::SendUnitData(UnitPacketData::new_connected(
                connection_id,
                cip_sequence_count,
                message,
            )),
        )
    }

    /// The Message Router response carried by the packet (Send RR Data or Send Unit Data), if it
    /// carries one
    pub fn response(&self) -> Option<&MessageRouterResponse> {
        let cip_message: &CipMessage = match &self.command_specific_data {
            CommandSpecificData::SendRrData(rr_data) => &rr_data.cip_message,
            CommandSpecificData::SendUnitData(unit_data) => &unit_data.cip_message,
            _ => return None,
        };
        match cip_message {
            CipMessage::Response(response) => Some(response),
            CipMessage::Request(_) => None,
        }
    }
}

impl WriteEndian for EnIpPacket {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl BinWrite for EnIpPacket {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        // Step 1: Serialize the `command_specific_data` field, its size is the Length of the header
        let mut command_specific_data_buffer = Vec::new();
        self.command_specific_data.write_options(
            &mut std::io::Cursor::new(&mut command_specific_data_buffer),
            endian,
            (),
        )?;

        // Step 2: Write the header with the length, then the command specific data
        self.header.write_options(
            writer,
            endian,
            (command_specific_data_buffer.len() as CipUint,),
        )?;
        writer.write_all(&command_specific_data_buffer)?;

        Ok(())
    }
}

// ^^^^^^^^ End of EnIpPacket impl ^^^^^^^^

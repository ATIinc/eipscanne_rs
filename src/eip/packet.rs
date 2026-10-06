use binrw::meta::WriteEndian;
use binrw::{
    BinWrite, // trait for writing
    binread,
    binwrite,
};

use crate::cip::types::{CipByte, CipUdint, CipUint};

use super::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use super::constants as eip_constants;
use super::description::{CipMessage, CommonPacketItem};

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
pub struct EnIpPacket<M: CipMessage> {
    pub header: EncapsulationHeader,

    #[br(args(header.command))]
    pub command_specific_data: CommandSpecificData<M>,
    /* Passes the command field of the header to the command_specific_data field for binary reading */
}

// ======= Start of EnIpPacket impl ========

impl<M: CipMessage> EnIpPacket<M> {
    pub fn new(
        command: EnIpCommand,
        session_handle: CipUdint,
        command_specific_data: CommandSpecificData<M>,
    ) -> Self {
        EnIpPacket {
            header: EncapsulationHeader {
                command,
                // will be calculated when serialized
                length: None,
                session_handle,
                status_code: EncapsStatusCode::Success,
                sender_context: [0x00; eip_constants::SENDER_CONTEXT_SIZE],
                options: 0x00,
            },
            command_specific_data,
        }
    }

    pub fn new_registration() -> Self {
        EnIpPacket::new(
            EnIpCommand::RegisterSession,
            0,
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
    pub fn new_send_rr_data(session_handle: CipUdint, timeout: CipUint, message: M) -> Self {
        EnIpPacket::new(
            EnIpCommand::SendRrData,
            session_handle,
            CommandSpecificData::new_request(0, timeout, message),
        )
    }

    /// The Common Packet Format items (empty for commands without them)
    pub fn items(&self) -> &[CommonPacketItem<M>] {
        self.command_specific_data.items()
    }

    /// The CIP message carried by the Unconnected Data Item, if any
    pub fn cip_message(&self) -> Option<&M> {
        match &self.command_specific_data {
            CommandSpecificData::SendRrData(rr_data) => rr_data.cip_message(),
            _ => None,
        }
    }

    /// The Sockaddr Info items of the packet, if any
    pub fn sockaddr_info_items(&self) -> impl Iterator<Item = &CommonPacketItem<M>> {
        self.items()
            .iter()
            .filter(|item| item.sockaddr_info().is_some())
    }

    /// Appends a Common Packet Format item (e.g. a Sockaddr Info item) to a SendRRData packet.
    /// Packets of other commands carry no items and are returned unchanged.
    pub fn with_item(mut self, item: CommonPacketItem<M>) -> Self {
        if let CommandSpecificData::SendRrData(ref mut rr_data) = self.command_specific_data {
            rr_data.items.push(item);
        }
        self
    }
}

impl<M: CipMessage> WriteEndian for EnIpPacket<M> {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl<M: CipMessage> BinWrite for EnIpPacket<M> {
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

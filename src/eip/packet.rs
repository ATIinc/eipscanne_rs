use binrw::meta::WriteEndian;
use binrw::{
    binread,
    binwrite,
    BinWrite, // trait for writing
};

use crate::cip::types::{CipByte, CipUdint, CipUint};

use super::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode, RegisterData};
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

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EnIpPacketDescription {
    pub header: EncapsulationHeader,

    #[br(args(header.command))]
    pub command_specific_data: CommandSpecificData,
    /* Passes the command field of the header to the command_specific_data field for binary reading */
}

// ======= Start of EnIpPacketDescription impl ========

impl EnIpPacketDescription {
    pub fn new(
        command: EnIpCommand,
        session_handle: CipUdint,
        command_specific_data: CommandSpecificData,
    ) -> Self {
        EnIpPacketDescription {
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

    pub fn new_registration_description() -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::RegisterSession,
            0,
            CommandSpecificData::RegisterSession(RegisterData {
                protocol_version: 1,
                option_flags: 0,
            }),
        )
    }

    pub fn new_unregistration_description(session_handle: CipUdint) -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::UnRegisterSession,
            session_handle,
            CommandSpecificData::UnregisterSession,
        )
    }

    pub fn new_cip_description(session_handle: CipUdint, timeout: CipUint) -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::SendRrData,
            session_handle,
            CommandSpecificData::new_request(0, timeout),
        )
    }
}

impl WriteEndian for EnIpPacketDescription {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl BinWrite for EnIpPacketDescription {
    // The EnIpPacketDescription is passed the packet_length
    type Args<'a> = (u16,);

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        // Step 1: Serialize the `command_specific_data` field
        let mut temp_buffer = Vec::new();
        let mut temp_writer = std::io::Cursor::new(&mut temp_buffer);

        let data_write_result =
            self.command_specific_data
                .write_options(&mut temp_writer, endian, args);

        if let Err(write_err) = data_write_result {
            return Err(write_err);
        };

        // Step 2: Calculate the total data size after header
        let full_proceeding_data_length = (temp_buffer.len() as u16) + args.0;

        // Step 3: Write the full struct to the actual writer
        if let Err(write_err) =
            self.header
                .write_options(writer, endian, (full_proceeding_data_length,))
        {
            return Err(write_err);
        }

        if let Err(write_err) = writer.write(&temp_buffer) {
            return Err(binrw::Error::Io(write_err));
        }

        Ok(())
    }
}

// ^^^^^^^^ End of EnIpPacketDescription impl ^^^^^^^^

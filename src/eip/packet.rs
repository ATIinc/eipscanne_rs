use binrw::meta::WriteEndian;
use binrw::{
    binread,
    binrw, // #[binrw] attribute
    binwrite,
    BinRead,  // trait for reading
    BinWrite, // trait for writing
};

use crate::cip::types::{CipByte, CipUdint, CipUint};

use crate::eip::constants as eip_constants;

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUint)]
#[bw(little, repr = CipUint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CommonPacketItemId {
    NullAddr = 0x0000,
    ListIdentity = 0x000C,
    ConnectionAddressItem = 0x00A1,
    ConnectedTransportPacket = 0x00B1,
    UnconnectedMessage = 0x00B2,
    O2TSockAddrInfo = 0x8000,
    T2OSockAddrInfo = 0x8001,
    SequencedAddressItem = 0x8002,
}

#[binrw::writer(writer: writer, endian)]
fn descripter_length_writer(_obj: &Option<CipUint>, arg0: Option<u16>) -> binrw::BinResult<()> {
    // If there isn't an input argument size, then just write 0
    let write_value = match arg0 {
        Some(value) => value,
        None => 0,
    };

    write_value.write_options(writer, endian, ())
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
#[bw(import(provided_packet_length: Option<u16>))]
pub struct CommonPacketDescriptor {
    pub type_id: CommonPacketItemId,

    #[bw(args(provided_packet_length), write_with = descripter_length_writer)]
    pub packet_length: Option<CipUint>,
}

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUint)]
#[bw(little, repr = CipUint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EnIpCommand {
    // Needs to be of type CipUint (u16)
    NOP = 0,
    ListServices = 0x0004,
    ListIdentity = 0x0063,
    ListInterfaces = 0x0064,
    RegisterSession = 0x0065,
    UnRegisterSession = 0x0066,
    SendRrData = 0x006F,
    SendUnitData = 0x0070,
    IndicateStatus = 0x0072,
    Cancel = 0x0073,
}

#[derive(BinRead, BinWrite)]
#[br(little, repr = CipUdint)]
#[bw(little, repr = CipUdint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EncapsStatusCode {
    // Needs to be of type CipUdint (u32)
    Success = 0x0000,
    UnsupportedCommand = 0x0001,
    InsufficientMemory = 0x0002,
    InvalidFormatOrData = 0x0003,
    InvalidSessionHandle = 0x0064,
    UnsupportedProtocolVersion = 0x0069,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[bw(import(provided_packet_length: u16))]
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,

    #[bw(calc = 2)]
    pub _item_count: CipUint,
    pub empty_data_packet: CommonPacketDescriptor,

    #[bw(args(Some(provided_packet_length)))]
    pub unconnected_data_packet: CommonPacketDescriptor,
}

// ======= Start of RRPacketData impl ========

impl RRPacketData {
    pub fn new_with_size(
        interface_handle: CipUdint,
        timeout: CipUint,
        unconnected_length: Option<u16>,
    ) -> Self {
        RRPacketData {
            interface_handle,
            timeout,
            empty_data_packet: CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: Some(0),
            },
            unconnected_data_packet: CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: unconnected_length,
            },
        }
    }

    pub fn new(interface_handle: CipUdint, timeout: CipUint) -> Self {
        Self::new_with_size(interface_handle, timeout, None)
    }
}

// ^^^^^^^^ End of RRPacketData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RegisterData {
    pub protocol_version: CipUint,
    pub option_flags: CipUint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(command_type: EnIpCommand))]
#[bw(import(provided_packet_length: u16))]
pub enum CommandSpecificData {
    #[br(pre_assert(command_type == EnIpCommand::UnRegisterSession))]
    UnregisterSession,

    #[br(pre_assert(command_type == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    #[br(pre_assert(command_type == EnIpCommand::SendRrData))]
    SendRrData(#[bw(args(provided_packet_length))] RRPacketData),
    /*  When reading -- make sure the provided command_type matches.
    When writing -- make sure the packet length is passed on */
}

// ======= Start of CommandSpecificData impl ========

impl CommandSpecificData {
    pub fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: 1,
            option_flags: 0,
        })
    }

    pub fn new_request(interface_handle: CipUdint, timeout: CipUint) -> Self {
        Self::SendRrData(RRPacketData::new(interface_handle, timeout))
    }
}

// ^^^^^^^^ End of CommandSpecificData impl ^^^^^^^^

#[binrw::writer(writer: writer, endian)]
fn header_length_writer(_obj: &Option<CipUint>, arg0: u16) -> binrw::BinResult<()> {
    arg0.write_options(writer, endian, ())
}

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

        // Step 4: Calculate the total data size after header
        let full_proceeding_data_length = (temp_buffer.len() as u16) + args.0;

        // Step 5: Write the full struct to the actual writer
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

// ^^^^^^^^ End of EnIpPacketDescription impl ^^^^^^^^

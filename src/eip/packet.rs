use binrw::meta::WriteEndian;
use binrw::{
    binread,
    binrw,    // #[binrw] attribute
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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct CommonPacketDescriptor {
    pub type_id: CommonPacketItemId,
    pub packet_length: CipUint,
}

pub fn generate_packet_descriptors(packet_size: usize) -> [CommonPacketDescriptor; 2] {
    [
        CommonPacketDescriptor {
            type_id: CommonPacketItemId::NullAddr,
            packet_length: 0,
        },
        CommonPacketDescriptor {
            type_id: CommonPacketItemId::UnconnectedMessage,
            packet_length: packet_size as u16,
        },
    ]
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
pub struct RRPacketData {
    pub interface_handle: CipUdint,
    pub timeout: CipUint,
    pub item_count: CipUint, // will always be 2
    pub cip_data_packets: [CommonPacketDescriptor; 2],
}

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
pub enum CommandSpecificData {
    #[br(pre_assert(command_type == EnIpCommand::UnRegisterSession))]
    UnregisterSession,

    #[br(pre_assert(command_type == EnIpCommand::RegisterSession))]
    RegisterSession(RegisterData),

    #[br(pre_assert(command_type == EnIpCommand::SendRrData))]
    SendRrData(RRPacketData),
}

// ======= Start of CommandSpecificData impl ========

impl CommandSpecificData {
    pub fn new_registration() -> Self {
        Self::RegisterSession(RegisterData {
            protocol_version: 1,
            option_flags: 0,
        })
    }

    pub fn new_request(interface_handle: CipUdint, timeout: CipUint, request_size: usize) -> Self {
        Self::SendRrData(RRPacketData {
            interface_handle,
            timeout,
            item_count: 2, // is always 2 because of the CipPacketDescriptor field
            cip_data_packets: generate_packet_descriptors(request_size),
        })
    }
}

// ======= Start of CommandSpecificData impl ========

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct EncapsulationHeader {
    pub command: EnIpCommand,
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

    #[br(args(header.command,))]
    pub command_specific_data: CommandSpecificData,
    /* Passes the command field of the header to the command_specific_data field for binary reading/writing */
}

// ======= Start of EnIpPacketDescription impl ========

impl WriteEndian for EnIpPacketDescription {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl BinWrite for EnIpPacketDescription {
    type Args<'a> = ();

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

        // Step 3: Calculate the data packet size
        let data_packet_byte_size: u16 = match &self.command_specific_data {
            CommandSpecificData::SendRrData(rr_data_ref) => {
                let mut running_total = 0;
                for descriptor in rr_data_ref.cip_data_packets {
                    running_total += descriptor.packet_length;
                }

                running_total
            }
            _ => 0,
        };

        // Step 4: Calculate the total data size after header
        let command_data_byte_size = temp_buffer.len();

        let mut updated_header = self.header.clone();
        updated_header.length = Some((command_data_byte_size as CipUint) + data_packet_byte_size);

        // Write the full struct to the actual writer
        if let Err(write_err) = updated_header.write_options(writer, endian, args) {
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
                // length variable is calculated when the full packet is written to bytes
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

    pub fn new_cip_description(
        session_handle: CipUdint,
        timeout: CipUint,
        request_size: usize,
    ) -> Self {
        EnIpPacketDescription::new(
            EnIpCommand::SendRrData,
            session_handle,
            CommandSpecificData::new_request(0, timeout, request_size),
        )
    }
}

// ======= Start of EnIpPacketDescription impl ========

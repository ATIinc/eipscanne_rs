use std::io::{Cursor, Seek, Write};

use binrw::meta::WriteEndian;
use binrw::{
    BinResult,
    BinWrite, // trait for writing
    Endian,
    binread,
};

use crate::cip::message::data::CipData;
use crate::cip::message::{
    request::MessageRouterRequest, response::MessageRouterResponse, shared::ServiceCode,
};
use crate::cip::path::CipPath;
use crate::cip::types::CipUdint;
use crate::eip::command::{BASE_ITEM_COUNT, CommandSpecificData};
use crate::eip::description::CommonPacketItem;
use crate::eip::packet::EnIpPacketDescription;

/// Writes a complete encapsulated packet: header, command specific data, the CIP message and any
/// Common Packet Format items that follow it.
///
/// The CIP message and the trailing items are serialized first so the lengths in the header and
/// in the Unconnected Data Item descriptor can be filled in.
fn write_assembly<W, M>(
    writer: &mut W,
    endian: Endian,
    packet_description: &EnIpPacketDescription,
    cip_message: &Option<M>,
    additional_items: &[CommonPacketItem],
) -> BinResult<()>
where
    W: Write + Seek,
    M: for<'a> BinWrite<Args<'a> = ()>,
{
    // Step 1: Serialize the `cip_message` field
    let mut cip_message_buffer = Vec::new();
    cip_message.write_options(&mut Cursor::new(&mut cip_message_buffer), endian, ())?;

    // Step 2: Serialize the items that follow the CIP message
    let mut additional_items_buffer = Vec::new();
    {
        let mut additional_items_writer = Cursor::new(&mut additional_items_buffer);
        for item in additional_items {
            item.write_options(&mut additional_items_writer, endian, ())?;
        }
    }

    // Step 3: Write the full packet, passing the sizes that are only known now
    let item_count = BASE_ITEM_COUNT + additional_items.len() as u16;
    packet_description.write_options(
        writer,
        endian,
        (
            cip_message_buffer.len() as u16,
            additional_items_buffer.len() as u16,
            item_count,
        ),
    )?;

    writer.write_all(&cip_message_buffer)?;
    writer.write_all(&additional_items_buffer)?;

    Ok(())
}

#[binread]
#[derive(Debug, PartialEq)]
pub struct RequestObjectAssembly {
    pub packet_description: EnIpPacketDescription,

    // Make sure that the MessageRouterRequest fails loudly if the command is SendRrData
    #[br(
        try,
        if(matches!(packet_description.command_specific_data, CommandSpecificData::SendRrData(_))),

        // Conditionally pass args depending on the command type
        args(if let CommandSpecificData::SendRrData(ref send_rr) = packet_description.command_specific_data {
            send_rr.unconnected_data_packet.packet_length.unwrap_or(0)
        } else {
            0
        })
    )]
    pub cip_message: Option<MessageRouterRequest>,

    // Items following the CIP message (e.g. Sockaddr Info items of a Forward_Open).
    // Only read when the CIP message was read, so a failed CIP read never gets interpreted as items.
    #[br(
        if(cip_message.is_some()),
        count = packet_description.command_specific_data.additional_item_count()
    )]
    pub additional_items: Vec<CommonPacketItem>,
}

// ======= Start of RequestObjectAssembly impl ========

impl WriteEndian for RequestObjectAssembly {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl BinWrite for RequestObjectAssembly {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        write_assembly(
            writer,
            endian,
            &self.packet_description,
            &self.cip_message,
            &self.additional_items,
        )
    }
}

// ^^^^^^^^ End of RequestObjectAssembly impl ^^^^^^^^

impl RequestObjectAssembly {
    pub fn new_registration() -> Self {
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_registration_description(),
            cip_message: None,
            additional_items: vec![],
        }
    }

    pub fn new_unregistration(session_handle: CipUdint) -> Self {
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_unregistration_description(
                session_handle,
            ),
            cip_message: None,
            additional_items: vec![],
        }
    }

    pub fn new_identity(session_handle: CipUdint) -> Self {
        Self::new_service_request(
            session_handle,
            CipPath::new(0x1, 0x1),
            ServiceCode::GetAttributeAll,
            None,
        )
    }
}

impl RequestObjectAssembly {
    pub fn new_service_request(
        session_handle: CipUdint,
        request_path: CipPath,
        service_code: ServiceCode,
        data: Option<Box<dyn CipData>>,
    ) -> Self {
        Self {
            packet_description: EnIpPacketDescription::new_cip_description(session_handle, 0),
            cip_message: Some(MessageRouterRequest::new_data(
                service_code,
                request_path,
                data,
            )),
            additional_items: vec![],
        }
    }

    /// Appends a Common Packet Format item after the CIP message (e.g. a Sockaddr Info item)
    pub fn with_additional_item(mut self, item: CommonPacketItem) -> Self {
        self.additional_items.push(item);
        self
    }
}

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseObjectAssembly {
    pub packet_description: EnIpPacketDescription,

    // TODO: Validate that the size of the EnIpPacketDescription correctly matches the remaining bytes
    //  * If the remaining bytes are 0, don't serialize the next step (otherwise do)
    #[br(
        try,
        if(matches!(packet_description.command_specific_data, CommandSpecificData::SendRrData(_))),

        // Conditionally pass args depending on the command type
        args(if let CommandSpecificData::SendRrData(ref send_rr) = packet_description.command_specific_data {
            send_rr.unconnected_data_packet.packet_length.unwrap_or(0)
        } else {
            0
        })
    )]
    pub cip_message: Option<MessageRouterResponse>,

    // Items following the CIP message (e.g. Sockaddr Info items of a Forward_Open reply).
    // Only read when the CIP message was read, so a failed CIP read never gets interpreted as items.
    #[br(
        if(cip_message.is_some()),
        count = packet_description.command_specific_data.additional_item_count()
    )]
    pub additional_items: Vec<CommonPacketItem>,
}

// ======= Start of ResponseObjectAssembly impl ========

impl WriteEndian for ResponseObjectAssembly {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl BinWrite for ResponseObjectAssembly {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        write_assembly(
            writer,
            endian,
            &self.packet_description,
            &self.cip_message,
            &self.additional_items,
        )
    }
}

impl ResponseObjectAssembly {
    /// The Sockaddr Info items that followed the CIP message, if any
    pub fn sockaddr_info_items(&self) -> impl Iterator<Item = &CommonPacketItem> {
        self.additional_items
            .iter()
            .filter(|item| item.sockaddr_info().is_some())
    }
}

// ^^^^^^^^ End of ResponseObjectAssembly impl ^^^^^^^^

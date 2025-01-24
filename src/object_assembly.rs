use binrw::meta::WriteEndian;
use binrw::{
    binread,
    BinRead,
    BinWrite, // trait for writing
};

use crate::cip::message::{
    request::MessageRouterRequest, response::MessageRouterResponse, shared::ServiceCode,
};
use crate::cip::path::CipPath;
use crate::cip::types::CipUdint;
use crate::eip::packet::EnIpPacketDescription;

#[derive(Debug, PartialEq)]
pub struct RequestObjectAssembly<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub packet_description: EnIpPacketDescription,
    pub cip_message: Option<MessageRouterRequest<T>>,
}

// ======= Start of RequestObjectAssembly impl ========

impl<T> WriteEndian for RequestObjectAssembly<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl<T> BinWrite for RequestObjectAssembly<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    type Args<'a> = ();

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        // Step 1: Serialize the `cip_message` field
        let mut temp_buffer = Vec::new();
        let mut temp_writer = std::io::Cursor::new(&mut temp_buffer);

        let cip_message_write_result =
            self.cip_message
                .write_options(&mut temp_writer, endian, args);

        if let Err(write_err) = cip_message_write_result {
            return Err(write_err);
        }

        // Step 2: Calculate the packet size
        let packet_byte_size = temp_buffer.len() as u16;

        // Step 3: Write the full packet
        if let Err(write_err) =
            self.packet_description
                .write_options(writer, endian, (packet_byte_size,))
        {
            return Err(write_err);
        }

        if let Err(write_err) = writer.write(&temp_buffer) {
            return Err(binrw::Error::Io(write_err));
        }

        Ok(())
    }
}

// ^^^^^^^^ End of RequestObjectAssembly impl ^^^^^^^^

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseObjectAssembly<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    pub packet_description: EnIpPacketDescription,

    // TODO: Validate that the size of the EnIpPacketDescription correctly matches the remaining bytes
    //  * If the remaining bytes are 0, don't serialize the next step (otherwise do)
    #[br(try)]
    pub cip_message: Option<MessageRouterResponse<T>>,
}

impl RequestObjectAssembly<u8> {
    pub fn new_registration() -> Self {
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_registration_description(),
            cip_message: None,
        }
    }

    pub fn new_unregistration(session_handle: CipUdint) -> Self {
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_unregistration_description(
                session_handle,
            ),
            cip_message: None,
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

impl<T> RequestObjectAssembly<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn new_single_request(
        session_handle: CipUdint,
        request_path: CipPath,
        data: Option<T>,
    ) -> Self {
        Self::new_service_request(
            session_handle,
            request_path,
            ServiceCode::GetAttributeSingle,
            data,
        )
    }

    pub fn new_service_request(
        session_handle: CipUdint,
        request_path: CipPath,
        service_code: ServiceCode,
        data: Option<T>,
    ) -> Self {
        Self {
            packet_description: EnIpPacketDescription::new_cip_description(session_handle, 0),
            cip_message: Some(MessageRouterRequest::new_data(
                service_code,
                request_path,
                data,
            )),
        }
    }
}

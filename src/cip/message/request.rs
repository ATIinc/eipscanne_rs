use std::mem;

use binrw::meta::WriteEndian;
use binrw::{
    binwrite,
    BinWrite, // BinWrite, // trait for writing
};

use super::shared::{ServiceCode, ServiceContainer};
use crate::cip::path::CipPath;
use crate::cip::types::CipUsint;

#[derive(Debug, PartialEq)]
pub struct RequestData<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub total_word_size: CipUsint,
    pub cip_path: CipPath,
    pub additional_data: Option<T>,
}

// ======= Start of RequestData impl ========

impl<T> RequestData<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn new(path: CipPath, request_data_content: Option<T>) -> Self {
        RequestData {
            total_word_size: 0,
            cip_path: path,
            additional_data: request_data_content,
        }
    }
}

// TODO: Figure out how to create a macro for Writing a later variable first
impl<T> WriteEndian for RequestData<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Little);
}

impl<T> BinWrite for RequestData<T>
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
        // Step 1: Serialize the `cip_path` field
        let mut temp_buffer = Vec::new();
        let mut temp_writer = std::io::Cursor::new(&mut temp_buffer);

        let cip_path_write_result = self.cip_path.write_options(&mut temp_writer, endian, args);

        if let Err(write_err) = cip_path_write_result {
            return Err(write_err);
        }

        // Step 2: Calculate the total packet size
        let header_byte_size = mem::size_of_val(&self.total_word_size);
        let cip_path_size = temp_buffer.len();

        let total_packet_word_size = (header_byte_size + cip_path_size) / mem::size_of::<u16>();

        let total_packet_word_size_array = [total_packet_word_size as CipUsint];

        // Step 3: Write the full struct to the actual writer
        if let Err(write_err) = writer.write(&total_packet_word_size_array) {
            return Err(binrw::Error::Io(write_err));
        }

        // Step 4: Write the `cip_path` field to the actual writer
        if let Err(write_err) = writer.write(&temp_buffer) {
            return Err(binrw::Error::Io(write_err));
        }

        // Step 5: Write the `additional_data` field
        if let Some(data_ref) = &self.additional_data {
            let data_write_result = data_ref.write_options(writer, endian, args);

            if let Err(write_err) = data_write_result {
                return Err(write_err);
            }
        }

        Ok(())
    }
}

// ^^^^^^^^ End of RequestData impl ^^^^^^^^

#[binwrite]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MessageRouterRequest<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub service_container: ServiceContainer,
    pub request_data: RequestData<T>,
}

// ======= Start of MessageRouterRequest impl ========

impl MessageRouterRequest<u8> {
    pub fn new(service_code: ServiceCode, path: CipPath) -> Self {
        Self::new_data(service_code, path, None)
    }
}

impl<T> MessageRouterRequest<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn new_data(
        service_code: ServiceCode,
        path: CipPath,
        request_data_content: Option<T>,
    ) -> Self {
        MessageRouterRequest {
            service_container: ServiceContainer::new(service_code, false),
            request_data: RequestData::new(path, request_data_content),
        }
    }
}

// ^^^^^^^^ End of MessageRouterRequest impl ^^^^^^^^

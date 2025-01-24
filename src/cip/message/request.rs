use std::io::SeekFrom;
use std::mem;

use binrw::{
    binwrite,
    BinResult,
    BinWrite, // BinWrite, // trait for writing
};

use super::shared::{ServiceCode, ServiceContainer};
use crate::cip::path::CipPath;
use crate::cip::types::CipUsint;

#[binrw::writer(writer, endian)]
fn write_cip_path_with_size(cip_path: &CipPath) -> BinResult<()> {
    // Step 1: Write the `cip_path` field
    let mut temp_buffer = Vec::new();
    let mut temp_writer = std::io::Cursor::new(&mut temp_buffer);

    cip_path.write_options(&mut temp_writer, endian, ())?;

    // Step 2: Calculate the `cip_path` byte size
    let cip_path_word_size = (temp_buffer.len()) / mem::size_of::<u16>();

    // Step 3: Write the full struct
    if let Err(write_err) = writer.write(&[cip_path_word_size as CipUsint]) {
        return Err(binrw::Error::Io(write_err));
    }

    if let Err(write_err) = writer.write(&temp_buffer) {
        return Err(binrw::Error::Io(write_err));
    }

    Ok(())
}

#[binwrite]
#[derive(Debug, PartialEq)]
#[bw(little)]
pub struct RequestData<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    pub total_word_size: CipUsint,
    // override the total_word_size by seeking back before it
    #[bw(seek_before = SeekFrom::Current(-1 * (mem::size_of::<CipUsint>() as i64)), write_with = write_cip_path_with_size)]
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

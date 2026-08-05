use std::io::{SeekFrom};
use std::mem;

use binrw::{binrw, BinResult, BinWrite};

use super::shared::{ServiceCode, ServiceContainer, SIZE_OF_SERVICE_CONTAINER};
use crate::cip::message::data::{CipData, CipDataOpt};
use crate::cip::message::shared::{BYTES_IN_A_WORD, SIZE_OF_CIP_USINT};
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

#[binrw]
#[derive(Debug, PartialEq)]
#[brw(little)]
#[br(import(additional_data_length: u16))]
pub struct RequestData {
    pub total_word_size: CipUsint,
    // override the total_word_size by seeking back before it
    #[bw(seek_before = SeekFrom::Current(-1 * (mem::size_of::<CipUsint>() as i64)), write_with = write_cip_path_with_size)]
    #[br(args(total_word_size))]
    pub cip_path: CipPath,

    // Subtract the number of bytes taken up by the `total_word_size` field and the `cip_path``
    #[br(args(additional_data_length - (SIZE_OF_CIP_USINT as u16) - (BYTES_IN_A_WORD * total_word_size as u16)))]
    pub additional_data: CipDataOpt,
}

// ======= Start of RequestData impl ========

impl RequestData {
    pub fn new(
        word_size: Option<CipUsint>,
        path: CipPath,
        request_data_content: Option<Box<dyn CipData>>,
    ) -> Self {
        RequestData {
            total_word_size: word_size.unwrap_or(0),
            cip_path: path,
            additional_data: match request_data_content {
                None => CipDataOpt::Raw(vec![]),
                Some(content) => CipDataOpt::Typed(content),
            },
        }
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(additional_data_length: u16))]
pub struct MessageRouterRequest {
    pub service_container: ServiceContainer,

    // Subtract number of bytes in the service container
    #[br(args(additional_data_length - SIZE_OF_SERVICE_CONTAINER as u16))]
    pub request_data: RequestData,
}

// ======= Start of MessageRouterRequest impl ========

impl MessageRouterRequest {
    pub fn new(service_code: ServiceCode, path: CipPath) -> Self {
        Self::new_data(service_code, path, None)
    }

    pub fn new_data(
        service_code: ServiceCode,
        path: CipPath,
        request_data_content: Option<Box<dyn CipData>>,
    ) -> Self {
        MessageRouterRequest {
            service_container: ServiceContainer::new(service_code, false),
            request_data: RequestData::new(None, path, request_data_content),
        }
    }
}

// ^^^^^^^^ End of MessageRouterRequest impl ^^^^^^^^

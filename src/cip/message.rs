use std::mem;

use binrw::meta::WriteEndian;
use binrw::{
    binread,
    binrw, // #[binrw] attribute
    binwrite,
    BinRead,  // BinRead,  // trait for reading
    BinWrite, // BinWrite, // trait for writing
};

use bilge::prelude::{bitsize, u7, Bitsized, DebugBits, FromBits, Number};

use super::path::CipPath;
use super::types::CipUsint;

#[bitsize(7)]
#[derive(FromBits, PartialEq, Debug)]
#[repr(u8)]
pub enum ServiceCode {
    None = 0x00,
    /* Start CIP common services */
    GetAttributeAll = 0x01,
    SetAttributeAll = 0x02,
    GetAttributeList = 0x03,
    SetAttributeList = 0x04,
    Reset = 0x05,
    Start = 0x06,
    Stop = 0x07,
    CreateObjectInstance = 0x08,
    DeleteObjectInstance = 0x09,
    MultipleServicePacket = 0x0A,
    ApplyAttributes = 0x0D,
    GetAttributeSingle = 0x0E,
    SetAttributeSingle = 0x10,
    FindNextObjectInstance = 0x11,
    ErrorResponse = 0x14, //DeviceNet only
    Restore = 0x15,
    Save = 0x16,
    GetMember = 0x18,
    NoOperation = 0x17,
    SetMember = 0x19,
    InsertMember = 0x1A,
    RemoveMember = 0x1B,
    GroupSync = 0x1C, /* End CIP common services */

    #[fallback]
    Unknown(u7),
}

#[bitsize(8)]
#[derive(FromBits, PartialEq, DebugBits, Clone, Copy)]
pub struct ServiceContainerBits {
    service: ServiceCode,
    response: bool,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ServiceContainer {
    service_representation: u8,
}

// ======= Start of ServiceContainer impl ========

impl From<ServiceContainer> for ServiceContainerBits {
    fn from(container: ServiceContainer) -> Self {
        ServiceContainerBits::from(container.service_representation)
    }
}

impl From<ServiceContainerBits> for ServiceContainer {
    fn from(container: ServiceContainerBits) -> Self {
        ServiceContainer {
            service_representation: container.value,
        }
    }
}

// ^^^^^^^^ End of ServiceContainer impl ^^^^^^^^

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

        // Step 2: Serialize the `additional_data` field
        if let Some(data_ref) = &self.additional_data {
            let data_write_result = data_ref.write_options(&mut temp_writer, endian, args);

            if let Err(write_err) = data_write_result {
                return Err(write_err);
            }
        }

        // Step 3: Calculate the total packet size
        let header_byte_size = mem::size_of_val(&self.total_word_size);
        let data_byte_size = temp_buffer.len();

        let total_packet_word_size = (header_byte_size + data_byte_size) / mem::size_of::<u16>();

        let total_packet_word_size_array = [total_packet_word_size as CipUsint];

        // Write the full struct to the actual writer
        if let Err(write_err) = writer.write(&total_packet_word_size_array) {
            return Err(binrw::Error::Io(write_err));
        }

        if let Err(write_err) = writer.write(&temp_buffer) {
            return Err(binrw::Error::Io(write_err));
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
            service_container: ServiceContainer::from(ServiceContainerBits::new(
                service_code,
                false,
            )),
            request_data: RequestData::new(path, request_data_content),
        }
    }
}

// ^^^^^^^^ End of MessageRouterRequest impl ^^^^^^^^

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseData<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    #[br(pad_before = 1)]
    pub status: u8,
    pub additional_status_size: u8,
    pub data: T,
}

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MessageRouterResponse<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    #[br(assert(ServiceContainerBits::from(service_container).response()))]
    pub service_container: ServiceContainer,
    pub router_data: ResponseData<T>,
}

// NOTE:
//  - Keeping a generic MessageRouter struct here for future reference
//  - It is cleaner to minimize duplicated code but having the Request and Response split up makes the interface simpler

// #[binrw]
// #[brw(little)]
// #[derive(Debug, PartialEq)]
// #[br(import(serviceContainer: ServiceContainerBits))]
// pub enum RouterData<T>
// where
//     T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
// {
//     #[br(pre_assert(!serviceContainer.response()))]
//     Request(RequestData<T>),

//     #[br(pre_assert(serviceContainer.response()))]
//     Response(ResponseData<T>),
// }

// #[binrw]
// #[brw(little)]
// #[derive(Debug, PartialEq)]
// pub struct MessageRouter<T>
// where
//     T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
// {
//     pub service_container: ServiceContainer,

//     // Only include this if the service code is NOT a response
//     #[br(args(ServiceContainerBits::from(service_container),))]
//     pub router_data: RouterData<T>,
// }

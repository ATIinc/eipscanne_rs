use std::mem;

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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RequestData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub data_word_size: CipUsint,
    pub cip_path: CipPath,
    pub data: Option<T>,
}

// TODO: Turn this into a macro
impl RequestData<u8> {
    pub fn new(path: CipPath) -> Self {
        RequestData::new_data(path, None)
    }
}

impl<T> RequestData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    fn new_data(path: CipPath, request_data_content: Option<T>) -> Self {
        let path_size = path.byte_size();

        let content_size = match &request_data_content {
            Some(content) => std::mem::size_of_val(&content),
            None => 0,
        };

        let content_word_size = (path_size + content_size) / mem::size_of::<u16>();

        RequestData {
            data_word_size: content_word_size as u8,
            cip_path: path,
            data: request_data_content,
        }
    }

    fn byte_size(&self) -> usize {
        let known_byte_size = mem::size_of_val(&self.data_word_size) + self.cip_path.byte_size();

        let variable_byte_size = match &self.data {
            Some(data) => std::mem::size_of_val(data),
            None => 0,
        };

        known_byte_size + variable_byte_size
    }
}

#[binwrite]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MessageRouterRequest<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
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
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn byte_size(&self) -> usize {
        // // Creating a manual function because std::mem::size_of_val not playing nice
        let service_container_size = mem::size_of_val(&self.service_container);

        service_container_size + self.request_data.byte_size()
    }

    pub fn new_data(
        service_code: ServiceCode,
        path: CipPath,
        request_data_content: Option<T>,
    ) -> Self {
        MessageRouterRequest {
            service_container: ServiceContainerBits::new(service_code, false).into(),
            request_data: RequestData::new_data(path, request_data_content),
        }
    }
}

// ^^^^^^^^ End of MessageRouterRequest impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
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
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
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

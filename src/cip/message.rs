use std::mem;

use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // BinRead,  // trait for reading
    BinWrite, // BinWrite, // trait for writing
};

use bilge::prelude::{bitsize, u7, Bitsized, DebugBits, Number, TryFromBits};

use super::types::CipSint;

#[bitsize(7)]
#[derive(TryFromBits, PartialEq, Debug)]
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
}

#[bitsize(8)]
#[derive(TryFromBits, PartialEq, DebugBits, Clone, Copy)]
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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RequestData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub data_word_size: CipSint,
    pub data: T,
}

impl<T> RequestData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    fn byte_size(&self) -> usize
    {
        mem::size_of_val(&self.data_word_size) + mem::size_of_val(&self.data)
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub _unused: u8,
    pub status: u8,
    pub additional_status_size: u8,
    pub data: T,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(serviceContainer: ServiceContainerBits))]
pub enum RouterData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    #[br(pre_assert(!serviceContainer.response()))]
    Request(RequestData<T>),

    #[br(pre_assert(serviceContainer.response()))]
    Response(ResponseData<T>),
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MessageRouter<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub service_container: ServiceContainer,

    // Only include this if the service code is NOT a response
    #[br(args(ServiceContainerBits::from(service_container),))]
    pub router_data: RouterData<T>,
}

impl From<ServiceContainer> for ServiceContainerBits {
    fn from(container: ServiceContainer) -> Self {
        ServiceContainerBits::try_from(container.service_representation).unwrap()
    }
}

impl From<ServiceContainerBits> for ServiceContainer {
    fn from(container: ServiceContainerBits) -> Self {
        ServiceContainer {
            service_representation: container.value,
        }
    }
}

impl<T> MessageRouter<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn byte_size(&self) -> usize {
        // Creating a manual function because std::mem::size_of_val not playing nice
        let service_container_size = mem::size_of_val(&self.service_container);

        // TODO: Actually get the byte_size of the Request rather than adding a value on top
        let data_size = match &self.router_data {
            RouterData::Request(request_data) => request_data.byte_size(),
            RouterData::Response(response_data) => mem::size_of_val(&response_data),
        };

        service_container_size + data_size
    }
}

impl<T> MessageRouter<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn new_request(service_code: ServiceCode, request_data_content: T) -> MessageRouter<T> {
        // temporarily create the request_data to calculate the byte_size
        let mut request_data = RequestData {
            data_word_size: 0,
            data: request_data_content,
        };

        let total_data_size = request_data.byte_size();
        let total_data_word_size = total_data_size / mem::size_of::<u16>();

        request_data.data_word_size = total_data_word_size.try_into().unwrap();

        MessageRouter {
            service_container: ServiceContainerBits::new(service_code, false).into(),
            router_data: RouterData::Request(request_data),
        }
    }
}

use std::mem;

use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // BinRead,  // trait for reading
    BinWrite, // BinWrite, // trait for writing
};

use bilge::prelude::{bitsize, u7, Bitsized, DebugBits, Number, TryFromBits};

use crate::eip::packet::EncapsStatusCode;

use super::types::CipUint;

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
    // NOTE: This bit is at the front of the byte in testing
    response: bool,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ServiceContainer {
    service_representation: u8,
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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RequestData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub data_word_size: CipUint,
    pub data: T,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseData<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub status: EncapsStatusCode,
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

impl<T> MessageRouter<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn byte_size(&self) -> CipUint {
        // Creating a manual function because std::mem::size_of_val not playing nice
        let service_container_size = mem::size_of_val(&self.service_container);

        let data_size = match &self.router_data {
            RouterData::Request(request_data) => mem::size_of_val(&request_data),
            RouterData::Response(response_data) => mem::size_of_val(&response_data),
        };

        (service_container_size + data_size) as CipUint
    }
}

impl<T> MessageRouter<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub fn new_request(service_code: ServiceCode, cip_data: T) -> MessageRouter<T> {
        let total_data_size = mem::size_of_val(&cip_data);
        let total_data_word_size = total_data_size / mem::size_of::<CipUint>();

        MessageRouter {
            service_container: ServiceContainerBits::new(service_code, false).into(),
            router_data: RouterData::Request(RequestData {
                data_word_size: total_data_word_size.try_into().unwrap(),
                data: cip_data,
            }),
        }
    }
}

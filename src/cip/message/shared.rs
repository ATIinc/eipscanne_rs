use binrw::{
    binrw, // #[binrw] attribute
};

use bilge::prelude::{bitsize, u7, Bitsized, DebugBits, FromBits, Number};

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
    pub service: ServiceCode,
    pub response: bool,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ServiceContainer {
    pub service_representation: u8,
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

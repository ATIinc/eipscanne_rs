use std::mem;

use binrw::{BinRead, BinWrite};

use bilge::prelude::{bitsize, u7, BuilderBits, DebugBits, DefaultBits, FromBits};

use crate::cip::types::CipUsint;

pub const BYTES_IN_A_WORD: u16 = 2;
pub const SIZE_OF_CIP_USINT: usize = mem::size_of::<CipUsint>();
pub const SIZE_OF_SERVICE_CONTAINER: usize = mem::size_of::<ServiceContainer>();

#[bitsize(7)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum ServiceCode {
    #[default]
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

    /* Start Connection Manager object specific services */
    ForwardClose = 0x4E,
    UnconnectedSend = 0x52,
    ForwardOpen = 0x54,
    GetConnectionData = 0x56,
    SearchConnectionData = 0x57,
    GetConnectionOwner = 0x5A,
    LargeForwardOpen = 0x5B, /* End Connection Manager object specific services */

    #[fallback]
    Unknown(u7),
}

#[bitsize(8)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u8::into)]
#[bw(map = |&x| u8::from(x))]
pub struct ServiceContainer {
    pub service: ServiceCode,
    pub response: bool,
}

// ======= Start of ServiceContainer impl ========

impl ServiceContainer {
    /// The service byte of a request
    pub fn new_request(service: ServiceCode) -> Self {
        ServiceContainer::builder()
            .service(service)
            .response(false)
            .build()
    }

    /// The service byte of a response (request bit set)
    pub fn new_response(service: ServiceCode) -> Self {
        ServiceContainer::builder()
            .service(service)
            .response(true)
            .build()
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

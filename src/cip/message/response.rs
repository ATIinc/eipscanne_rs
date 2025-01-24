use binrw::{
    binread,
    BinRead, // BinRead,  // trait for reading
};

use crate::cip::types::CipUsint;

use super::shared::ServiceContainer;

#[derive(BinRead)]
#[br(little, repr = CipUsint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ResponseStatusCode {
    Success = 0x0000,
}

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseData<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    #[br(pad_before = 1)]
    pub status: ResponseStatusCode,
    pub additional_status_size: u8,

    #[br(try)]
    pub data: Option<T>,
}

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MessageRouterResponse<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    #[br(assert(service_container.response()))]
    pub service_container: ServiceContainer,
    pub response_data: ResponseData<T>,
}

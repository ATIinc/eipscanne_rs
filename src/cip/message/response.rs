use binrw::{
    binread,
    BinRead, // BinRead,  // trait for reading
};

use super::shared::{ServiceContainer, ServiceContainerBits};

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

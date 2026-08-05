use binrw::{
    binrw,
    BinRead,
    BinWrite, // BinRead,  // trait for reading
};

use crate::cip::{message::{data::CipDataOpt, shared::SIZE_OF_CIP_USINT}, types::CipUsint};

use super::shared::{ServiceContainer, SIZE_OF_SERVICE_CONTAINER};

#[derive(BinRead, BinWrite)]
#[brw(little, repr = CipUsint)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ResponseStatusCode {
    Success = 0x0000,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(data_length: u16))]
pub struct ResponseData
{
    #[brw(pad_before = 1)]
    pub status: ResponseStatusCode,
    pub additional_status_size: CipUsint,

    // Subtract the `pad_before` byte and the size of `status` and `additional_status_size`
    #[br(args(data_length - (SIZE_OF_CIP_USINT * 3) as u16))]
    pub data: CipDataOpt,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(data_length: u16))]
pub struct MessageRouterResponse
{
    #[br(assert(service_container.response()))]
    pub service_container: ServiceContainer,

    #[br(args(data_length - SIZE_OF_SERVICE_CONTAINER as u16))]
    pub response_data: ResponseData,
}

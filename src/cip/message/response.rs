use binrw::binrw;

use crate::cip::{
    message::{
        data::CipDataOpt,
        shared::{BYTES_IN_A_WORD, SIZE_OF_CIP_USINT},
    },
    types::{CipUint, CipUsint},
};

use super::shared::{SIZE_OF_SERVICE_CONTAINER, ServiceContainer};

/// General Status of a Message Router response.
///
/// Unknown values are kept as-is so a response from a device that uses a reserved or
/// object-specific code can still be parsed and inspected.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ResponseStatusCode {
    #[brw(magic = 0x00u8)]
    Success,
    #[brw(magic = 0x01u8)]
    ConnectionFailure,
    #[brw(magic = 0x02u8)]
    ResourceUnavailable,
    #[brw(magic = 0x03u8)]
    InvalidParameterValue,
    #[brw(magic = 0x04u8)]
    PathSegmentError,
    #[brw(magic = 0x05u8)]
    PathDestinationUnknown,
    #[brw(magic = 0x06u8)]
    PartialTransfer,
    #[brw(magic = 0x07u8)]
    ConnectionLost,
    #[brw(magic = 0x08u8)]
    ServiceNotSupported,
    #[brw(magic = 0x09u8)]
    InvalidAttributeValue,
    #[brw(magic = 0x0Au8)]
    AttributeListError,
    #[brw(magic = 0x0Bu8)]
    AlreadyInRequestedMode,
    #[brw(magic = 0x0Cu8)]
    ObjectStateConflict,
    #[brw(magic = 0x0Du8)]
    ObjectAlreadyExists,
    #[brw(magic = 0x0Eu8)]
    AttributeNotSettable,
    #[brw(magic = 0x0Fu8)]
    PrivilegeViolation,
    #[brw(magic = 0x10u8)]
    DeviceStateConflict,
    #[brw(magic = 0x11u8)]
    ReplyDataTooLarge,
    #[brw(magic = 0x12u8)]
    FragmentationOfPrimitiveValue,
    #[brw(magic = 0x13u8)]
    NotEnoughData,
    #[brw(magic = 0x14u8)]
    AttributeNotSupported,
    #[brw(magic = 0x15u8)]
    TooMuchData,
    #[brw(magic = 0x16u8)]
    ObjectDoesNotExist,
    #[brw(magic = 0x17u8)]
    ServiceFragmentationSequenceNotInProgress,
    #[brw(magic = 0x18u8)]
    NoStoredAttributeData,
    #[brw(magic = 0x19u8)]
    StoreOperationFailure,
    #[brw(magic = 0x1Au8)]
    RoutingFailureRequestPacketTooLarge,
    #[brw(magic = 0x1Bu8)]
    RoutingFailureResponsePacketTooLarge,
    #[brw(magic = 0x1Cu8)]
    MissingAttributeListEntryData,
    #[brw(magic = 0x1Du8)]
    InvalidAttributeValueList,
    #[brw(magic = 0x1Eu8)]
    EmbeddedServiceError,
    #[brw(magic = 0x1Fu8)]
    VendorSpecificError,
    #[brw(magic = 0x20u8)]
    InvalidParameter,
    #[brw(magic = 0x21u8)]
    WriteOnceValueOrMediumAlreadyWritten,
    #[brw(magic = 0x22u8)]
    InvalidReplyReceived,
    #[brw(magic = 0x23u8)]
    BufferOverflow,
    #[brw(magic = 0x24u8)]
    MessageFormatError,
    #[brw(magic = 0x25u8)]
    KeyFailureInPath,
    #[brw(magic = 0x26u8)]
    PathSizeInvalid,
    #[brw(magic = 0x27u8)]
    UnexpectedAttributeInList,
    #[brw(magic = 0x28u8)]
    InvalidMemberId,
    #[brw(magic = 0x29u8)]
    MemberNotSettable,
    #[brw(magic = 0x2Au8)]
    Group2OnlyServerGeneralFailure,
    #[brw(magic = 0x2Bu8)]
    UnknownModbusError,
    Unknown(CipUsint),
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(data_length: u16))]
pub struct ResponseData {
    #[brw(pad_before = 1)]
    pub status: ResponseStatusCode,
    pub additional_status_size: CipUsint,

    // One 16-bit word per `additional_status_size` (Additional Status in Wireshark)
    #[br(count = additional_status_size)]
    pub additional_status: Vec<CipUint>,

    // Subtract the `pad_before` byte, the size of `status` and `additional_status_size`,
    // and the additional status words
    #[br(args(data_length - (SIZE_OF_CIP_USINT * 3) as u16 - BYTES_IN_A_WORD * (additional_status_size as u16)))]
    pub data: CipDataOpt,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(data_length: u16))]
pub struct MessageRouterResponse {
    #[br(assert(service_container.response()))]
    pub service_container: ServiceContainer,

    #[br(args(data_length - SIZE_OF_SERVICE_CONTAINER as u16))]
    pub response_data: ResponseData,
}

// ======= Start of MessageRouterResponse impl ========

impl MessageRouterResponse {
    pub fn is_success(&self) -> bool {
        self.response_data.status == ResponseStatusCode::Success
    }
}

// ^^^^^^^^ End of MessageRouterResponse impl ^^^^^^^^

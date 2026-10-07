//! Reply side of the Connection Manager services. Which reply data a Message Router response
//! carries is decided by its service; a refused request carries a `Rejection` instead.

use std::fmt;
use std::io::Cursor;

use binrw::{BinRead, BinResult, BinWrite, binrw};

use crate::cip::connection_manager::forward_close::ForwardCloseResponse;
use crate::cip::connection_manager::forward_open::ForwardOpenResponse;
use crate::cip::message::response::{MessageRouterResponse, Rejection, variant_words};
use crate::cip::message::shared::ServiceCode;
use crate::cip::types::CipUint;

/// Extended status of a Connection Manager reply (the first Additional Status word).
///
/// Unknown values are kept as-is.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ConnectionManagerExtendedStatus {
    #[brw(magic = 0x0100u16)]
    ConnectionInUseOrDuplicateForwardOpen,
    #[brw(magic = 0x0103u16)]
    TransportClassAndTriggerCombinationNotSupported,
    #[brw(magic = 0x0106u16)]
    OwnershipConflict,
    #[brw(magic = 0x0107u16)]
    TargetConnectionNotFound,
    #[brw(magic = 0x0108u16)]
    InvalidNetworkConnectionParameter,
    #[brw(magic = 0x0109u16)]
    InvalidConnectionSize,
    #[brw(magic = 0x0110u16)]
    TargetForConnectionNotConfigured,
    #[brw(magic = 0x0111u16)]
    RequestedPacketIntervalNotSupported,
    #[brw(magic = 0x0113u16)]
    OutOfConnections,
    #[brw(magic = 0x0114u16)]
    VendorIdOrProductCodeMismatch,
    #[brw(magic = 0x0115u16)]
    ProductTypeMismatch,
    #[brw(magic = 0x0116u16)]
    RevisionMismatch,
    #[brw(magic = 0x0117u16)]
    InvalidProducedOrConsumedApplicationPath,
    #[brw(magic = 0x0118u16)]
    InvalidOrInconsistentConfigurationApplicationPath,
    #[brw(magic = 0x0119u16)]
    NonListenOnlyConnectionNotOpened,
    #[brw(magic = 0x011Au16)]
    TargetObjectOutOfConnections,
    #[brw(magic = 0x011Bu16)]
    RequestedPacketIntervalSmallerThanProductionInhibitTime,
    #[brw(magic = 0x0203u16)]
    ConnectionTimedOut,
    #[brw(magic = 0x0204u16)]
    UnconnectedRequestTimedOut,
    #[brw(magic = 0x0205u16)]
    ParameterErrorInUnconnectedRequestService,
    #[brw(magic = 0x0206u16)]
    MessageTooLargeForUnconnectedSendService,
    #[brw(magic = 0x0207u16)]
    UnconnectedAcknowledgeWithoutReply,
    #[brw(magic = 0x0301u16)]
    NoBufferMemoryAvailable,
    #[brw(magic = 0x0302u16)]
    NetworkBandwidthNotAvailableForData,
    #[brw(magic = 0x0303u16)]
    NoConsumedConnectionIdFilterAvailable,
    #[brw(magic = 0x0304u16)]
    NotConfiguredToSendScheduledPriorityData,
    #[brw(magic = 0x0305u16)]
    ScheduleSignatureMismatch,
    #[brw(magic = 0x0306u16)]
    ScheduleSignatureValidationNotPossible,
    #[brw(magic = 0x0311u16)]
    PortNotAvailable,
    #[brw(magic = 0x0312u16)]
    LinkAddressNotValid,
    #[brw(magic = 0x0315u16)]
    InvalidSegmentInConnectionPath,
    #[brw(magic = 0x0316u16)]
    ErrorInForwardCloseServiceConnectionPath,
    #[brw(magic = 0x0317u16)]
    SchedulingNotSpecified,
    #[brw(magic = 0x0318u16)]
    LinkAddressToSelfInvalid,
    #[brw(magic = 0x0319u16)]
    SecondaryResourcesUnavailable,
    #[brw(magic = 0x031Au16)]
    RackConnectionAlreadyEstablished,
    #[brw(magic = 0x031Bu16)]
    ModuleConnectionAlreadyEstablished,
    #[brw(magic = 0x031Cu16)]
    Miscellaneous,
    #[brw(magic = 0x031Du16)]
    RedundantConnectionMismatch,
    #[brw(magic = 0x031Eu16)]
    NoMoreUserConfigurableLinkConsumerResourcesAvailableInTheProducingModule,
    #[brw(magic = 0x031Fu16)]
    NoUserConfigurableLinkConsumerResourcesConfiguredInTheProducingModule,
    #[brw(magic = 0x0800u16)]
    NetworkLinkOffline,
    #[brw(magic = 0x0810u16)]
    NoTargetApplicationDataAvailable,
    #[brw(magic = 0x0811u16)]
    NoOriginatorApplicationDataAvailable,
    #[brw(magic = 0x0812u16)]
    NodeAddressHasChangedSinceTheNetworkWasScheduled,
    #[brw(magic = 0x0813u16)]
    NotConfiguredForOffSubnetMulticast,
    #[brw(magic = 0x0814u16)]
    InvalidProduceConsumeDataFormat,
    Unknown(CipUint),
}

/// Reply data of a successful Connection Manager service, chosen by the service of the Message
/// Router response that carries it
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(service: ServiceCode))]
pub enum ConnectionManagerResponse {
    #[br(pre_assert(matches!(service, ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen)))]
    ForwardOpen(ForwardOpenResponse),

    #[br(pre_assert(service == ServiceCode::ForwardClose))]
    ForwardClose(ForwardCloseResponse),
}

// ======= Start of ConnectionManagerExtendedStatus impl ========

/// The extended status in words with its code: `connection in use or duplicate forward open
/// (0x0100)`
impl fmt::Display for ConnectionManagerExtendedStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut code = Cursor::new(Vec::new());
        // Every variant, `Unknown` included, writes its 16-bit code
        self.write_le(&mut code).map_err(|_| fmt::Error)?;
        let code = u16::from_le_bytes([code.get_ref()[0], code.get_ref()[1]]);
        match self {
            ConnectionManagerExtendedStatus::Unknown(_) => {
                write!(f, "unknown extended status ({code:#06x})")
            }
            _ => write!(f, "{} ({code:#06x})", variant_words(&format!("{self:?}"))),
        }
    }
}

// ^^^^^^^^ End of ConnectionManagerExtendedStatus impl ^^^^^^^^

// ======= Start of Rejection impl ========

impl Rejection {
    /// Why the Connection Manager refused a Forward_Open, Large_Forward_Open or Forward_Close: the
    /// first Additional Status word. `None` for any other service, whose Additional Status means
    /// something else, and when the reply carries no Additional Status.
    pub fn extended_status(&self) -> Option<ConnectionManagerExtendedStatus> {
        if !is_connection_manager_service(self.service) {
            return None;
        }
        let word = self.additional_status.first()?;
        // Reading a 16-bit word cannot fail: every value without a variant of its own is `Unknown`
        ConnectionManagerExtendedStatus::read_le(&mut Cursor::new(word.to_le_bytes())).ok()
    }
}

// ^^^^^^^^ End of Rejection impl ^^^^^^^^

// ======= Start of ConnectionManagerResponse impl ========

impl ConnectionManagerResponse {
    /// Interprets the reply to a successful Forward_Open, Large_Forward_Open or Forward_Close. A
    /// refused request carries no reply data to interpret, so callers check
    /// `Rejection::from_response` first; a refused reply, or the reply to any other service, is
    /// an error here.
    pub fn from_message_router_response(
        response: &MessageRouterResponse,
    ) -> BinResult<ConnectionManagerResponse> {
        let service = response.service_container.service();
        if !is_connection_manager_service(service) {
            return Err(binrw::Error::AssertFail {
                pos: 0,
                message: format!(
                    "not a Forward_Open, Large_Forward_Open or Forward_Close reply: {service:?}"
                ),
            });
        }
        if let Some(rejection) = Rejection::from_response(response) {
            return Err(binrw::Error::AssertFail {
                pos: 0,
                message: rejection.to_string(),
            });
        }

        // The reply data as bytes, whether the response holds them raw or typed; the write side
        // of `CipDataOpt` ignores its length argument
        let mut data = Cursor::new(Vec::new());
        response.response_data.data.write_le_args(&mut data, (0,))?;
        data.set_position(0);

        ConnectionManagerResponse::read_le_args(&mut data, (service,))
    }
}

// ^^^^^^^^ End of ConnectionManagerResponse impl ^^^^^^^^

fn is_connection_manager_service(service: ServiceCode) -> bool {
    matches!(
        service,
        ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen | ServiceCode::ForwardClose
    )
}

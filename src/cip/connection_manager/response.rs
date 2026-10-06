//! Reply side of the Connection Manager services. Which reply data a Message Router response
//! carries is decided by its service and general status.

use std::io::Cursor;

use binrw::{BinRead, binrw};

use crate::cip::connection_manager::forward_close::ForwardCloseResponse;
use crate::cip::connection_manager::forward_open::ForwardOpenResponse;
use crate::cip::connection_manager::shared::UnsuccessfulResponse;
use crate::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
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
    RpiNotSupported,
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
    RpiSmallerThanProductionInhibitTime,
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

/// Reply data of a Connection Manager service, chosen by the service and general status of the
/// Message Router response that carries it
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(service: ServiceCode, status: ResponseStatusCode))]
pub enum ConnectionManagerResponse {
    #[br(pre_assert(
        status == ResponseStatusCode::Success
            && matches!(service, ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen)
    ))]
    ForwardOpen(ForwardOpenResponse),

    #[br(pre_assert(status == ResponseStatusCode::Success && service == ServiceCode::ForwardClose))]
    ForwardClose(ForwardCloseResponse),

    #[br(pre_assert(status != ResponseStatusCode::Success))]
    Unsuccessful(UnsuccessfulResponse),
}

/// Why the target rejected a Connection Manager request
#[derive(Debug, PartialEq, Clone)]
pub struct ConnectionManagerFailure {
    pub general_status: ResponseStatusCode,
    /// The first Additional Status word, if the reply carried one
    pub extended_status: Option<ConnectionManagerExtendedStatus>,
    pub additional_status: Vec<CipUint>,
    pub response: UnsuccessfulResponse,
}

#[derive(Debug)]
pub enum ConnectionManagerError {
    /// The target answered with a general status other than success
    Rejected(ConnectionManagerFailure),
    /// The reply is not a Connection Manager reply, or its data could not be parsed
    Malformed(binrw::Error),
}

impl From<binrw::Error> for ConnectionManagerError {
    fn from(error: binrw::Error) -> Self {
        ConnectionManagerError::Malformed(error)
    }
}

// ======= Start of ConnectionManagerResponse impl ========

impl ConnectionManagerResponse {
    /// Interprets the reply to a Forward_Open, Large_Forward_Open or Forward_Close
    pub fn from_message_router_response(
        response: &MessageRouterResponse,
    ) -> Result<ConnectionManagerResponse, ConnectionManagerError> {
        let service = response.service_container.service();
        if !matches!(
            service,
            ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen | ServiceCode::ForwardClose
        ) {
            return Err(ConnectionManagerError::Malformed(
                binrw::Error::AssertFail {
                    pos: 0,
                    message: format!("expected a Connection Manager reply, got {service:?}"),
                },
            ));
        }

        let status = response.response_data.status;
        let data = response.response_data.data.to_bytes()?;
        let mut reader = Cursor::new(&data);

        match ConnectionManagerResponse::read_le_args(&mut reader, (service, status))? {
            ConnectionManagerResponse::Unsuccessful(unsuccessful_response) => {
                let extended_status = match response.response_data.additional_status.first() {
                    Some(word) => Some(ConnectionManagerExtendedStatus::read_le(
                        &mut Cursor::new(word.to_le_bytes()),
                    )?),
                    None => None,
                };

                Err(ConnectionManagerError::Rejected(ConnectionManagerFailure {
                    general_status: status,
                    extended_status,
                    additional_status: response.response_data.additional_status.clone(),
                    response: unsuccessful_response,
                }))
            }
            successful_response => Ok(successful_response),
        }
    }
}

// ^^^^^^^^ End of ConnectionManagerResponse impl ^^^^^^^^

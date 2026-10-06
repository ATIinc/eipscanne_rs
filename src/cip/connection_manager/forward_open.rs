//! Forward_Open and Large_Forward_Open: opening a connection through the Connection Manager.

use std::io::Cursor;

use binrw::{BinRead, binrw};

use crate::cip::connection_manager::parameters::{
    ConnectionDirection, ConnectionSizeError, ConnectionTimeoutMultiplier,
    NetworkConnectionParameters, PriorityTimeTick, TransportTypeTrigger,
};
use crate::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use crate::cip::message::shared::ServiceCode;
use crate::cip::path::CipPath;
use crate::cip::types::{CipUdint, CipUint, CipUsint};

/// Forward_Open / Large_Forward_Open request data (everything after the request path).
///
/// The width of the Network Connection Parameters decides the service: 16 bits for
/// Forward_Open, 32 bits for Large_Forward_Open. Reading needs to be told which one to expect.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(large: bool))]
pub struct ForwardOpenRequest {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    pub o2t_network_connection_id: CipUdint,
    pub t2o_network_connection_id: CipUdint,
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,

    // Followed by three reserved bytes
    #[brw(pad_after = 3)]
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,

    /// Requested packet interval, originator to target, in microseconds
    pub o2t_rpi: CipUdint,
    #[br(args(large))]
    pub o2t_network_connection_parameters: NetworkConnectionParameters,

    /// Requested packet interval, target to originator, in microseconds
    pub t2o_rpi: CipUdint,
    #[br(args(large))]
    pub t2o_network_connection_parameters: NetworkConnectionParameters,

    pub transport_type_trigger: TransportTypeTrigger,

    // Connection Path Size in 16-bit words, derived from the path on write
    #[br(temp)]
    #[bw(calc = connection_path.word_len() as CipUsint)]
    connection_path_size: CipUsint,

    #[br(args(connection_path_size))]
    pub connection_path: CipPath,
}

/// Everything needed to build a Forward_Open request
#[derive(Debug, PartialEq, Clone, Default)]
pub struct ConnectionParameters {
    pub priority_time_tick: PriorityTimeTick,
    pub timeout_ticks: CipUsint,
    /// Left at 0 for a point-to-point connection: the target chooses it and returns it in the reply
    pub o2t_network_connection_id: CipUdint,
    /// Chosen by the originator
    pub t2o_network_connection_id: CipUdint,
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,
    /// Requested packet interval, originator to target, in microseconds
    pub o2t_rpi: CipUdint,
    /// Requested packet interval, target to originator, in microseconds
    pub t2o_rpi: CipUdint,
    pub o2t: ConnectionDirection,
    pub t2o: ConnectionDirection,
    pub transport_type_trigger: TransportTypeTrigger,
    pub connection_path: CipPath,
    /// Send a Large_Forward_Open (32-bit connection parameters) instead of a Forward_Open
    pub large: bool,
}

// ======= Start of ForwardOpenRequest impl ========

impl ForwardOpenRequest {
    /// Builds the request data, computing the connection size of each direction
    pub fn new(
        parameters: &ConnectionParameters,
    ) -> Result<ForwardOpenRequest, ConnectionSizeError> {
        let transport_class = parameters.transport_type_trigger.transport_class();

        Ok(ForwardOpenRequest {
            priority_time_tick: parameters.priority_time_tick,
            timeout_ticks: parameters.timeout_ticks,
            o2t_network_connection_id: parameters.o2t_network_connection_id,
            t2o_network_connection_id: parameters.t2o_network_connection_id,
            connection_serial_number: parameters.connection_serial_number,
            originator_vendor_id: parameters.originator_vendor_id,
            originator_serial_number: parameters.originator_serial_number,
            connection_timeout_multiplier: parameters.connection_timeout_multiplier,
            o2t_rpi: parameters.o2t_rpi,
            o2t_network_connection_parameters: NetworkConnectionParameters::new(
                &parameters.o2t,
                transport_class,
                parameters.large,
            )?,
            t2o_rpi: parameters.t2o_rpi,
            t2o_network_connection_parameters: NetworkConnectionParameters::new(
                &parameters.t2o,
                transport_class,
                parameters.large,
            )?,
            transport_type_trigger: parameters.transport_type_trigger,
            connection_path: parameters.connection_path.clone(),
        })
    }

    /// The service this request is sent with, decided by the width of its connection parameters
    pub fn service_code(&self) -> ServiceCode {
        match self.o2t_network_connection_parameters {
            NetworkConnectionParameters::Standard(_) => ServiceCode::ForwardOpen,
            NetworkConnectionParameters::Large(_) => ServiceCode::LargeForwardOpen,
        }
    }
}

// ^^^^^^^^ End of ForwardOpenRequest impl ^^^^^^^^

/// Forward_Open / Large_Forward_Open reply data when the general status is success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardOpenResponse {
    pub o2t_network_connection_id: CipUdint,
    pub t2o_network_connection_id: CipUdint,
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,
    /// Actual packet interval, originator to target, in microseconds
    pub o2t_api: CipUdint,
    /// Actual packet interval, target to originator, in microseconds
    pub t2o_api: CipUdint,

    /// Application Reply Size in 16-bit words, followed by a reserved byte
    #[brw(pad_after = 1)]
    pub application_reply_size: CipUsint,

    #[br(count = usize::from(application_reply_size) * 2)]
    pub application_reply: Vec<CipUsint>,
}

/// Forward_Open / Large_Forward_Open reply data when the general status is not success
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardOpenUnsuccessfulResponse {
    pub connection_serial_number: CipUint,
    pub originator_vendor_id: CipUint,
    pub originator_serial_number: CipUdint,

    /// Remaining Path Size in 16-bit words and its reserved byte; only present when a routing
    /// node rejected the request
    #[br(try)]
    pub remaining_path_size: Option<(CipUsint, CipUsint)>,
}

/// Extended status of a Connection Manager reply (the first Additional Status word)
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ConnectionManagerExtendedStatus {
    ConnectionInUseOrDuplicateForwardOpen,
    TransportClassAndTriggerCombinationNotSupported,
    OwnershipConflict,
    TargetConnectionNotFound,
    InvalidNetworkConnectionParameter,
    InvalidConnectionSize,
    TargetForConnectionNotConfigured,
    RpiNotSupported,
    OutOfConnections,
    VendorIdOrProductCodeMismatch,
    ProductTypeMismatch,
    RevisionMismatch,
    InvalidProducedOrConsumedApplicationPath,
    InvalidOrInconsistentConfigurationApplicationPath,
    NonListenOnlyConnectionNotOpened,
    TargetObjectOutOfConnections,
    RpiSmallerThanProductionInhibitTime,
    ConnectionTimedOut,
    UnconnectedRequestTimedOut,
    ParameterErrorInUnconnectedRequestService,
    MessageTooLargeForUnconnectedSendService,
    UnconnectedAcknowledgeWithoutReply,
    NoBufferMemoryAvailable,
    NetworkBandwidthNotAvailableForData,
    NoConsumedConnectionIdFilterAvailable,
    NotConfiguredToSendScheduledPriorityData,
    ScheduleSignatureMismatch,
    ScheduleSignatureValidationNotPossible,
    PortNotAvailable,
    LinkAddressNotValid,
    InvalidSegmentInConnectionPath,
    ErrorInForwardCloseServiceConnectionPath,
    SchedulingNotSpecified,
    LinkAddressToSelfInvalid,
    SecondaryResourcesUnavailable,
    RackConnectionAlreadyEstablished,
    ModuleConnectionAlreadyEstablished,
    Miscellaneous,
    RedundantConnectionMismatch,
    NoMoreUserConfigurableLinkConsumerResourcesAvailableInTheProducingModule,
    NoUserConfigurableLinkConsumerResourcesConfiguredInTheProducingModule,
    NetworkLinkOffline,
    NoTargetApplicationDataAvailable,
    NoOriginatorApplicationDataAvailable,
    NodeAddressHasChangedSinceTheNetworkWasScheduled,
    NotConfiguredForOffSubnetMulticast,
    InvalidProduceConsumeDataFormat,
    Unknown(u16),
}

// ======= Start of ConnectionManagerExtendedStatus impl ========

impl From<u16> for ConnectionManagerExtendedStatus {
    fn from(code: u16) -> Self {
        use ConnectionManagerExtendedStatus::*;
        match code {
            0x0100 => ConnectionInUseOrDuplicateForwardOpen,
            0x0103 => TransportClassAndTriggerCombinationNotSupported,
            0x0106 => OwnershipConflict,
            0x0107 => TargetConnectionNotFound,
            0x0108 => InvalidNetworkConnectionParameter,
            0x0109 => InvalidConnectionSize,
            0x0110 => TargetForConnectionNotConfigured,
            0x0111 => RpiNotSupported,
            0x0113 => OutOfConnections,
            0x0114 => VendorIdOrProductCodeMismatch,
            0x0115 => ProductTypeMismatch,
            0x0116 => RevisionMismatch,
            0x0117 => InvalidProducedOrConsumedApplicationPath,
            0x0118 => InvalidOrInconsistentConfigurationApplicationPath,
            0x0119 => NonListenOnlyConnectionNotOpened,
            0x011A => TargetObjectOutOfConnections,
            0x011B => RpiSmallerThanProductionInhibitTime,
            0x0203 => ConnectionTimedOut,
            0x0204 => UnconnectedRequestTimedOut,
            0x0205 => ParameterErrorInUnconnectedRequestService,
            0x0206 => MessageTooLargeForUnconnectedSendService,
            0x0207 => UnconnectedAcknowledgeWithoutReply,
            0x0301 => NoBufferMemoryAvailable,
            0x0302 => NetworkBandwidthNotAvailableForData,
            0x0303 => NoConsumedConnectionIdFilterAvailable,
            0x0304 => NotConfiguredToSendScheduledPriorityData,
            0x0305 => ScheduleSignatureMismatch,
            0x0306 => ScheduleSignatureValidationNotPossible,
            0x0311 => PortNotAvailable,
            0x0312 => LinkAddressNotValid,
            0x0315 => InvalidSegmentInConnectionPath,
            0x0316 => ErrorInForwardCloseServiceConnectionPath,
            0x0317 => SchedulingNotSpecified,
            0x0318 => LinkAddressToSelfInvalid,
            0x0319 => SecondaryResourcesUnavailable,
            0x031A => RackConnectionAlreadyEstablished,
            0x031B => ModuleConnectionAlreadyEstablished,
            0x031C => Miscellaneous,
            0x031D => RedundantConnectionMismatch,
            0x031E => NoMoreUserConfigurableLinkConsumerResourcesAvailableInTheProducingModule,
            0x031F => NoUserConfigurableLinkConsumerResourcesConfiguredInTheProducingModule,
            0x0800 => NetworkLinkOffline,
            0x0810 => NoTargetApplicationDataAvailable,
            0x0811 => NoOriginatorApplicationDataAvailable,
            0x0812 => NodeAddressHasChangedSinceTheNetworkWasScheduled,
            0x0813 => NotConfiguredForOffSubnetMulticast,
            0x0814 => InvalidProduceConsumeDataFormat,
            other => Unknown(other),
        }
    }
}

impl ConnectionManagerExtendedStatus {
    /// The 16-bit status word as it appears in the reply
    pub fn code(&self) -> u16 {
        use ConnectionManagerExtendedStatus::*;
        match self {
            ConnectionInUseOrDuplicateForwardOpen => 0x0100,
            TransportClassAndTriggerCombinationNotSupported => 0x0103,
            OwnershipConflict => 0x0106,
            TargetConnectionNotFound => 0x0107,
            InvalidNetworkConnectionParameter => 0x0108,
            InvalidConnectionSize => 0x0109,
            TargetForConnectionNotConfigured => 0x0110,
            RpiNotSupported => 0x0111,
            OutOfConnections => 0x0113,
            VendorIdOrProductCodeMismatch => 0x0114,
            ProductTypeMismatch => 0x0115,
            RevisionMismatch => 0x0116,
            InvalidProducedOrConsumedApplicationPath => 0x0117,
            InvalidOrInconsistentConfigurationApplicationPath => 0x0118,
            NonListenOnlyConnectionNotOpened => 0x0119,
            TargetObjectOutOfConnections => 0x011A,
            RpiSmallerThanProductionInhibitTime => 0x011B,
            ConnectionTimedOut => 0x0203,
            UnconnectedRequestTimedOut => 0x0204,
            ParameterErrorInUnconnectedRequestService => 0x0205,
            MessageTooLargeForUnconnectedSendService => 0x0206,
            UnconnectedAcknowledgeWithoutReply => 0x0207,
            NoBufferMemoryAvailable => 0x0301,
            NetworkBandwidthNotAvailableForData => 0x0302,
            NoConsumedConnectionIdFilterAvailable => 0x0303,
            NotConfiguredToSendScheduledPriorityData => 0x0304,
            ScheduleSignatureMismatch => 0x0305,
            ScheduleSignatureValidationNotPossible => 0x0306,
            PortNotAvailable => 0x0311,
            LinkAddressNotValid => 0x0312,
            InvalidSegmentInConnectionPath => 0x0315,
            ErrorInForwardCloseServiceConnectionPath => 0x0316,
            SchedulingNotSpecified => 0x0317,
            LinkAddressToSelfInvalid => 0x0318,
            SecondaryResourcesUnavailable => 0x0319,
            RackConnectionAlreadyEstablished => 0x031A,
            ModuleConnectionAlreadyEstablished => 0x031B,
            Miscellaneous => 0x031C,
            RedundantConnectionMismatch => 0x031D,
            NoMoreUserConfigurableLinkConsumerResourcesAvailableInTheProducingModule => 0x031E,
            NoUserConfigurableLinkConsumerResourcesConfiguredInTheProducingModule => 0x031F,
            NetworkLinkOffline => 0x0800,
            NoTargetApplicationDataAvailable => 0x0810,
            NoOriginatorApplicationDataAvailable => 0x0811,
            NodeAddressHasChangedSinceTheNetworkWasScheduled => 0x0812,
            NotConfiguredForOffSubnetMulticast => 0x0813,
            InvalidProduceConsumeDataFormat => 0x0814,
            Unknown(code) => *code,
        }
    }
}

// ^^^^^^^^ End of ConnectionManagerExtendedStatus impl ^^^^^^^^

/// Why the target rejected a Forward_Open
#[derive(Debug, PartialEq, Clone)]
pub struct ForwardOpenFailure {
    pub general_status: ResponseStatusCode,
    /// The first Additional Status word, if the reply carried one
    pub extended_status: Option<ConnectionManagerExtendedStatus>,
    pub additional_status: Vec<CipUint>,
    /// The reply data, if the reply carried any
    pub response: Option<ForwardOpenUnsuccessfulResponse>,
}

#[derive(Debug)]
pub enum ForwardOpenError {
    /// The target answered with a general status other than success
    Rejected(ForwardOpenFailure),
    /// The reply is not a Forward_Open reply, or its data could not be parsed
    Malformed(binrw::Error),
}

impl From<binrw::Error> for ForwardOpenError {
    fn from(error: binrw::Error) -> Self {
        ForwardOpenError::Malformed(error)
    }
}

// ======= Start of ForwardOpenResponse impl ========

impl ForwardOpenResponse {
    /// Interprets the reply to a Forward_Open or Large_Forward_Open
    pub fn from_message_router_response(
        response: &MessageRouterResponse,
    ) -> Result<ForwardOpenResponse, ForwardOpenError> {
        let service = response.service_container.service();
        if service != ServiceCode::ForwardOpen && service != ServiceCode::LargeForwardOpen {
            return Err(ForwardOpenError::Malformed(binrw::Error::AssertFail {
                pos: 0,
                message: format!(
                    "expected a Forward_Open or Large_Forward_Open reply, got {service:?}"
                ),
            }));
        }

        let data = response.response_data.data.to_bytes()?;
        let mut reader = Cursor::new(&data);

        if response.is_success() {
            return Ok(ForwardOpenResponse::read(&mut reader)?);
        }

        let unsuccessful_response = if data.is_empty() {
            None
        } else {
            Some(ForwardOpenUnsuccessfulResponse::read(&mut reader)?)
        };

        Err(ForwardOpenError::Rejected(ForwardOpenFailure {
            general_status: response.response_data.status,
            extended_status: response
                .response_data
                .additional_status
                .first()
                .map(|&code| ConnectionManagerExtendedStatus::from(code)),
            additional_status: response.response_data.additional_status.clone(),
            response: unsuccessful_response,
        }))
    }
}

// ^^^^^^^^ End of ForwardOpenResponse impl ^^^^^^^^

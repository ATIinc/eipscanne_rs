//! The input assembly of a Teknic IO-HUB-4-E (instance 100, 228 bytes): what the hub reports.
//! ClearPath-IP Software Reference, Appendix H, IO-HUB-4-E T2O Input Assembly:
//! https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=60
//!
//! Read only: these types are decoded from replies and never built, so none of the bitfields
//! needs a builder.

use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{DebugBits, FromBits, TryFromBits, bitsize, u2, u3, u4, u10, u56};

use eipscanne_rs::cip::types::{CipDint, CipInt, CipUint, CipUsint};

/// Assembly instance that produces the inputs (the full-featured variant)
pub const INPUT_ASSEMBLY_INSTANCE: u8 = 0x64;

/// Motor Statusword: the real-time status bits of one motor
/// (https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=49)
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct MotorStatusword {
    /// Bit 0: motor windings energized
    pub enabled: bool,
    /// Bit 1: ready to receive motion commands
    pub ready_for_command: bool,
    /// Bit 2: in a shutdown state, see the shutdown register
    pub motor_shutdown_present: bool,
    /// Bit 3: non-critical warnings present, see the warning register
    pub motor_warning_present: bool,
    /// Bit 4: actual position within the in-range window of the commanded position
    pub in_range: bool,
    /// Bit 5: enabled and no motion being commanded
    pub command_complete: bool,
    /// Bit 6: in range and command complete for the settled verify time
    pub settled: bool,
    /// Bit 7: moving at the target velocity, within the velocity window
    pub at_speed: bool,
    /// Bit 8: homing routine in progress
    pub homing: bool,
    /// Bit 9: homing completed since power-up
    pub has_homed: bool,
    /// Bit 10: external brake output active (brake released)
    pub brake_released: bool,
    reserved_11_14: u4,
    /// Bit 15: settled at the target of a position move
    pub at_target_position: bool,
    /// Bit 16: configured to follow another axis
    pub following_configured: bool,
    /// Bit 17: currently following a master axis
    pub actively_following: bool,
    /// Bit 18: other motors are configured to follow this one
    pub is_followed: bool,
    /// Bit 19: the last move was cancelled, see the warning register
    pub move_cancelled: bool,
    /// Bit 20: shutdowns cleared (handshakes the Shutdown Reset controlword bit)
    pub shutdown_reset_ack: bool,
    /// Bit 21: home sensor active
    pub in_home_sensor: bool,
    /// Bit 22: motion blocked by the stop sensor or the Stop All Motion controlword bit
    pub all_motion_blocked: bool,
    /// Bit 23: positive limit input active
    pub in_pos_limit_switch: bool,
    /// Bit 24: negative limit input active
    pub in_neg_limit_switch: bool,
    /// Bit 25: at or beyond the positive software limit
    pub in_pos_soft_limit: bool,
    /// Bit 26: at or beyond the negative software limit
    pub in_neg_soft_limit: bool,
    /// Bit 27: parameter write succeeded (handshakes the Write Parameter controlword bit)
    pub write_parameter_ack: bool,
    /// Bit 28: position capture sensor active
    pub position_capture_sensor_state: bool,
    /// Bit 29: motor model is IPSK or IPHP (not IPVC)
    pub motor_model_type: bool,
    reserved_30: bool,
    /// Bit 31: motor connected and communicating with the hub
    pub motor_connected: bool,
}

/// Motor Shutdown Register: which shutdowns (faults) are present
/// (https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=45)
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct MotorShutdownRegister {
    pub unit_requires_repair: bool,
    pub firmware_problem: u2,
    reserved_3: bool,
    pub startup_issue: bool,
    pub config_load_required: bool,
    pub encoder_noise: bool,
    pub ras_problem: u2,
    pub motor_phase_overload: bool,
    pub rms_torque_limit_exceeded: bool,
    pub tracking_error_limit_exceeded: bool,
    pub e_stopped: bool,
    reserved_13: bool,
    pub excessive_motor_temp: bool,
    pub bus_voltage_lost: bool,
    pub max_bus_voltage_exceeded: bool,
    pub excessive_bus_current: bool,
    reserved_18_20: u3,
    pub motor_reset_unexpectedly: bool,
    reserved_22_31: u10,
}

/// Motor Warning Register: non-critical warnings and why the last move was cancelled
/// (https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=47)
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct MotorWarningRegister {
    pub torque_saturation: bool,
    pub voltage_saturation: bool,
    pub over_speed: bool,
    reserved_3_4: u2,
    pub high_temp: bool,
    pub high_rms_torque: bool,
    pub bus_voltage_low: bool,
    reserved_8: bool,
    pub access_conflict: bool,
    pub io_hub_warning_present: bool,
    pub write_parameter_error: bool,
    pub read_parameter_error: bool,
    pub motor_link_errors_detected: bool,
    pub motor_link_timeout: bool,
    pub motor_io_config_invalid: bool,
    pub move_canceled_pos_limit: bool,
    pub move_canceled_neg_limit: bool,
    pub move_canceled_pos_soft_limit: bool,
    pub move_canceled_neg_soft_limit: bool,
    pub move_canceled_disabled_or_shutdown: bool,
    pub move_canceled_stop_switch: bool,
    pub move_canceled_slave_axis_interrupted: bool,
    pub move_canceled_motor_disconnected: bool,
    pub move_canceled_stop_all_motion: bool,
    pub move_canceled_communication_lost: bool,
    reserved_26_27: u2,
    pub ac_wiring_error: bool,
    pub ac_loss: bool,
    reserved_30_31: u2,
}

/// AOI Error Code: what `move_type_ack` carries when a move is rejected (values of 100 and up)
/// (https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=43)
#[bitsize(8)]
#[derive(TryFromBits, PartialEq, Copy, Clone, Debug)]
pub enum AoiErrorCode {
    NoError = 0,
    InvalidMoveType = 100,
    UnsupportedMoveType = 101,
    MotorInShutdown = 102,
    MotorDisabled = 103,
    ActiveLimitSwitch = 104,
    SoftwareLimitExceeded = 105,
    MotionStoppedByControlword = 106,
    StopSensorActive = 107,
    InvalidMotionParameters = 108,
    MotorUnreachable = 109,
    RedefinePositionBlocked = 110,
    SlaveAxisDisabled = 120,
    InvalidGearRatio = 121,
    InvalidMasterAxis = 122,
    MoveNotAllowedWhileFollowing = 123,
    MoveNotAllowedWhileBeingFollowed = 124,
    AlreadyFollowingDifferentAxis = 125,
    AlreadyBeingFollowed = 126,
    FollowingRejectedMoveInProgress = 128,
    IndeterminateFollowingError = 129,
    MoveCanceledWarning = 254,
    MoveCanceledShutdown = 255,
}

/// Per-motor input data, 44 bytes per motor
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct MotorInputData {
    pub statusword: MotorStatusword,
    pub shutdown_register: MotorShutdownRegister,
    pub warning_register: MotorWarningRegister,
    pub position_measured: CipDint,
    pub velocity_measured: CipDint,
    pub torque_measured: CipInt,
    pub position_target: CipDint,
    pub velocity_target: CipDint,
    pub torque_target: CipInt,
    pub read_parameter_id_echo: CipInt,
    pub read_parameter_value: CipDint,
    /// The move type of the last command, or an AOI error code when it was rejected
    pub move_type_ack: CipUsint,
    #[brw(pad_after = 3)]
    pub move_number_ack: CipInt,
}

// ======= Start of MotorInputData impl ========

impl MotorInputData {
    /// Why the last move was rejected, if it was: `move_type_ack` read as an AOI error code
    /// (`None` for a move type, or an unknown code)
    pub fn rejection(&self) -> Option<AoiErrorCode> {
        match AoiErrorCode::try_from(self.move_type_ack) {
            Ok(AoiErrorCode::NoError) | Err(_) => None,
            Ok(code) if self.move_type_ack >= 100 => Some(code),
            Ok(_) => None,
        }
    }
}

// ^^^^^^^^ End of MotorInputData impl ^^^^^^^^

/// Analog inputs I/O-0 through I/O-12, in millivolts.
///
/// UINT, as the hub's EDS declares them (Param133-Param145). The Software Reference lists the
/// matching `AnalogInputs` tag as INT[13]
/// (https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=56); the EDS
/// is followed here, and the documentation may be wrong.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct AnalogInputs {
    pub io0: CipUint,
    pub io1: CipUint,
    pub io2: CipUint,
    pub io3: CipUint,
    pub io4: CipUint,
    pub io5: CipUint,
    pub io6: CipUint,
    pub io7: CipUint,
    pub io8: CipUint,
    pub io9: CipUint,
    pub io10: CipUint,
    pub io11: CipUint,
    pub io12: CipUint,
}

/// Digital inputs I/O-0 through I/O-12
#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
pub struct DigitalInputs {
    pub io0: bool,
    pub io1: bool,
    pub io2: bool,
    pub io3: bool,
    pub io4: bool,
    pub io5: bool,
    pub io6: bool,
    pub io7: bool,
    pub io8: bool,
    pub io9: bool,
    pub io10: bool,
    pub io11: bool,
    pub io12: bool,
    reserved: u3,
}

/// The electrical mode of one I/O point, a 4-bit nibble
/// (https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=68)
#[bitsize(4)]
#[derive(TryFromBits, PartialEq, Copy, Clone, Debug)]
pub enum IoConfigMode {
    NpnDigitalInput = 0,
    PnpDigitalInput = 1,
    PushPull24VDigitalOutput = 2,
    OpenCollectorDigitalOutput = 3,
    PushPull24VPwmOutput = 4,
    OpenCollectorPwmOutput = 5,
    AnalogInput = 6,
    AnalogOutput = 7,
}

/// The 7-byte I/O configuration block: the mode of each I/O point as a nibble, io0 in the low
/// nibble of the first byte, io12 in the low nibble of the last, whose high nibble is padding
#[bitsize(56)]
#[derive(TryFromBits, DebugBits, PartialEq, Copy, Clone, BinRead, BinWrite)]
#[br(little, try_map = |raw: [u8; 7]| {
    let packed = u64::from_le_bytes([raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], 0]);
    IoPointConfiguration::try_from(u56::new(packed)).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "an I/O configuration nibble is not a known mode",
        )
    })
})]
#[bw(little, map = |&x| -> [u8; 7] {
    let packed = u64::from(u56::from(x)).to_le_bytes();
    [packed[0], packed[1], packed[2], packed[3], packed[4], packed[5], packed[6]]
})]
pub struct IoPointConfiguration {
    pub io0: IoConfigMode,
    pub io1: IoConfigMode,
    pub io2: IoConfigMode,
    pub io3: IoConfigMode,
    pub io4: IoConfigMode,
    pub io5: IoConfigMode,
    pub io6: IoConfigMode,
    pub io7: IoConfigMode,
    pub io8: IoConfigMode,
    pub io9: IoConfigMode,
    pub io10: IoConfigMode,
    pub io11: IoConfigMode,
    pub io12: IoConfigMode,
    reserved: u4,
}

/// I/O input data, 36 bytes
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct IoInputData {
    pub digital_inputs: DigitalInputs,
    pub io_configuration: IoPointConfiguration,
    #[brw(pad_before = 1)]
    pub analog_inputs: AnalogInputs,
}

/// Encoder input data, 16 bytes
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct EncoderInputData {
    pub external_encoder_position: CipDint,
    pub external_encoder_velocity: CipDint,
    pub external_encoder_index_position: CipDint,
    pub external_encoder_alarm_flag: CipUsint,
    #[brw(pad_after = 2)]
    pub external_encoder_add_to_position_ack: CipUsint,
}

/// The T->O input assembly of the IO-HUB-4-E (instance 100): I/O, four motors and the encoder
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct InputAssemblyHub4E {
    pub io_input_data: IoInputData,
    motor0_input_data: MotorInputData,
    motor1_input_data: MotorInputData,
    motor2_input_data: MotorInputData,
    motor3_input_data: MotorInputData,
    pub encoder_input_data: EncoderInputData,
}

// ======= Start of InputAssemblyHub4E impl ========

impl InputAssemblyHub4E {
    /// The inputs of motor port `index` (0 = M0 ... 3 = M3)
    pub fn motor_input(&self, index: u8) -> &MotorInputData {
        match index {
            0 => &self.motor0_input_data,
            1 => &self.motor1_input_data,
            2 => &self.motor2_input_data,
            3 => &self.motor3_input_data,
            _ => panic!("motor index must be 0-3, got {index}"),
        }
    }
}

// ^^^^^^^^ End of InputAssemblyHub4E impl ^^^^^^^^

use binrw::{BinRead, BinWrite, binrw};

use bilge::{
    TryFromBits,
    prelude::{Bitsized, DebugBits, FromBits, Number, bitsize, u2, u3, u4, u10, u56},
};


use eipscanne_rs::cip::types::{CipDint, CipInt, CipUint, CipUsint};


/// Analog input connectors: I/O-0 through I/O-12 (hardware reports millivolts)
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AnalogInputPin {
    IO0 = 0,
    IO1 = 1,
    IO2 = 2,
    IO3 = 3,
    IO4 = 4,
    IO5 = 5,
    IO6 = 6,
    IO7 = 7,
    IO8 = 8,
    IO9 = 9,
    IO10 = 10,
    IO11 = 11,
    IO12 = 12,
}

/// Any I/O point on the IO-HUB-4-E (I/O-0 through I/O-12).
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum IoPin {
    IO0 = 0,
    IO1 = 1,
    IO2 = 2,
    IO3 = 3,
    IO4 = 4,
    IO5 = 5,
    IO6 = 6,
    IO7 = 7,
    IO8 = 8,
    IO9 = 9,
    IO10 = 10,
    IO11 = 11,
    IO12 = 12,
}

/// Digital input connectors: I/O-0 through I/O-12
pub type DigitalInputPin = IoPin;

/// Motor Statusword - 32-bit real-time status information per motor.
/// ClearPath-IP Software Reference, Appendix D: Motor Statusword
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct MotorStatusword {
    /// Bit 0 — Enabled
    ///
    /// | State   | Meaning                          |
    /// |---------|----------------------------------|
    /// | 1 (ON)  | Motor windings energized.        |
    /// | 0 (OFF) | Motor is disabled/de-energized.  |
    pub enabled: bool,
    /// Bit 1 — Ready for Command
    ///
    /// | State   | Meaning                                                                    |
    /// |---------|----------------------------------------------------------------------------|
    /// | 1 (ON)  | Servo is ready to receive motion commands.                                 |
    /// | 0 (OFF) | Motor is disabled, shutdown, or startup sequence is in process.            |
    pub ready_for_command: bool,
    /// Bit 2 — Motor Shutdown Present
    ///
    /// | State   | Meaning                                                                    |
    /// |---------|----------------------------------------------------------------------------|
    /// | 1 (ON)  | The motor is in a shutdown state; refer to the Motor Shutdown Register.    |
    /// | 0 (OFF) | No shutdowns present.                                                      |
    pub motor_shutdown_present: bool,
    /// Bit 3 — Motor Warning Present
    ///
    /// | State   | Meaning                                                                          |
    /// |---------|----------------------------------------------------------------------------------|
    /// | 1 (ON)  | The motor has non-critical warning(s) present; refer to the Motor Warning Register. |
    /// | 0 (OFF) | No warnings present.                                                             |
    pub motor_warning_present: bool,
    /// Bit 4 — In Range
    ///
    /// | State   | Meaning                                                                              |
    /// |---------|--------------------------------------------------------------------------------------|
    /// | 1 (ON)  | Actual position is within the configured In-Range Window of its commanded position.  |
    /// | 0 (OFF) | Position error exceeds the In-Range Window.                                          |
    pub in_range: bool,
    /// Bit 5 — Command Complete
    ///
    /// | State   | Meaning                                                               |
    /// |---------|-----------------------------------------------------------------------|
    /// | 1 (ON)  | Motor is enabled and no motion is being commanded.                    |
    /// | 0 (OFF) | A motion command is executing, or the motor is shutdown/disabled.     |
    pub command_complete: bool,
    /// Bit 6 — Settled
    ///
    /// | State   | Meaning                                                                    |
    /// |---------|----------------------------------------------------------------------------|
    /// | 1 (ON)  | Motor is In Range and Command Complete for the Settled Verify Time.        |
    /// | 0 (OFF) | Motor is moving, or no longer within the In-Range Window.                  |
    pub settled: bool,
    /// Bit 7 — At Speed
    ///
    /// | State   | Meaning                                                                                    |
    /// |---------|--------------------------------------------------------------------------------------------|
    /// | 1 (ON)  | Motor is moving and actual velocity is within the configured Velocity Window of target.    |
    /// | 0 (OFF) | Velocity error exceeds the Velocity Window, motor is not moving, or motor is homing.       |
    pub at_speed: bool,
    /// Bit 8 — Homing
    ///
    /// | State   | Meaning                        |
    /// |---------|--------------------------------|
    /// | 1 (ON)  | Homing routine in progress.    |
    /// | 0 (OFF) | Motor is not homing.           |
    pub homing: bool,
    /// Bit 9 — Has Homed
    ///
    /// | State   | Meaning                                    |
    /// |---------|--------------------------------------------|
    /// | 1 (ON)  | Homing has completed successfully.         |
    /// | 0 (OFF) | Motor has not homed since power-up.        |
    pub has_homed: bool,
    /// Bit 10 — Brake Released
    ///
    /// | State   | Meaning                                                                                          |
    /// |---------|--------------------------------------------------------------------------------------------------|
    /// | 1 (ON)  | External brake output is active (brake released). If no brake output configured, stays ON by default. |
    /// | 0 (OFF) | External brake is de-energized and holding the load.                                             |
    pub brake_released: bool,
    /// Bits 11-14: Reserved
    reserved_11_14: u4,
    /// Bit 15 — At Target Position
    ///
    /// | State   | Meaning                                                                              |
    /// |---------|--------------------------------------------------------------------------------------|
    /// | 1 (ON)  | Motor is Settled at its target position; position move has successfully completed.   |
    /// | 0 (OFF) | Motor is moving, or was prevented from reaching its target position.                 |
    pub at_target_position: bool,
    /// Bit 16 — Following Configured
    ///
    /// | State   | Meaning                                           |
    /// |---------|---------------------------------------------------|
    /// | 1 (ON)  | Motor is configured to follow another axis.       |
    /// | 0 (OFF) | Motor not configured for following.               |
    pub following_configured: bool,
    /// Bit 17 — Actively Following
    ///
    /// | State   | Meaning                                           |
    /// |---------|---------------------------------------------------|
    /// | 1 (ON)  | Motor is currently following a master axis.       |
    /// | 0 (OFF) | Motor is not following.                           |
    pub actively_following: bool,
    /// Bit 18 — Is Followed
    ///
    /// | State   | Meaning                                                |
    /// |---------|--------------------------------------------------------|
    /// | 1 (ON)  | Other motor(s) are configured to follow this motor.    |
    /// | 0 (OFF) | —                                                      |
    pub is_followed: bool,
    /// Bit 19 — Move Cancelled
    ///
    /// | State   | Meaning                                                               |
    /// |---------|-----------------------------------------------------------------------|
    /// | 1 (ON)  | The last move was cancelled; refer to the Motor Warning Register.     |
    /// | 0 (OFF) | —                                                                     |
    pub move_cancelled: bool,
    /// Bit 20 — Shutdown Reset Ack
    ///
    /// | State   | Meaning                                                                             |
    /// |---------|-------------------------------------------------------------------------------------|
    /// | 1 (ON)  | Motor shutdowns were cleared successfully (handshakes Controlword "Shutdown Reset"). |
    /// | 0 (OFF) | —                                                                                   |
    pub shutdown_reset_ack: bool,
    /// Bit 21 — In Home Sensor
    ///
    /// | State   | Meaning                                              |
    /// |---------|------------------------------------------------------|
    /// | 1 (ON)  | Motor's home sensor is active.                       |
    /// | 0 (OFF) | Motor's home sensor is not configured or inactive.   |
    pub in_home_sensor: bool,
    /// Bit 22 — All Motion Blocked
    ///
    /// | State   | Meaning                                                                       |
    /// |---------|-------------------------------------------------------------------------------|
    /// | 1 (ON)  | Motion blocked by active Stop Sensor or Controlword "Stop All Motion".        |
    /// | 0 (OFF) | —                                                                             |
    pub all_motion_blocked: bool,
    /// Bit 23 — In (+) Limit Switch
    ///
    /// | State   | Meaning                                                                            |
    /// |---------|------------------------------------------------------------------------------------|
    /// | 1 (ON)  | Positive limit input on I/O HUB is active, or Controlword "Ext Positive Limit" is ON. |
    /// | 0 (OFF) | —                                                                                  |
    pub in_pos_limit_switch: bool,
    /// Bit 24 — In (-) Limit Switch
    ///
    /// | State   | Meaning                                                                            |
    /// |---------|------------------------------------------------------------------------------------|
    /// | 1 (ON)  | Negative limit input on I/O HUB is active, or Controlword "Ext Negative Limit" is ON. |
    /// | 0 (OFF) | —                                                                                  |
    pub in_neg_limit_switch: bool,
    /// Bit 25 — In (+) Soft Limit
    ///
    /// | State   | Meaning                                                                           |
    /// |---------|-----------------------------------------------------------------------------------|
    /// | 1 (ON)  | Position meets or exceeds the positive software position limit.                   |
    /// | 0 (OFF) | Soft limit not reached, motor not homed, or soft limits not configured.           |
    pub in_pos_soft_limit: bool,
    /// Bit 26 — In (-) Soft Limit
    ///
    /// | State   | Meaning                                                                           |
    /// |---------|-----------------------------------------------------------------------------------|
    /// | 1 (ON)  | Position meets or exceeds the negative software position limit.                   |
    /// | 0 (OFF) | Soft limit not reached, motor not homed, or soft limits not configured.           |
    pub in_neg_soft_limit: bool,
    /// Bit 27 — Write Parameter Ack
    ///
    /// | State   | Meaning                                                                         |
    /// |---------|---------------------------------------------------------------------------------|
    /// | 1 (ON)  | Parameter write was successful (handshakes Controlword "Write Parameter").      |
    /// | 0 (OFF) | —                                                                               |
    pub write_parameter_ack: bool,
    /// Bit 28 — Position Capture Sensor State
    ///
    /// | State   | Meaning                                          |
    /// |---------|--------------------------------------------------|
    /// | 1 (ON)  | Position capture sensor is active.               |
    /// | 0 (OFF) | Sensor not configured or inactive.               |
    pub position_capture_sensor_state: bool,
    /// Bit 29 — Motor Model Type
    ///
    /// | State   | Meaning                      |
    /// |---------|------------------------------|
    /// | 1 (ON)  | Motor model is IPSK or IPHP. |
    /// | 0 (OFF) | Motor model is IPVC.         |
    pub motor_model_type: bool,
    /// Bit 30: Reserved
    reserved_30: bool,
    /// Bit 31 — Motor Connected
    ///
    /// | State   | Meaning                                                              |
    /// |---------|----------------------------------------------------------------------|
    /// | 1 (ON)  | Motor is connected and communicating with the I/O HUB.               |
    /// | 0 (OFF) | Motor is disconnected or has communication problems.                 |
    pub motor_connected: bool,
}

/// Motor Shutdown Register - 32-bit real-time info on what shutdowns/faults are present.
/// ClearPath-IP Software Reference, Appendix B: Motor Shutdown Register Breakdown
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct MotorShutdownRegister {
    /// Bit 0: Unit requires repair - return to Teknic
    pub unit_requires_repair: bool,
    /// Bits 1-2: Firmware problem
    pub firmware_problem: u2,
    /// Bit 3: Reserved
    reserved_3: bool,
    /// Bit 4: Motor failed startup sequence
    pub startup_issue: bool,
    /// Bit 5: Load a config file compatible with motor's firmware version
    pub config_load_required: bool,
    /// Bit 6: Encoder noise detected
    pub encoder_noise: bool,
    /// Bits 7-8: RAS problem
    pub ras_problem: u2,
    /// Bit 9: Phase current beyond allowed ADC limit
    pub motor_phase_overload: bool,
    /// Bit 10: RMS torque limit exceeded
    pub rms_torque_limit_exceeded: bool,
    /// Bit 11: Tracking error limit exceeded
    pub tracking_error_limit_exceeded: bool,
    /// Bit 12: E-Stopped via ClearView
    pub e_stopped: bool,
    /// Bit 13: Reserved
    reserved_13: bool,
    /// Bit 14: Temperature exceeded specified limit
    pub excessive_motor_temp: bool,
    /// Bit 15: Power supply disconnected or brown out
    pub bus_voltage_lost: bool,
    /// Bit 16: Large regenerated voltage upon deceleration
    pub max_bus_voltage_exceeded: bool,
    /// Bit 17: Bad tuning, low bus voltage, or overloaded power supply
    pub excessive_bus_current: bool,
    /// Bits 18-20: Reserved
    reserved_18_20: u3,
    /// Bit 21: Motor power cycled or control cables swapped
    pub motor_reset_unexpectedly: bool,
    /// Bits 22-31: Reserved
    extra_padding: u10,
}

/// Motor Warning Register - 32-bit real-time info on non-critical warnings.
/// ClearPath-IP Software Reference, Appendix C: Motor Warning Register Breakdown
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct MotorWarningRegister {
    /// Bit 0: Motor has reached its torque limit
    pub torque_saturation: bool,
    /// Bit 1: Available torque limited by DC bus voltage
    pub voltage_saturation: bool,
    /// Bit 2: Commanded speed exceeds motor max speed limit
    pub over_speed: bool,
    /// Bits 3-4: Reserved
    reserved_3_4: u2,
    /// Bit 5: Motor temperature approaching specified limit
    pub high_temp: bool,
    /// Bit 6: RMS torque approaching max limit
    pub high_rms_torque: bool,
    /// Bit 7: Bus voltage below user-specified operating voltage
    pub bus_voltage_low: bool,
    /// Bit 8: Reserved
    reserved_8: bool,
    /// Bit 9: ClearView has full access, rejecting all commands
    pub access_conflict: bool,
    /// Bit 10: Warning present on the I/O HUB
    pub io_hub_warning_present: bool,
    /// Bit 11: Writing to parameter failed (invalid ID)
    pub write_parameter_error: bool,
    /// Bit 12: Read parameter failed (invalid ID)
    pub read_parameter_error: bool,
    /// Bit 13: Errors in motor-to-hub connection
    pub motor_link_errors_detected: bool,
    /// Bit 14: Motor-to-hub connection timed out
    pub motor_link_timeout: bool,
    /// Bit 15: Motor I/O incorrectly configured
    pub motor_io_config_invalid: bool,
    /// Bit 16: Move canceled by positive limit switch
    pub move_canceled_pos_limit: bool,
    /// Bit 17: Move canceled by negative limit switch
    pub move_canceled_neg_limit: bool,
    /// Bit 18: Move canceled by positive soft limit
    pub move_canceled_pos_soft_limit: bool,
    /// Bit 19: Move canceled by negative soft limit
    pub move_canceled_neg_soft_limit: bool,
    /// Bit 20: Move canceled because motor disabled or shutdown
    pub move_canceled_disabled_or_shutdown: bool,
    /// Bit 21: Move canceled by stop switch
    pub move_canceled_stop_switch: bool,
    /// Bit 22: Move canceled because slave axis interrupted
    pub move_canceled_slave_axis_interrupted: bool,
    /// Bit 23: Move canceled because motor disconnected
    pub move_canceled_motor_disconnected: bool,
    /// Bit 24: Move canceled by Stop All Motion controlword bit
    pub move_canceled_stop_all_motion: bool,
    /// Bit 25: Move canceled due to EtherNet/IP communication loss
    pub move_canceled_communication_lost: bool,
    /// Bits 26-27: Reserved
    reserved_26_27: u2,
    /// Bit 28: AC wiring error (phase mismatch)
    pub ac_wiring_error: bool,
    /// Bit 29: AC loss (interlock open, bad wiring)
    pub ac_loss: bool,
    /// Bits 30-31: Reserved
    reserved_30_31: u2,
}

/// AOI Error Code returned in `move_type_ack` when a move is rejected (value >= 100).
/// ClearPath-IP Software Reference, Appendix A: AOI Error Codes
#[bitsize(8)]
#[derive(TryFromBits, PartialEq, Copy, Clone, Debug)]
pub enum AoiErrorCode {
    /// 0: No error
    NoError = 0,
    /// 100: Invalid Move Type - requested move type not recognized
    InvalidMoveType = 100,
    /// 101: Unsupported Move Type - not supported by this motor model (IPVC only)
    UnsupportedMoveType = 101,
    /// 102: Motor in Shutdown - clear all shutdowns before commanding motion
    MotorInShutdown = 102,
    /// 103: Motor Disabled - enable motor before commanding motion
    MotorDisabled = 103,
    /// 104: Active Limit Switch - command move in opposite direction
    ActiveLimitSwitch = 104,
    /// 105: Software Limit Exceeded - adjust target or move direction
    SoftwareLimitExceeded = 105,
    /// 106: Motion Stopped by Controlword - clear Stop All Motion bit
    MotionStoppedByControlword = 106,
    /// 107: Stop Sensor Active - reset stop sensor or clear interlock
    StopSensorActive = 107,
    /// 108: Invalid Motion Parameters - acceleration must be non-zero; position moves require positive speed limit
    InvalidMotionParameters = 108,
    /// 109: Motor Unreachable - verify network/device connections; check ClearView access level
    MotorUnreachable = 109,
    /// 110: Redefine Position Blocked - wait for homing to complete
    RedefinePositionBlocked = 110,
    /// 120: Slave Axis Disabled - enable all slaves before proceeding
    SlaveAxisDisabled = 120,
    /// 121: Invalid Gear Ratio - both numerator and denominator must be non-zero
    InvalidGearRatio = 121,
    /// 122: Invalid Master Axis - master does not exist or is not connected
    InvalidMasterAxis = 122,
    /// 123: Move Not Allowed While Following - axis is configured as a slave
    MoveNotAllowedWhileFollowing = 123,
    /// 124: Move Not Allowed While Being Followed - axis is configured as a master
    MoveNotAllowedWhileBeingFollowed = 124,
    /// 125: Already Following Different Axis - stop following before reassigning
    AlreadyFollowingDifferentAxis = 125,
    /// 126: Already Being Followed - stop following before reassigning
    AlreadyBeingFollowed = 126,
    /// 128: Following Rejected due to Move in Progress
    FollowingRejectedMoveInProgress = 128,
    /// 129: Indeterminate Following Error - contact Teknic
    IndeterminateFollowingError = 129,
    /// 254: Move Canceled (Warning) - see MoveCanceledWarning register
    MoveCanceledWarning = 254,
    /// 255: Move Canceled (Shutdown) - see MoveCanceledShutdown register
    MoveCanceledShutdown = 255,
}

/// Per-motor input data - 44 bytes per motor.
/// ClearPath-IP Software Reference, Appendix H: EtherNet/IP Assemblies
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct MotorInputData {
    pub statusword: MotorStatusword,
    pub shutdown_register: MotorShutdownRegister,
    pub warning_register: MotorWarningRegister,
    pub position_measured: CipDint,
    pub velocity_measured: CipDint,
    pub torque_measured: CipInt,
    pub position_target: CipDint,
    pub velocity_target: CipDint,
    #[brw(pad_before = 2)]
    pub read_parameter_id_echo: CipUint,
    pub read_parameter_value: CipDint,
    pub move_type_ack: CipUsint,
    #[brw(pad_after = 3)]
    pub move_number_ack: CipUint,
}

// ==================== IO-HUB-4-E specific input data ====================

/// Analog Inputs for IO-HUB-4-E - 13 inputs (I/O-0 through I/O-12, millivolts)
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct AnalogInputs {
    pub io0: CipInt,
    pub io1: CipInt,
    pub io2: CipInt,
    pub io3: CipInt,
    pub io4: CipInt,
    pub io5: CipInt,
    pub io6: CipInt,
    pub io7: CipInt,
    pub io8: CipInt,
    pub io9: CipInt,
    pub io10: CipInt,
    pub io11: CipInt,
    pub io12: CipInt,
}

impl AnalogInputs {
    pub fn get(self, pin: AnalogInputPin) -> CipInt {
        match pin {
            AnalogInputPin::IO0 => self.io0,
            AnalogInputPin::IO1 => self.io1,
            AnalogInputPin::IO2 => self.io2,
            AnalogInputPin::IO3 => self.io3,
            AnalogInputPin::IO4 => self.io4,
            AnalogInputPin::IO5 => self.io5,
            AnalogInputPin::IO6 => self.io6,
            AnalogInputPin::IO7 => self.io7,
            AnalogInputPin::IO8 => self.io8,
            AnalogInputPin::IO9 => self.io9,
            AnalogInputPin::IO10 => self.io10,
            AnalogInputPin::IO11 => self.io11,
            AnalogInputPin::IO12 => self.io12,
        }
    }
}

/// Digital Inputs for IO-HUB-4-E - 13 inputs (I/O-0 through I/O-12)
#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u16, little)]
#[bw(map = |&x| u16::from(x), little)]
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
    extra_padding: u3,
}

impl DigitalInputs {
    pub fn get(self, pin: DigitalInputPin) -> bool {
        match pin {
            DigitalInputPin::IO0 => self.io0(),
            DigitalInputPin::IO1 => self.io1(),
            DigitalInputPin::IO2 => self.io2(),
            DigitalInputPin::IO3 => self.io3(),
            DigitalInputPin::IO4 => self.io4(),
            DigitalInputPin::IO5 => self.io5(),
            DigitalInputPin::IO6 => self.io6(),
            DigitalInputPin::IO7 => self.io7(),
            DigitalInputPin::IO8 => self.io8(),
            DigitalInputPin::IO9 => self.io9(),
            DigitalInputPin::IO10 => self.io10(),
            DigitalInputPin::IO11 => self.io11(),
            DigitalInputPin::IO12 => self.io12(),
        }
    }
}

/// Electrical mode of a single I/O point, encoded as a 4-bit nibble.
/// ClearPath-IP Software Reference: I/O Configuration Modes
#[bitsize(4)]
#[derive(TryFromBits, PartialEq, Copy, Clone, Debug)]
pub enum IoConfigMode {
    /// 0: NPN Digital Input (default)
    NpnDigitalInput = 0,
    /// 1: PNP Digital Input
    PnpDigitalInput = 1,
    /// 2: 24V Push-Pull Digital Output
    PushPull24VDigitalOutput = 2,
    /// 3: 3.3V/5V Open Collector Digital Output
    OpenCollectorDigitalOutput = 3,
    /// 4: 24V Push-Pull PWM Output
    PushPull24VPwmOutput = 4,
    /// 5: 3.3V/5V Open Collector PWM Output
    OpenCollectorPwmOutput = 5,
    /// 6: Analog Input
    AnalogInput = 6,
    /// 7: Analog Output
    AnalogOutput = 7,
}

/// 7-byte I/O configuration block from the input assembly.
/// Each field reports the electrical mode of one I/O point.
/// Wire format: 13 nibbles for io0–io12 packed low-nibble-first into bytes 0–6;
/// the high nibble of byte 6 is padding and is discarded on read / zeroed on write.
#[bitsize(56)]
#[derive(TryFromBits, DebugBits, PartialEq, Copy, Clone, BinRead, BinWrite)]
#[br(little, try_map = |raw: [u8; 7]| {
    let b = u64::from_le_bytes([raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], 0]);
    IoPointConfiguration::try_from(u56::new(b)).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid IoConfigMode nibble in IoPointConfiguration",
        )
    })
})]
#[bw(little, map = |&x| -> [u8; 7] {
    let b = u64::from(u56::from(x)).to_le_bytes();
    [b[0], b[1], b[2], b[3], b[4], b[5], b[6]]
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
    padding: u4,
}

impl IoPointConfiguration {
    /// Returns the electrical mode of the given I/O pin.
    pub fn get_mode(&self, pin: IoPin) -> IoConfigMode {
        match pin {
            IoPin::IO0 => self.io0(),
            IoPin::IO1 => self.io1(),
            IoPin::IO2 => self.io2(),
            IoPin::IO3 => self.io3(),
            IoPin::IO4 => self.io4(),
            IoPin::IO5 => self.io5(),
            IoPin::IO6 => self.io6(),
            IoPin::IO7 => self.io7(),
            IoPin::IO8 => self.io8(),
            IoPin::IO9 => self.io9(),
            IoPin::IO10 => self.io10(),
            IoPin::IO11 => self.io11(),
            IoPin::IO12 => self.io12(),
        }
    }
}

/// I/O Input Data for IO-HUB-4-E - 36 bytes
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct IoInputData {
    pub digital_inputs: DigitalInputs,
    /// 7 bytes of I/O configuration (read-only, reports how each I/O point is configured)
    pub io_configuration: IoPointConfiguration,
    /// 1 byte reserved padding
    #[brw(pad_before = 1)]
    /// 13 analog inputs, 2 bytes each (millivolts)
    pub analog_inputs: AnalogInputs,
}

/// Encoder Input Data for IO-HUB-4-E - 16 bytes
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct EncoderInputData {
    pub external_encoder_position: CipDint,
    pub external_encoder_velocity: CipDint,
    pub external_encoder_index_position: CipDint,
    pub external_encoder_alarm_flag: CipUsint,
    #[brw(pad_after = 2)]
    pub external_encoder_add_to_position_ack: CipUsint,
}

// ==================== Assembly Objects ====================

/// IO-HUB-4-E T2O Input Assembly (Instance 100, 0x64) - 228 bytes
/// Full-featured variant with I/O, 4 motors, and encoder
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct InputAssemblyHub4E {
    pub io_input_data: IoInputData,
    pub motor0_input_data: MotorInputData,
    pub motor1_input_data: MotorInputData,
    pub motor2_input_data: MotorInputData,
    pub motor3_input_data: MotorInputData,
    pub encoder_input_data: EncoderInputData,
}

/// IO-HUB-4-R T2O Input Assembly (Instance 104, 0x68) - 176 bytes
/// Reduced I/O variant with 4 motors only
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct InputAssemblyHub4R {
    pub motor0_input_data: MotorInputData,
    pub motor1_input_data: MotorInputData,
    pub motor2_input_data: MotorInputData,
    pub motor3_input_data: MotorInputData,
}

/// IO-HUB-2-R T2O Input Assembly (Instance 106, 0x6A) - 88 bytes
/// Reduced I/O variant with 2 motors only
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct InputAssemblyHub2R {
    pub motor0_input_data: MotorInputData,
    pub motor1_input_data: MotorInputData,
}

#[cfg(test)]
mod test {
    use super::*;

    #[allow(dead_code)]
    const MOTOR_INPUT_DATA: MotorInputData = MotorInputData {
        statusword: MotorStatusword { value: 0x0 },
        shutdown_register: MotorShutdownRegister { value: 0x0 },
        warning_register: MotorWarningRegister { value: 0x0 },
        position_measured: 0x0,
        velocity_measured: 0x0,
        torque_measured: 0x0,
        position_target: 0x0,
        velocity_target: 0x0,
        read_parameter_id_echo: 0x0,
        read_parameter_value: 0x0,
        move_type_ack: 0x0,
        move_number_ack: 0x0,
    };

    #[test]
    fn test_motor_input_data_size() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut buf);
        MOTOR_INPUT_DATA
            .write(&mut writer)
            .expect("infallible: Cursor<Vec<u8>> writes never fail");
        assert_eq!(buf.len(), 44);
    }

    #[test]
    fn test_input_assembly_hub4e_size() {
        let io_config = IoPointConfiguration { value: u56::new(0) };
        let assembly = InputAssemblyHub4E {
            io_input_data: IoInputData {
                digital_inputs: DigitalInputs { value: 0 },
                io_configuration: io_config,
                analog_inputs: AnalogInputs {
                    io0: 0,
                    io1: 0,
                    io2: 0,
                    io3: 0,
                    io4: 0,
                    io5: 0,
                    io6: 0,
                    io7: 0,
                    io8: 0,
                    io9: 0,
                    io10: 0,
                    io11: 0,
                    io12: 0,
                },
            },
            motor0_input_data: MOTOR_INPUT_DATA,
            motor1_input_data: MOTOR_INPUT_DATA,
            motor2_input_data: MOTOR_INPUT_DATA,
            motor3_input_data: MOTOR_INPUT_DATA,
            encoder_input_data: EncoderInputData {
                external_encoder_position: 0,
                external_encoder_velocity: 0,
                external_encoder_index_position: 0,
                external_encoder_alarm_flag: 0,
                external_encoder_add_to_position_ack: 0,
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut buf);
        assembly
            .write(&mut writer)
            .expect("infallible: Cursor<Vec<u8>> writes never fail");
        assert_eq!(buf.len(), 228);
    }

    #[test]
    fn test_input_assembly_hub4r_size() {
        let assembly = InputAssemblyHub4R {
            motor0_input_data: MOTOR_INPUT_DATA,
            motor1_input_data: MOTOR_INPUT_DATA,
            motor2_input_data: MOTOR_INPUT_DATA,
            motor3_input_data: MOTOR_INPUT_DATA,
        };
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut buf);
        assembly
            .write(&mut writer)
            .expect("infallible: Cursor<Vec<u8>> writes never fail");
        assert_eq!(buf.len(), 176);
    }

    #[test]
    fn test_input_assembly_hub2r_size() {
        let assembly = InputAssemblyHub2R {
            motor0_input_data: MOTOR_INPUT_DATA,
            motor1_input_data: MOTOR_INPUT_DATA,
        };
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut buf);
        assembly
            .write(&mut writer)
            .expect("infallible: Cursor<Vec<u8>> writes never fail");
        assert_eq!(buf.len(), 88);
    }

    #[test]
    fn test_io_point_configuration_roundtrip() {
        use std::io::Cursor;

        // Each I/O gets a distinct nibble value 0–7 to exercise packing
        let config = IoPointConfiguration::new(
            IoConfigMode::NpnDigitalInput,            // 0
            IoConfigMode::PnpDigitalInput,            // 1
            IoConfigMode::PushPull24VDigitalOutput,   // 2
            IoConfigMode::OpenCollectorDigitalOutput, // 3
            IoConfigMode::PushPull24VPwmOutput,       // 4
            IoConfigMode::OpenCollectorPwmOutput,     // 5
            IoConfigMode::AnalogInput,                // 6
            IoConfigMode::AnalogOutput,               // 7
            IoConfigMode::NpnDigitalInput,            // 0
            IoConfigMode::NpnDigitalInput,            // 0
            IoConfigMode::NpnDigitalInput,            // 0
            IoConfigMode::NpnDigitalInput,            // 0
            IoConfigMode::NpnDigitalInput,            // 0 — io12
        );

        let mut buf: Vec<u8> = Vec::new();
        config.write(&mut Cursor::new(&mut buf)).unwrap();
        assert_eq!(buf.len(), 7);
        assert_eq!(buf[0], 0x10); // io0=0 (low), io1=1 (high)
        assert_eq!(buf[1], 0x32); // io2=2 (low), io3=3 (high)
        assert_eq!(buf[2], 0x54); // io4=4 (low), io5=5 (high)
        assert_eq!(buf[3], 0x76); // io6=6 (low), io7=7 (high)
        assert_eq!(buf[4], 0x00);
        assert_eq!(buf[5], 0x00);
        assert_eq!(buf[6], 0x00);

        let parsed = IoPointConfiguration::read(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(parsed, config);
    }
}

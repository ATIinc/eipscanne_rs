use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{Bitsized, DebugBits, FromBits, Number, bitsize, u4, u23};

use eipscanne_rs::cip::types::{CipDint, CipInt, CipUsint};

// ClearPath-IP Software Reference, Appendix E: Controlword & Move Commands

/// Move Type enumeration from the ClearPath-IP MoveType Reference Table.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MoveType {
    /// MoveType 1: Absolute Move (MAM)
    AbsoluteMove = 1,
    /// MoveType 2: Relative Move (MAM)
    RelativeMove = 2,
    /// MoveType 3: Velocity/Jog Move (MAJ)
    VelocityJogMove = 3,
    /// MoveType 4: Homing Move (MAH)
    HomingMove = 4,
    /// MoveType 6: Stop Move (MAS)
    StopMove = 6,
    /// MoveType 7: Configure Follower (MAG)
    ConfigureFollower = 7,
    /// MoveType 8: Redefine Position (MRP)
    RedefinePosition = 8,
}

impl MoveType {
    pub fn to_u8(self) -> CipUsint {
        self as CipUsint
    }
}

/// Motor Controlword — 32-bit value used to enable the servo and control features.
/// ClearPath-IP Software Reference, Appendix E: Motor Controlword
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct MotorControlword {
    /// Bit 0: Request the motor to enable
    pub enable: bool,
    /// Bit 1: Clear all shutdowns and move-cancelled warnings
    pub shutdown_reset: bool,
    /// Bit 2: Clear stored shutdown history
    pub clear_shutdown_history: bool,
    /// Bit 3: Clear stored warning history
    pub clear_warning_history: bool,
    /// Bit 4: External positive limit
    pub external_positive_limit: bool,
    /// Bit 5: External negative limit
    pub external_negative_limit: bool,
    /// Bit 6: Perform a parameter write via Generic Parameter Interface
    pub write_parameter: bool,
    /// Bit 7: Stop cyclically reading a parameter
    pub pause_parameter_reading: bool,
    /// Bit 8: Override all motion commands, decelerate to stop
    pub stop_all_motion: bool,
    /// Bits 9–31: Reserved
    extra_padding: u23,
}

impl MotorControlword {
    const PADDING: u23 = u23::new(0x0);
}

impl Default for MotorControlword {
    fn default() -> Self {
        MotorControlword::new(
            false, false, false, false, false, false, false, false, false,
            Self::PADDING,
        )
    }
}

/// Per-motor output data — 32 bytes per motor.
/// ClearPath-IP Software Reference, Appendix H: EtherNet/IP Assemblies
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub struct MotorOutputData {
    pub controlword: MotorControlword,
    /// Motion Parameters (position, velocity, acceleration, deceleration)
    pub move_param_1: CipDint,
    pub move_param_2: CipDint,
    pub move_param_3: CipDint,
    pub move_param_4: CipDint,
    pub move_type: CipUsint,
    pub move_number: u16,
    pub read_parameter_id: u16,
    pub write_parameter_id: u16,
    #[brw(pad_after = 1)]
    pub write_parameter_value: CipDint,
}

// ── IO-HUB-4-E specific output data ──────────────────────────────────────────

/// Digital Outputs for IO-HUB-4-E — 12 outputs (I/O-0 through I/O-11).
#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, Default)]
#[br(repr = u16, little)]
#[bw(map = |&x| u16::from(x), little)]
pub struct DigitalOutputs {
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
    extra_padding: u4,
}

/// I/O Output Data for IO-HUB-4-E — 16 bytes.
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub struct IoOutputData {
    pub digital_outputs: DigitalOutputs,
    /// Analog output for I/O-12 in microamps (0–20,000 μA)
    pub analog_output_io12_ua: CipInt,
    /// PWM duty cycles for I/O-0 through I/O-11 (0 = 0%, 255 = 100%)
    pub pwm_duty_cycles: [CipUsint; 12],
}

/// Encoder Output Data for IO-HUB-4-E — 4 bytes.
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub struct EncoderOutputData {
    pub encoder_add_to_position: CipDint,
}

/// IO-HUB-4-E O2T Output Assembly (Instance 101, 0x65) — 148 bytes.
/// Full-featured variant with I/O, 4 motors, and encoder.
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub struct OutputAssemblyHub4E {
    pub io_output_data: IoOutputData,
    pub motor0_output_data: MotorOutputData,
    pub motor1_output_data: MotorOutputData,
    pub motor2_output_data: MotorOutputData,
    pub motor3_output_data: MotorOutputData,
    pub encoder_output_data: EncoderOutputData,
}

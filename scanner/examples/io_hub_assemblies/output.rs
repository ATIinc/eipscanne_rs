//! The output assembly of a Teknic IO-HUB-4-E (instance 101, 148 bytes): what the scanner
//! commands. ClearPath-IP Software Reference, Appendix E (Controlword, move commands) and
//! Appendix H (EtherNet/IP assemblies).

use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{BuilderBits, DebugBits, DefaultBits, FromBits, bitsize, u4, u23};

use eipscanne_rs::cip::types::{CipDint, CipInt, CipUint, CipUsint};

/// Assembly instance that consumes the outputs (the full-featured variant)
pub const OUTPUT_ASSEMBLY_INSTANCE: u8 = 0x65;

/// The kind of move a `move_type` byte asks for
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum MoveType {
    AbsoluteMove = 1,
    RelativeMove = 2,
    VelocityJogMove = 3,
    HomingMove = 4,
    StopMove = 6,
    ConfigureFollower = 7,
    RedefinePosition = 8,
}

/// Motor Controlword: enables the servo and controls its features
#[bitsize(32)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct MotorControlword {
    /// Bit 0: request the motor to enable
    pub enable: bool,
    /// Bit 1: clear all shutdowns and move-cancelled warnings
    pub shutdown_reset: bool,
    /// Bit 2: clear the stored shutdown history
    pub clear_shutdown_history: bool,
    /// Bit 3: clear the stored warning history
    pub clear_warning_history: bool,
    /// Bit 4: external positive limit
    pub external_positive_limit: bool,
    /// Bit 5: external negative limit
    pub external_negative_limit: bool,
    /// Bit 6: perform a parameter write via the Generic Parameter Interface
    pub write_parameter: bool,
    /// Bit 7: stop cyclically reading a parameter
    pub pause_parameter_reading: bool,
    /// Bit 8: override all motion commands, decelerate to a stop
    pub stop_all_motion: bool,
    reserved: u23, // bits 9-31
}

/// Per-motor output data, 32 bytes per motor
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MotorOutputData {
    pub controlword: MotorControlword,
    /// Motion parameters (position, velocity, acceleration, deceleration), by move type
    pub move_param_1: CipDint,
    pub move_param_2: CipDint,
    pub move_param_3: CipDint,
    pub move_param_4: CipDint,
    /// A `MoveType`; the hub acts on it when `move_number` changes
    pub move_type: CipUsint,
    pub move_number: CipUint,
    pub read_parameter_id: CipUint,
    pub write_parameter_id: CipUint,
    #[brw(pad_after = 1)]
    pub write_parameter_value: CipDint,
}

// ======= Start of MotorOutputData impl ========

impl MotorOutputData {
    /// Asks for `move_type` as move number `move_number` (a new number per command)
    pub fn command_move(&mut self, move_type: MoveType, move_number: CipUint) {
        self.move_type = move_type as CipUsint;
        self.move_number = move_number;
    }
}

// ^^^^^^^^ End of MotorOutputData impl ^^^^^^^^

/// Digital outputs I/O-0 through I/O-11
#[bitsize(16)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
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
    reserved: u4,
}

/// I/O output data, 16 bytes
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct IoOutputData {
    pub digital_outputs: DigitalOutputs,
    /// Analog output on I/O-12 in microamps (0-20000)
    pub analog_output_io12_ua: CipInt,
    /// PWM duty cycles of I/O-0 through I/O-11 (0 = 0 %, 255 = 100 %)
    pub pwm_duty_cycles: [CipUsint; 12],
}

/// Encoder output data, 4 bytes
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EncoderOutputData {
    pub encoder_add_to_position: CipDint,
}

/// The O->T output assembly of the IO-HUB-4-E (instance 101): I/O, four motors and the encoder
#[binrw]
#[brw(little)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct OutputAssemblyHub4E {
    pub io_output_data: IoOutputData,
    motor0_output_data: MotorOutputData,
    motor1_output_data: MotorOutputData,
    motor2_output_data: MotorOutputData,
    motor3_output_data: MotorOutputData,
    pub encoder_output_data: EncoderOutputData,
}

// ======= Start of OutputAssemblyHub4E impl ========

impl OutputAssemblyHub4E {
    /// The outputs of motor port `index` (0 = M0 ... 3 = M3)
    pub fn motor_output_mut(&mut self, index: u8) -> &mut MotorOutputData {
        match index {
            0 => &mut self.motor0_output_data,
            1 => &mut self.motor1_output_data,
            2 => &mut self.motor2_output_data,
            3 => &mut self.motor3_output_data,
            _ => panic!("motor index must be 0-3, got {index}"),
        }
    }
}

// ^^^^^^^^ End of OutputAssemblyHub4E impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_assembly_is_148_bytes() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        OutputAssemblyHub4E::default().write(&mut bytes).unwrap();
        assert_eq!(bytes.into_inner().len(), 148);
    }
}

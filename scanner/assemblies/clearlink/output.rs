//! The ClearLink output assembly (instance 0x70):
//! https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=20

use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{BuilderBits, DebugBits, DefaultBits, FromBits, bitsize, u10, u24};

use eipscanne_rs::cip::types::{CipDint, CipDword, CipInt, CipUdint, CipUlint, CipUsint};

/// Assembly instance holding the ClearLink outputs
pub const OUTPUT_ASSEMBLY_INSTANCE: u8 = 0x70;

#[bitsize(16, new = pub)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(repr = u16)]
#[bw(map = |&x| u16::from(x))]
pub struct DigitalOutputs {
    pub output0: bool,
    pub output1: bool,
    pub output2: bool,
    pub output3: bool,
    pub output4: bool,
    pub output5: bool,
    reserved: u10,
}

// ======= Start of private IOOutputData impl ========

impl DigitalOutputs {
    fn set_digital_output(&mut self, index: usize, value: bool) {
        match index {
            0 => &self.set_output0(value),
            1 => &self.set_output1(value),
            2 => &self.set_output2(value),
            3 => &self.set_output3(value),
            4 => &self.set_output4(value),
            5 => &self.set_output5(value),
            _ => &(),
        };
    }
}

// ^^^^^^^^ End of private IOOutputData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct IOOutputData {
    pub aop_value: CipInt,
    pub dop_value: DigitalOutputs,
    pub dop_pwm: [CipUsint; 6],
    #[brw(pad_before = 2)]
    pub ccio_output_data: CipUlint,
    pub encoder_add_to_position: CipDint,
}

// ======= Start of private IOOutputData impl ========

impl IOOutputData {
    const DEFAULT_PWM_VALUE: u8 = 0;

    #[allow(dead_code)]
    fn default() -> Self {
        IOOutputData::new_digital_outputs(DigitalOutputs::default())
    }

    pub fn new_digital_outputs(digital_outputs: DigitalOutputs) -> Self {
        IOOutputData {
            aop_value: 0x0,
            dop_value: digital_outputs,
            dop_pwm: [0x0; 6],
            ccio_output_data: 0x0,
            encoder_add_to_position: 0x0,
        }
    }

    pub fn set_digital_output(&mut self, index: usize, turn_on: bool) {
        if let Some(existing_pwm_value) = self.dop_pwm.get_mut(index) {
            *existing_pwm_value = Self::DEFAULT_PWM_VALUE;
        }

        self.dop_value.set_digital_output(index, turn_on);
    }

    pub fn set_digital_pwm(&mut self, index: usize, pwm_value: u8) {
        if let Some(existing_pwm_value) = self.dop_pwm.get_mut(index) {
            *existing_pwm_value = pwm_value;
        }
    }
}

// ^^^^^^^^ End of private IOOutputData impl ^^^^^^^^

/// The command bits of one motor connector
#[bitsize(32)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct OutputRegister {
    pub enable: bool,
    pub absolute_move: bool,
    pub homing_move: bool,
    pub load_position_move: bool,
    pub load_velocity_move: bool,
    pub software_e_stop: bool,
    pub clear_alerts: bool,
    pub clear_motor_fault: bool,
    reserved: u24, // bits 8-31
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=58
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct MotorOutputData {
    move_distance: CipDint,
    velocity_limit: CipUdint,
    pub acceleration_limit: CipUdint,
    pub deceleration_limit: CipUdint,
    pub jog_velocity: CipDint,
    add_to_position: CipDint,
    pub output_register: OutputRegister,
}

// ======= Start of private MotorOutputData impl ========

impl MotorOutputData {
    #[allow(dead_code)]
    pub fn new() -> Self {
        MotorOutputData {
            move_distance: 0x0,
            velocity_limit: 0x0,
            acceleration_limit: 0x0,
            deceleration_limit: 0x0,
            jog_velocity: 0x0,
            add_to_position: 0x0,
            output_register: OutputRegister::default(),
        }
    }
}

// ^^^^^^^^ End of private MotorOutputData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct SerialAsciiOutputData {
    serial_config: CipDword,
    input_sequence_ack: CipUdint,
    output_size: CipUdint,
    output_sequence: CipUdint,
    output_data: [CipUsint; 128],
}

// ======= Start of private SerialAsciiOutputData impl ========

impl SerialAsciiOutputData {
    #[allow(dead_code)]
    pub fn new() -> Self {
        SerialAsciiOutputData {
            serial_config: 0x0,
            input_sequence_ack: 0x0,
            output_size: 0x0,
            output_sequence: 0x0,
            output_data: [0x0; 128],
        }
    }
}

// ^^^^^^^^ End of private SerialAsciiOutputData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct OutputAssemblyObject {
    pub io_output_data: IOOutputData,
    pub motor0_output_data: MotorOutputData,
    pub motor1_output_data: MotorOutputData,
    pub motor2_output_data: MotorOutputData,
    pub motor3_output_data: MotorOutputData,
    pub serial_ascii_output_data: SerialAsciiOutputData,
}

// ======= Start of OutputAssemblyObject impl ========

impl OutputAssemblyObject {
    /// The outputs of motor connector `index` (0 = M0 ... 3 = M3)
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

// ^^^^^^^^ End of OutputAssemblyObject impl ^^^^^^^^

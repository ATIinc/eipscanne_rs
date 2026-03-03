use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{Bitsized, DebugBits, FromBits, Number, bitsize, u10, u24};

use eipscanne_rs::cip::types::{CipDint, CipInt, CipUdint, CipUlint, CipUsint};

// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=31

// The clearlink turns the output light off but not the actual output if a 0 is sent. This seems like a bug. The 32+1
// value is arrived at by a mix of guess-work and experimentation. The guess work is based on the documentation
// indicating that it's actually an 11 bit DAC, so each LSB is 32767 / 2048 = 16. Experimentation shows that we somehow
// need two LSBs plus an off-by-one (another bug?) to get the output to actually go to zero-ish (measured at ~30µA).
const MINIMUM_ANALOG_OUTPUT_VALUE: CipInt = 32 + 1;

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u16, little)]
#[bw(map = |&x| u16::from(x), little)]
pub struct DigitalOutputs {
    output0: bool,
    output1: bool,
    output2: bool,
    output3: bool,
    output4: bool,
    output5: bool,
    extra_padding: u10,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct IOOutputData {
    aop_value: CipInt,
    dop_value: DigitalOutputs,
    dop_pwm: [CipUsint; 6],
    #[brw(pad_before = 2)]
    ccio_output_data: CipUlint,
    encoder_add_to_position: CipDint,
}

#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct OutputRegister {
    pub enable: bool,
    absolute_move: bool,
    pub homing_move: bool,
    load_position_move: bool,
    pub load_velocity_move: bool,
    software_e_stop: bool,
    pub clear_alerts: bool,
    pub clear_motor_fault: bool,
    // bits 8–31 are reserved
    extra_padding: u24,
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=58
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct MotorOutputData {
    move_distance: CipDint,
    velocity_limit: CipUdint,
    pub acceleration_limit: CipUdint,
    pub deceleration_limit: CipUdint,
    pub jog_velocity: CipDint,
    add_to_position: CipDint,
    pub output_register: OutputRegister,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SerialAsciiOutputData {
    serial_config: CipUdint,
    input_sequence_ack: CipUdint,
    output_size: CipUdint,
    output_sequence: CipUdint,
    output_data: [CipUsint; 128],
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=31
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct OutputAssemblyObject {
    io_output_data: IOOutputData,
    motor0_output_data: MotorOutputData,
    motor1_output_data: MotorOutputData,
    motor2_output_data: MotorOutputData,
    motor3_output_data: MotorOutputData,
    serial_ascii_output_data: SerialAsciiOutputData,
}

// ---- Implementations ----------------------------------------

impl Default for DigitalOutputs {
    fn default() -> Self {
        DigitalOutputs::new(false, false, false, false, false, false, u10::new(0x0))
    }
}

impl Default for IOOutputData {
    fn default() -> Self {
        IOOutputData {
            aop_value: MINIMUM_ANALOG_OUTPUT_VALUE,
            dop_value: DigitalOutputs::default(),
            dop_pwm: [0x0; 6],
            ccio_output_data: 0x0,
            encoder_add_to_position: 0x0,
        }
    }
}

impl OutputRegister {
    const PADDING: u24 = u24::new(0x0);
}

impl Default for OutputRegister {
    fn default() -> Self {
        OutputRegister::new(
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            OutputRegister::PADDING,
        )
    }
}

impl Default for MotorOutputData {
    fn default() -> Self {
        MotorOutputData {
            move_distance: 0,
            velocity_limit: 0,
            acceleration_limit: 0,
            deceleration_limit: 0,
            jog_velocity: 0,
            add_to_position: 0,
            output_register: OutputRegister::default(),
        }
    }
}

impl Default for SerialAsciiOutputData {
    fn default() -> Self {
        SerialAsciiOutputData {
            serial_config: 0x0,
            input_sequence_ack: 0x0,
            output_size: 0x0,
            output_sequence: 0x0,
            output_data: [0x0; 128],
        }
    }
}

impl Default for OutputAssemblyObject {
    fn default() -> Self {
        OutputAssemblyObject {
            io_output_data: IOOutputData::default(),
            motor0_output_data: MotorOutputData::default(),
            motor1_output_data: MotorOutputData::default(),
            motor2_output_data: MotorOutputData::default(),
            motor3_output_data: MotorOutputData::default(),
            serial_ascii_output_data: SerialAsciiOutputData::default(),
        }
    }
}

impl OutputAssemblyObject {
    /// Returns a mutable reference to the `MotorOutputData` for the given motor index (0–3).
    pub fn get_motor_output_mut(&mut self, index: u8) -> &mut MotorOutputData {
        match index {
            0 => &mut self.motor0_output_data,
            1 => &mut self.motor1_output_data,
            2 => &mut self.motor2_output_data,
            3 => &mut self.motor3_output_data,
            _ => panic!("Motor index must be 0-3"),
        }
    }
}

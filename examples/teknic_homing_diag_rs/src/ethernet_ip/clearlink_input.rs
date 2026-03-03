use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{Bitsized, DebugBits, FromBits, Number, bitsize, u3, u10, u12, u20};

use eipscanne_rs::cip::types::{
    CipBool, CipDint, CipInt, CipReal, CipUdint, CipUlint, CipUsint,
};

// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=30

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u16, little)]
#[bw(map = |&x| u16::from(x), little)]
pub struct DigitalInputs {
    input0: bool,
    input1: bool,
    input2: bool,
    input3: bool,
    input4: bool,
    input5: bool,
    input6: bool,
    input7: bool,
    input8: bool,
    input9: bool,
    input10: bool,
    input11: bool,
    input12: bool,
    extra_padding: u3,
}

type DigitalInputValues = DigitalInputs;
type DigitalInputStates = DigitalInputs;

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct AnalogInputValues {
    input9: CipInt,
    input10: CipInt,
    input11: CipInt,
    input12: CipInt,
}

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u16, little)]
#[bw(map = |&x| u16::from(x), little)]
pub struct AnalogInputStates {
    input9: bool,
    input10: bool,
    input11: bool,
    input12: bool,
    extra_padding: u12,
}

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u16, little)]
#[bw(map = |&x| u16::from(x), little)]
pub struct AnalogAndDigitalOutputStates {
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
#[derive(Debug, PartialEq)]
pub struct IOInputData {
    dip_value: DigitalInputValues,
    dip_status: DigitalInputStates,
    aip_value: AnalogInputValues,
    aoip_status: AnalogInputStates,
    dop_status: AnalogAndDigitalOutputStates,
    ccio_input_value: CipUlint,
    ccio_status: CipUlint,
    #[brw(pad_after = 3)]
    ccio_board_count: CipUsint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct EncoderInputData {
    encoder_position: CipDint,
    encoder_velocity: CipDint,
    encoder_index_position: CipDint,
    #[brw(pad_after = 2)]
    encoder_status: [CipBool; 2],
}

#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct MotorStatus {
    at_target_position: bool,
    steps_active: bool,
    at_target_velocity: bool,
    move_direction: bool,
    in_positive_limit: bool,
    in_negative_limit: bool,
    in_e_stop_sensor: bool,
    in_home_sensor: bool,
    is_homing: bool,
    pub motor_in_fault: bool,
    pub enabled: bool,
    at_soft_limit: bool,
    positional_move: bool,
    pub has_homed: bool,
    high_level_feedback_on: bool,
    has_torque_measurement: bool,
    pub ready_to_home: bool,
    pub shutdowns_present: bool,
    add_to_position_ack: bool,
    load_position_move_ack: bool,
    pub load_velocity_move_ack: bool,
    pub clear_motor_fault_ack: bool,
    extra_padding: u10,
}

#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u32, little)]
#[bw(map = |&x| u32::from(x), little)]
pub struct MotorShutdowns {
    command_while_shutdown: bool,
    pos_limit: bool,
    neg_limit: bool,
    sensor_e_stop: bool,
    software_e_stop: bool,
    motor_disabled: bool,
    soft_limit_exceeded: bool,
    follower_axis_fault: bool,
    command_while_following: bool,
    homing_not_ready: bool,
    motor_faulted: bool,
    following_overspeed: bool,
    extra_padding: u20,
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=53
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct MotorInputData {
    commanded_position: CipDint,
    commanded_velocity: CipDint,
    target_position: CipDint,
    target_velocity: CipDint,
    captured_position: CipDint,
    measured_torque_percentage: CipReal,
    pub motor_status: MotorStatus,
    motor_shutdowns: MotorShutdowns,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct SerialAsciiInputData {
    serial_status: CipUdint,
    output_char_count: CipUdint,
    input_char_count: CipUdint,
    output_sequence_ack: CipUdint,
    input_size: CipUdint,
    input_sequence: CipUdint,
    input_data: [CipUsint; 128],
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=30
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct InputAssemblyObject {
    io_input_data: IOInputData,
    encoder_input_data: EncoderInputData,
    motor0_input_data: MotorInputData,
    motor1_input_data: MotorInputData,
    motor2_input_data: MotorInputData,
    motor3_input_data: MotorInputData,
    serial_ascii_input_data: SerialAsciiInputData,
}

impl InputAssemblyObject {
    /// Returns the `MotorInputData` for the given motor index (0–3).
    pub fn motor_input(&self, index: u8) -> &MotorInputData {
        match index {
            0 => &self.motor0_input_data,
            1 => &self.motor1_input_data,
            2 => &self.motor2_input_data,
            3 => &self.motor3_input_data,
            _ => panic!("Motor index must be 0–3"),
        }
    }
}

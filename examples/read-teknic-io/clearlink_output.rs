use binrw::binrw; // binrw attribute

extern crate eipscanne_rs;

use eipscanne_rs::cip::types::{CipBool, CipDint, CipDword, CipInt, CipUdint, CipUlint, CipUsint};

// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=20

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct IOOutputData {
    aop_value: CipInt,
    dop_value: [CipBool; 2],
    dop_pwm: [CipUsint; 6],
    #[brw(pad_before = 2)]
    ccio_output_data: CipUlint,
    encoder_add_to_position: CipDint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MotorOutputData {
    move_distance: CipDint,
    velocity_limit: CipUdint,
    acceleration_limit: CipUdint,
    deceleration_limit: CipUdint,
    jog_velocity: CipDint,
    add_to_position: CipDint,
    output_register: CipDword,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct SerialAsciiOutputData {
    serial_config: CipDword,
    input_sequence_ack: CipUdint,
    output_size: CipUdint,
    output_sequence: CipUdint,
    output_data: [CipUsint; 128],
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct OutputAssemblyObject {
    io_output_data: IOOutputData,
    motor0_output_data: MotorOutputData,
    motor1_output_data: MotorOutputData,
    motor2_output_data: MotorOutputData,
    motor3_output_data: MotorOutputData,
    serial_ascii_output_data: SerialAsciiOutputData,
}

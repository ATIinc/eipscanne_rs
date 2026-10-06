//! The ClearLink input assembly (instance 0x64): what the device reports back.
//! https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=30
//!
//! Read only: these types are decoded from replies and never built, so none of the bitfields
//! needs a builder.

use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{DebugBits, FromBits, bitsize, u3, u10, u12, u20};

use eipscanne_rs::cip::types::{CipBool, CipDint, CipInt, CipReal, CipUdint, CipUlint, CipUsint};

/// Assembly instance holding the ClearLink inputs
pub const INPUT_ASSEMBLY_INSTANCE: u8 = 0x64;

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
pub struct DigitalInputs {
    pub input0: bool,
    pub input1: bool,
    pub input2: bool,
    pub input3: bool,
    pub input4: bool,
    pub input5: bool,
    pub input6: bool,
    pub input7: bool,
    pub input8: bool,
    pub input9: bool,
    pub input10: bool,
    pub input11: bool,
    pub input12: bool,
    reserved: u3,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct AnalogInputValues {
    pub input9: CipInt,
    pub input10: CipInt,
    pub input11: CipInt,
    pub input12: CipInt,
}

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
pub struct AnalogInputStates {
    pub input9: bool,
    pub input10: bool,
    pub input11: bool,
    pub input12: bool,
    reserved: u12,
}

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
pub struct AnalogAndDigitalOutputStates {
    pub output0: bool,
    pub output1: bool,
    pub output2: bool,
    pub output3: bool,
    pub output4: bool,
    pub output5: bool,
    reserved: u10,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct IOInputData {
    pub dip_value: DigitalInputs,
    pub dip_status: DigitalInputs,
    pub aip_value: AnalogInputValues,
    pub aoip_status: AnalogInputStates,
    pub dop_status: AnalogAndDigitalOutputStates,
    pub ccio_input_value: CipUlint,
    pub ccio_status: CipUlint,
    #[brw(pad_after = 3)]
    pub ccio_board_count: CipUsint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct EncoderInputData {
    pub encoder_position: CipDint,
    pub encoder_velocity: CipDint,
    pub encoder_index_position: CipDint,
    #[brw(pad_after = 2)]
    pub encoder_status: [CipBool; 2],
}

/// The status bits of one motor connector
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct MotorStatus {
    pub at_target_position: bool,
    pub steps_active: bool,
    pub at_target_velocity: bool,
    pub move_direction: bool,
    pub in_positive_limit: bool,
    pub in_negative_limit: bool,
    pub in_e_stop_sensor: bool,
    pub in_home_sensor: bool,
    pub is_homing: bool,
    pub motor_in_fault: bool,
    pub enabled: bool,
    pub at_soft_limit: bool,
    pub positional_move: bool,
    pub has_homed: bool,
    pub high_level_feedback_on: bool,
    pub has_torque_measurement: bool,
    pub ready_to_home: bool,
    pub shutdowns_present: bool,
    pub add_to_position_ack: bool,
    pub load_position_move_ack: bool,
    pub load_velocity_move_ack: bool,
    pub clear_motor_fault_ack: bool,
    reserved: u10,
}

/// Why a motor connector shut down
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct MotorShutdowns {
    pub command_while_shutdown: bool,
    pub pos_limit: bool,
    pub neg_limit: bool,
    pub sensor_e_stop: bool,
    pub software_e_stop: bool,
    pub motor_disabled: bool,
    pub soft_limit_exceeded: bool,
    pub follower_axis_fault: bool,
    pub command_while_following: bool,
    pub homing_not_ready: bool,
    pub motor_faulted: bool,
    pub following_overspeed: bool,
    reserved: u20,
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=53
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct MotorInputData {
    pub commanded_position: CipDint,
    pub commanded_velocity: CipDint,
    pub target_position: CipDint,
    pub target_velocity: CipDint,
    pub captured_position: CipDint,
    pub measured_torque_percentage: CipReal,
    pub motor_status: MotorStatus,
    pub motor_shutdowns: MotorShutdowns,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct SerialAsciiInputData {
    pub serial_status: CipUdint,
    pub output_char_count: CipUdint,
    pub input_char_count: CipUdint,
    pub output_sequence_ack: CipUdint,
    pub input_size: CipUdint,
    pub input_sequence: CipUdint,
    pub input_data: [CipUsint; 128],
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=30
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct InputAssemblyObject {
    pub io_input_data: IOInputData,
    pub encoder_input_data: EncoderInputData,
    motor0_input_data: MotorInputData,
    motor1_input_data: MotorInputData,
    motor2_input_data: MotorInputData,
    motor3_input_data: MotorInputData,
    pub serial_ascii_input_data: SerialAsciiInputData,
}

// ======= Start of InputAssemblyObject impl ========

impl InputAssemblyObject {
    /// The inputs of motor connector `index` (0 = M0 ... 3 = M3)
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

// ^^^^^^^^ End of InputAssemblyObject impl ^^^^^^^^

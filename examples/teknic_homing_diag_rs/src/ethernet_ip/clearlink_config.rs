use binrw::{BinRead, BinWrite, binrw};

use bilge::Bitsized;
use bilge::prelude::{DebugBits, FromBits, Number, bitsize};
use eipscanne_rs::cip::types::{CipBool, CipDint, CipDword, CipSint, CipUdint, CipUint, CipUsint};

// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=32

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum AnalogInputRange {
    #[brw(magic = 2u8)]
    ZeroToTenVolts,

    #[brw(magic = 100u8)]
    AsDigitalInput,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum AnalogOutputRange {
    #[brw(magic = 0u8)]
    FourToTwentyMilliamps,

    #[brw(magic = 2u8)]
    ZeroToTwentyMilliamps,

    #[brw(magic = 100u8)]
    AsDigitalOutput,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum PWMFrequency {
    #[brw(magic = 0u8)]
    FiveHundredHz,

    #[brw(magic = 1u8)]
    EightKiloHz,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct IOModeConfigData {
    ai0_range: AnalogInputRange,
    ai1_range: AnalogInputRange,
    ai2_range: AnalogInputRange,
    ai3_range: AnalogInputRange,
    ao0_range: AnalogOutputRange,
    dop_pwm_frequency: PWMFrequency,
    #[brw(pad_after = 1)]
    ccio_enable: CipBool,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct IOFiltersConfigData {
    aip_filters: [CipUsint; 4],
    dip_filters: [CipUint; 26],
    ccio_filters: [CipUsint; 8],
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct EncoderConfigData {
    encoder_velocity_resolution: CipUdint,
    #[brw(pad_after = 3)]
    reserved_set_byte: CipUsint,
}

#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct ConfigRegisterData {
    pub homing_enable: bool,        // bit 0: must be true for sensor-based homing
    home_sensor_active_level: bool, // bit 1
    enable_inversion: bool,         // bit 2
    pub hlfb_inversion: bool,       // bit 3: default is HIGH
    position_capture_active_level: bool, // bit 4
    pub software_limit_enable: bool, // bit 5
    _padding: [bool; 26],           // bits 6–31
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=49
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct MotorConfigData {
    pub config_register: ConfigRegisterData,
    follow_divisor: CipDint,
    follow_multiplier: CipDint,
    pub max_deceleration: CipDint,
    soft_limit_position1: CipDint,
    soft_limit_position2: CipDint,
    positive_limit_connector: CipSint,
    negative_limit_connector: CipSint,
    /// Set to the I/O pin index (0–12) for sensor-based homing, or -1 for hard-stop homing
    pub home_sensor_connector: CipSint,
    brake_output_connector: CipSint,
    stop_sensor_connector: CipSint,
    trigger_position_capture_connector: CipSint,
    #[brw(pad_after = 1)]
    follow_axis: CipSint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct SerialAsciiConfigData {
    serial_baud_rate: CipUdint,
    input_start_delimiter: CipDword,
    input_end_delimiter: CipDword,
    output_start_delimiter: CipDword,
    output_end_delimiter: CipDword,
    input_timeout: CipUdint,
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=32
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct ConfigAssemblyObject {
    io_mode_config_data: IOModeConfigData,
    io_filters_config_data: IOFiltersConfigData,
    encoder_config_data: EncoderConfigData,
    motor0_config_data: MotorConfigData,
    motor1_config_data: MotorConfigData,
    motor2_config_data: MotorConfigData,
    motor3_config_data: MotorConfigData,
    serial_ascii_config_data: SerialAsciiConfigData,
}

// ---- Implementations ----------------------------------------

impl Default for IOModeConfigData {
    fn default() -> Self {
        IOModeConfigData {
            ai0_range: AnalogInputRange::AsDigitalInput,
            ai1_range: AnalogInputRange::AsDigitalInput,
            ai2_range: AnalogInputRange::AsDigitalInput,
            ai3_range: AnalogInputRange::AsDigitalInput,
            ao0_range: AnalogOutputRange::AsDigitalOutput,
            dop_pwm_frequency: PWMFrequency::EightKiloHz,
            ccio_enable: false as CipBool,
        }
    }
}

impl Default for IOFiltersConfigData {
    fn default() -> Self {
        Self {
            aip_filters: [10; 4],
            dip_filters: [10000; 26],
            ccio_filters: [10; 8],
        }
    }
}

impl Default for EncoderConfigData {
    fn default() -> Self {
        Self {
            encoder_velocity_resolution: 100,
            reserved_set_byte: 5,
        }
    }
}

impl Default for MotorConfigData {
    fn default() -> Self {
        Self {
            config_register: ConfigRegisterData::new(false, false, false, true, false, false),
            follow_divisor: 1,
            follow_multiplier: 1,
            max_deceleration: 10000000,
            soft_limit_position1: 0,
            soft_limit_position2: 0,
            positive_limit_connector: -1,
            negative_limit_connector: -1,
            home_sensor_connector: -1,
            brake_output_connector: -1,
            stop_sensor_connector: -1,
            trigger_position_capture_connector: -1,
            follow_axis: -1,
        }
    }
}

impl Default for SerialAsciiConfigData {
    fn default() -> Self {
        Self {
            serial_baud_rate: 115200,
            input_start_delimiter: 0,
            input_end_delimiter: 0,
            output_start_delimiter: 0,
            output_end_delimiter: 0,
            input_timeout: 10,
        }
    }
}

impl Default for ConfigAssemblyObject {
    fn default() -> Self {
        Self {
            io_mode_config_data: IOModeConfigData::default(),
            io_filters_config_data: IOFiltersConfigData::default(),
            encoder_config_data: EncoderConfigData::default(),
            motor0_config_data: MotorConfigData::default(),
            motor1_config_data: MotorConfigData::default(),
            motor2_config_data: MotorConfigData::default(),
            motor3_config_data: MotorConfigData::default(),
            serial_ascii_config_data: SerialAsciiConfigData::default(),
        }
    }
}

impl ConfigAssemblyObject {
    /// Returns a mutable reference to the `MotorConfigData` for the given motor index (0–3).
    pub fn get_motor_config_mut(&mut self, index: u8) -> &mut MotorConfigData {
        match index {
            0 => &mut self.motor0_config_data,
            1 => &mut self.motor1_config_data,
            2 => &mut self.motor2_config_data,
            3 => &mut self.motor3_config_data,
            _ => panic!("Motor index must be 0-3"),
        }
    }
}

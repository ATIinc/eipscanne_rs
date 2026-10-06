//! The ClearLink configuration assembly (instance 0x96):
//! https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=32

use binrw::{BinRead, BinWrite, binrw};

use bilge::prelude::{BuilderBits, DebugBits, FromBits, bitsize, u26};
use eipscanne_rs::cip::types::{CipBool, CipDint, CipDword, CipSint, CipUdint, CipUint, CipUsint};

/// Assembly instance holding the ClearLink configuration
pub const CONFIG_ASSEMBLY_INSTANCE: u8 = 0x96;

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)] // CipUsint
pub enum AnalogInputRange {
    #[brw(magic = 2u8)]
    ZeroToTenVolts,

    #[brw(magic = 100u8)]
    AsDigitalInput,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)] // CipUsint
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
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)] // CipBool
pub enum PWMFrequency {
    #[brw(magic = 0u8)]
    FiveHundredHz,

    #[brw(magic = 1u8)]
    EightKiloHz,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
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

// ======= Start of FiltersConfig impl ========

impl IOModeConfigData {
    fn default() -> Self {
        IOModeConfigData {
            ai0_range: AnalogInputRange::AsDigitalInput,
            ai1_range: AnalogInputRange::AsDigitalInput,
            ai2_range: AnalogInputRange::AsDigitalInput,
            ai3_range: AnalogInputRange::AsDigitalInput,
            ao0_range: AnalogOutputRange::AsDigitalOutput,
            dop_pwm_frequency: PWMFrequency::FiveHundredHz,
            ccio_enable: false as CipBool,
        }
    }
}

// ^^^^^^^ End of IOFiltersConfigData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct IOFiltersConfigData {
    aip_filters: [CipUsint; 4],
    dip_filters: [CipUint; 26],
    ccio_filters: [CipUsint; 8],
}

// ======= Start of IOFiltersConfigData impl ========

impl IOFiltersConfigData {
    fn default() -> Self {
        Self {
            aip_filters: [10; 4],
            dip_filters: [10000; 26],
            ccio_filters: [10; 8],
        }
    }
}

// ^^^^^^^ End of IOFiltersConfigData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct EncoderConfigData {
    encoder_velocity_resolution: CipUdint,
    #[brw(pad_after = 3)]
    reserved_set_byte: CipUsint,
}

// ======= Start of EncoderConfigData impl ========

impl EncoderConfigData {
    fn default() -> Self {
        Self {
            encoder_velocity_resolution: 100,
            reserved_set_byte: 5,
        }
    }
}

// ^^^^^^^ End of EncoderConfigData impl ^^^^^^^^

#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct ConfigRegisterData {
    /// Must be set for sensor-based homing
    pub homing_enable: bool, // bit 0
    pub home_sensor_active_level: bool,      // bit 1
    pub enable_inversion: bool,              // bit 2
    pub hlfb_inversion: bool,                // bit 3, the device default is HIGH
    pub position_capture_active_level: bool, // bit 4
    pub software_limit_enable: bool,         // bit 5
    reserved: u26,                           // bits 6-31
}

/// Based on the ClearLink Ethernet/IP Object Reference:
/// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=49
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct MotorConfigData {
    pub config_register: ConfigRegisterData,
    follow_divisor: CipDint,
    follow_multiplier: CipDint,
    pub max_deceleration: CipDint,
    soft_limit_position1: CipDint,
    soft_limit_position2: CipDint,
    pub positive_limit_connector: CipSint,
    pub negative_limit_connector: CipSint,
    /// The I/O connector (0-12) of the home sensor, or -1 for hard-stop homing
    pub home_sensor_connector: CipSint,
    brake_output_connector: CipSint,
    stop_sensor_connector: CipSint,
    trigger_position_capture_connector: CipSint,
    #[brw(pad_after = 1)]
    follow_axis: CipSint,
}

// ======= Start of EncoderConfigData impl ========

impl MotorConfigData {
    fn default() -> Self {
        Self {
            config_register: ConfigRegisterData::builder()
                .homing_enable(false)
                .home_sensor_active_level(false)
                .enable_inversion(false)
                .hlfb_inversion(true)
                .position_capture_active_level(false)
                .software_limit_enable(false)
                .build(),
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

// ^^^^^^^ End of EncoderConfigData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
pub struct SerialAsciiConfigData {
    serial_baud_rate: CipUdint,
    input_start_delimiter: CipDword,
    input_end_delimiter: CipDword,
    output_start_delimiter: CipDword,
    output_end_delimiter: CipDword,
    input_timeout: CipUdint,
}

// ======= Start of SerialAsciiConfigData impl ========

impl SerialAsciiConfigData {
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

// ^^^^^^^ End of SerialAsciiConfigData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone)]
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

// ======= Start of ConfigAssemblyObject impl ========

impl ConfigAssemblyObject {
    pub fn default() -> Self {
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

    /// The configuration of motor connector `index` (0 = M0 ... 3 = M3)
    pub fn motor_config_mut(&mut self, index: u8) -> &mut MotorConfigData {
        match index {
            0 => &mut self.motor0_config_data,
            1 => &mut self.motor1_config_data,
            2 => &mut self.motor2_config_data,
            3 => &mut self.motor3_config_data,
            _ => panic!("motor index must be 0-3, got {index}"),
        }
    }
}

// ^^^^^^^ End of ConfigAssemblyObject impl ^^^^^^^^

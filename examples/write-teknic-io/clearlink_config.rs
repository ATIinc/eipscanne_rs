use binrw::{binrw, BinRead, BinWrite};

use bilge::prelude::{bitsize, DebugBits, FromBits, Number};
use bilge::Bitsized;
use eipscanne_rs::cip::types::{CipBool, CipDint, CipDword, CipSint, CipUdint, CipUint, CipUsint};

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[repr(u8)] // CipUsint
pub enum AnalogInputRange {
    #[brw(magic = 2u8)]
    ZeroToTenVolts,

    #[brw(magic = 100u8)]
    AsDigitalInput,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
#[repr(u8)] // CipBool
pub enum PWMFrequency {
    #[brw(magic = 0u8)]
    FiveHundredHz,

    #[brw(magic = 1u8)]
    EightKiloHz,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct ConfigRegisterData {
    homing_enable: bool,                 // = bit 0
    home_sensor_active_level: bool,      // bit = 1,
    enable_inversion: bool,              // bit = 2,
    hlfb_inversion: bool,                // bit = 3, // NOTE: The default if HIGH
    position_capture_active_level: bool, // bit = 4,
    software_limit_enable: bool,         // bit = 5,
    _padding: [bool; 26],                // bits 6-31
}
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct MotorConfigData {
    config_register: ConfigRegisterData,
    follow_divisor: CipDint,
    follow_multiplier: CipDint,
    max_deceleration: CipDint,
    soft_limit_position1: CipDint,
    soft_limit_position2: CipDint,
    positive_limit_connector: CipSint,
    negative_limit_connector: CipSint,
    home_sensor_connector: CipSint,
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

// ^^^^^^^ End of EncoderConfigData impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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
}

// ^^^^^^^ End of ConfigAssemblyObject impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use std::vec;

    use binrw::{BinRead, BinWrite};

    use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseData};
    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::message::request::MessageRouterRequest;
    use eipscanne_rs::cip::message::response::ResponseStatusCode;
    use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
    use eipscanne_rs::cip::message::data::CipDataOpt;
    use eipscanne_rs::cip::path::CipPath;
    use eipscanne_rs::cip::types::CipByte;
    use eipscanne_rs::eip::command::{
        CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
    };
    use eipscanne_rs::eip::description::{CommonPacketDescriptor, CommonPacketItemId};
    use eipscanne_rs::eip::packet::{EnIpPacketDescription, EncapsulationHeader};
    use eipscanne_rs::object_assembly::ResponseObjectAssembly;

    use crate::clearlink_config::ConfigAssemblyObject;

    #[test]
    fn test_write_clearlink_config_assembly_object() {
        /*
        EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
            Encapsulation Header
                Command: Send RR Data (0x006f)
                Length: 256
                Session Handle: 0x00000003
                Status: Success (0x00000000)
                Sender Context: 0000000000000000
                Options: 0x00000000
            Command Specific Data
                Interface Handle: CIP (0x00000000)
                Timeout: 0
                Item Count: 2
                    Type ID: Null Address Item (0x0000)
                        Length: 0
                    Type ID: Unconnected Data Item (0x00b2)
                        Length: 240
                [Response In: 10]
        Common Industrial Protocol
            Service: Set Attribute Single (Request)
            Request Path Size: 3 words
            Request Path: Assembly, Instance: 0x96, Attribute: 0x03
            Set Attribute Single (Request)
                Data: 64646464640000000a0a0a0a102710271027102710271027102710271027102710271027…



        -------------------------------------
        Hex Dump:

        0000   6f 00 00 01 03 00 00 00 00 00 00 00 00 00 00 00
        0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
        0020   00 00 00 00 b2 00 f0 00 10 03 20 04 24 96 30 03
        0030   64 64 64 64 64 00 00 00 0a 0a 0a 0a 10 27 10 27
        0040   10 27 10 27 10 27 10 27 10 27 10 27 10 27 10 27
        0050   10 27 10 27 10 27 10 27 10 27 10 27 10 27 10 27
        0060   10 27 10 27 10 27 10 27 10 27 10 27 10 27 10 27
        0070   0a 0a 0a 0a 0a 0a 0a 0a 64 00 00 00 05 00 00 00
        0080   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
        0090   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
        00a0   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
        00b0   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
        00c0   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
        00d0   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
        00e0   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
        00f0   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
        0100   00 c2 01 00 00 00 00 00 00 00 00 00 00 00 00 00
        0110   00 00 00 00 0a 00 00 00


        */
        let expected_byte_array: Vec<CipByte> = vec![
            0x6f, 0x00, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0xf0, 0x00, 0x10, 0x03,
            0x20, 0x04, 0x24, 0x96, 0x30, 0x03, 0x64, 0x64, 0x64, 0x64, 0x64, 0x00, 0x00, 0x00,
            0x0a, 0x0a, 0x0a, 0x0a, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27,
            0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27,
            0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27,
            0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27,
            0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x64, 0x00, 0x00, 0x00, 0x05, 0x00,
            0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x80, 0x96, 0x98, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x80, 0x96, 0x98, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x08, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x96, 0x98, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x96,
            0x98, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0x00, 0x00, 0xc2, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00,
        ];

        let provided_session_handle = 0x3;

        let set_clearlink_config_message = MessageRouterRequest::new_data(
            ServiceCode::SetAttributeSingle,
            CipPath::new_full(0x4, 0x96, 0x3),
            Some(Box::new(ConfigAssemblyObject::default())),
        );

        let set_clearlink_config_object = eipscanne_rs::object_assembly::RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_cip_description(
                provided_session_handle,
                0,
            ),
            cip_message: Some(set_clearlink_config_message),
        };

        // Write the object_assembly binary data to the buffer
        let mut byte_array_buffer: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

        set_clearlink_config_object.write(&mut writer).unwrap();

        // Assert equality
        assert_eq_hex!(expected_byte_array, byte_array_buffer);
    }

    #[test]
    fn test_read_clearlink_config_assembly_object_response() {
        /*
        EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
            Encapsulation Header
                Command: Send RR Data (0x006f)
                Length: 20
                Session Handle: 0x00000003
                Status: Success (0x00000000)
                Sender Context: 0000000000000000
                Options: 0x00000000
            Command Specific Data
                Interface Handle: CIP (0x00000000)
                Timeout: 0
                Item Count: 2
                    Type ID: Null Address Item (0x0000)
                        Length: 0
                    Type ID: Unconnected Data Item (0x00b2)
                        Length: 4
                [Request In: 9]
                [Time: 0.000727724 seconds]
        Common Industrial Protocol
            Service: Set Attribute Single (Response)
                1... .... = Request/Response: Response (0x1)
                .001 0000 = Service: Set Attribute Single (0x10)
            Status: Success:
                General Status: Success (0x00)
                Additional Status Size: 0 words
            [Request Path Size: 3 words]
            [Request Path: Assembly, Instance: 0x96, Attribute: 0x03]
            Set Attribute Single (Response)

        -------------------------------------
        Hex Dump:

        0000   6f 00 14 00 03 00 00 00 00 00 00 00 00 00 00 00
        0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
        0020   00 00 00 00 b2 00 04 00 90 00 00 00

        */

        let raw_bytes: Vec<CipByte> = vec![
            0x6f, 0x00, 0x14, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x04, 0x00, 0x90, 0x00,
            0x00, 0x00,
        ];

        let expected_set_config_assembly_response = ResponseObjectAssembly {
            packet_description: EnIpPacketDescription {
                header: EncapsulationHeader {
                    command: EnIpCommand::SendRrData,
                    length: Some(20),
                    session_handle: 0x3,
                    status_code: EncapsStatusCode::Success,
                    sender_context: [0x0; 8],
                    options: 0x0,
                },
                command_specific_data: CommandSpecificData::SendRrData(RRPacketData {
                    interface_handle: 0x0,
                    timeout: 0,
                    empty_data_packet: CommonPacketDescriptor {
                        type_id: CommonPacketItemId::NullAddr,
                        packet_length: Some(0),
                    },
                    unconnected_data_packet: CommonPacketDescriptor {
                        type_id: CommonPacketItemId::UnconnectedMessage,
                        packet_length: Some(4),
                    },
                }),
            },
            cip_message: Some(MessageRouterResponse {
                service_container: ServiceContainer::new(ServiceCode::SetAttributeSingle, true),
                response_data: ResponseData {
                    status: ResponseStatusCode::Success,
                    additional_status_size: 0,
                    data: CipDataOpt::Raw(vec![]),
                },
            }),
        };

        let byte_cursor = std::io::Cursor::new(raw_bytes);
        let mut buf_reader = std::io::BufReader::new(byte_cursor);

        let response_object = ResponseObjectAssembly::read(&mut buf_reader).unwrap();

        assert_eq!(expected_set_config_assembly_response, response_object);
    }
}

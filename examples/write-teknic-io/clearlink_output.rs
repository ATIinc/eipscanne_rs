use binrw::{binrw, BinRead, BinWrite};

use bilge::prelude::{bitsize, u10, Bitsized, DebugBits, FromBits, Number};

use eipscanne_rs::cip::types::{CipDint, CipDword, CipInt, CipUdint, CipUlint, CipUsint};

// https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=20

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(repr = u16)]
#[bw(map = |&x| u16::from(x))]
pub struct DigitalOutputs {
    pub output0: bool,
    pub output1: bool,
    pub output2: bool,
    pub output3: bool,
    pub output4: bool,
    pub output5: bool,
    extra_padding: u10,
}

// ======= Start of private IOOutputData impl ========

impl DigitalOutputs {
    pub fn default() -> Self {
        DigitalOutputs::new(false, false, false, false, false, false, u10::new(0x0))
    }

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
#[derive(Debug, PartialEq)]
pub struct IOOutputData {
    aop_value: CipInt,
    pub dop_value: DigitalOutputs,
    dop_pwm: [CipUsint; 6],
    #[brw(pad_before = 2)]
    ccio_output_data: CipUlint,
    encoder_add_to_position: CipDint,
}

// ======= Start of private IOOutputData impl ========

impl IOOutputData {
    const DEFAULT_PWM_VALUE: u8 = 0;

    #[allow(dead_code)]
    fn default() -> Self {
        IOOutputData::new_digital_outputs(DigitalOutputs::default())
    }

    fn new_digital_outputs(digital_outputs: DigitalOutputs) -> Self {
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

// ======= Start of private MotorOutputData impl ========

impl MotorOutputData {
    #[allow(dead_code)]
    fn new() -> Self {
        MotorOutputData {
            move_distance: 0x0,
            velocity_limit: 0x0,
            acceleration_limit: 0x0,
            deceleration_limit: 0x0,
            jog_velocity: 0x0,
            add_to_position: 0x0,
            output_register: 0x0,
        }
    }
}

// ^^^^^^^^ End of private MotorOutputData impl ^^^^^^^^

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

// ======= Start of private SerialAsciiOutputData impl ========

impl SerialAsciiOutputData {
    #[allow(dead_code)]
    fn new() -> Self {
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
#[derive(Debug, PartialEq)]
pub struct OutputAssemblyObject {
    pub io_output_data: IOOutputData,
    motor0_output_data: MotorOutputData,
    motor1_output_data: MotorOutputData,
    motor2_output_data: MotorOutputData,
    motor3_output_data: MotorOutputData,
    serial_ascii_output_data: SerialAsciiOutputData,
}

#[cfg(test)]
mod tests {
    use binrw::{BinRead, BinWrite};

    use bilge::prelude::u10;

    use eipscanne_rs::cip::message::request::RequestData;
    use pretty_assertions::assert_eq;

    use eipscanne_rs::eip::description::{CommonPacketDescriptor, CommonPacketItemId};
    use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};
    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::message::response::{ResponseData, ResponseStatusCode};
    use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
    use eipscanne_rs::cip::message::data::CipDataOpt;
    use eipscanne_rs::cip::message::{
        request::MessageRouterRequest, response::MessageRouterResponse,
    };
    use eipscanne_rs::cip::path::CipPath;
    use eipscanne_rs::cip::types::CipByte;
    use eipscanne_rs::eip::command::{
        CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
    };
    use eipscanne_rs::eip::packet::{EnIpPacketDescription, EncapsulationHeader};


    use crate::clearlink_output::{
        DigitalOutputs, IOOutputData, MotorOutputData, OutputAssemblyObject, SerialAsciiOutputData,
    };

    #[test]
    fn test_write_output_assembly_cip() {
        let expected_byte_array: Vec<CipByte> = vec![
            0x10, 0x03, 0x20, 0x04, 0x24, 0x70, 0x30, 0x03, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let set_digital_output_message = MessageRouterRequest::new_data(
            ServiceCode::SetAttributeSingle,
            CipPath::new_full(0x4, 0x70, 0x3),
            Some(Box::new(OutputAssemblyObject {
                io_output_data: IOOutputData::new_digital_outputs(DigitalOutputs::new(
                    false,
                    true,
                    false,
                    false,
                    false,
                    false,
                    u10::new(0x0),
                )),
                motor0_output_data: MotorOutputData::new(),
                motor1_output_data: MotorOutputData::new(),
                motor2_output_data: MotorOutputData::new(),
                motor3_output_data: MotorOutputData::new(),
                serial_ascii_output_data: SerialAsciiOutputData::new(),
            })),
        );

        // Write the object_assembly binary data to the buffer
        let mut byte_array_buffer: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

        set_digital_output_message.write(&mut writer).unwrap();

        // Assert equality
        assert_eq_hex!(expected_byte_array, byte_array_buffer);
    }

    #[test]
    fn test_write_output_assembly_object_request() {
        /*
        EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
            Encapsulation Header
                Command: Send RR Data (0x006f)
                Length: 300
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
                        Length: 284
                [Request In: 11]
                [Time: 0.000584448 seconds]
        Common Industrial Protocol
            Service: Get Attribute Single (Response)
                1... .... = Request/Response: Response (0x1)
                .000 1110 = Service: Get Attribute Single (0x0e)
            Status: Success:
            [Request Path Size: 3 words]
            [Request Path: Assembly, Instance: 0x70, Attribute: 0x03]
                [Path Segment: 0x20 (8-Bit Class Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 00.. = Logical Segment Type: Class ID (0)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Class: Assembly (0x04)]
                [Path Segment: 0x24 (8-Bit Instance Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 01.. = Logical Segment Type: Instance ID (1)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Instance: 0x70]
                [Path Segment: 0x30 (8-Bit Attribute Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...1 00.. = Logical Segment Type: Attribute ID (4)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Attribute: 3]
            Get Attribute Single (Response)
                Data: 000000000000000000000000000000000000000000000000000000000000000000000000…



            -------------------------------------
            Hex Dump:

            0000   6f 00 30 01 03 00 00 00 00 00 00 00 00 00 00 00
            0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
            0020   00 00 00 00 b2 00 20 01 10 03 20 04 24 70 30 03
            0030   00 00 02 00 00 00 00 00 00 00 00 00 00 00 00 00
            0040   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0050   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0060   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0070   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0080   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0090   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00a0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00b0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00c0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00d0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00e0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00f0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0100   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0110   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0120   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0130   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0140   00 00 00 00 00 00 00 00


        */
        let expected_byte_array: Vec<CipByte> = vec![
            0x6f, 0x00, 0x30, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x20, 0x01, 0x10, 0x03,
            0x20, 0x04, 0x24, 0x70, 0x30, 0x03, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let provided_session_handle = 0x3;

        let set_digital_output_message = MessageRouterRequest::new_data(
            ServiceCode::SetAttributeSingle,
            CipPath::new_full(0x4, 0x70, 0x3),
            Some(Box::new(OutputAssemblyObject {
                io_output_data: IOOutputData::new_digital_outputs(DigitalOutputs::new(
                    false,
                    true,
                    false,
                    false,
                    false,
                    false,
                    u10::new(0x0),
                )),
                motor0_output_data: MotorOutputData::new(),
                motor1_output_data: MotorOutputData::new(),
                motor2_output_data: MotorOutputData::new(),
                motor3_output_data: MotorOutputData::new(),
                serial_ascii_output_data: SerialAsciiOutputData::new(),
            })),
        );

        let set_digital_output_object = eipscanne_rs::object_assembly::RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_cip_description(
                provided_session_handle,
                0,
            ),
            cip_message: Some(set_digital_output_message),
        };

        // Write the object_assembly binary data to the buffer
        let mut byte_array_buffer: Vec<u8> = Vec::new();
        let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

        set_digital_output_object.write(&mut writer).unwrap();

        // Assert equality
        assert_eq_hex!(expected_byte_array, byte_array_buffer);
    }

    #[test]
    fn test_read_output_assembly_object_response() {
        /*
        EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
            Encapsulation Header
                Command: Send RR Data (0x006f)
                Length: 300
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
                        Length: 284
                [Request In: 11]
                [Time: 0.000584448 seconds]
        Common Industrial Protocol
            Service: Get Attribute Single (Response)
                1... .... = Request/Response: Response (0x1)
                .000 1110 = Service: Get Attribute Single (0x0e)
            Status: Success:
            [Request Path Size: 3 words]
            [Request Path: Assembly, Instance: 0x70, Attribute: 0x03]
                [Path Segment: 0x20 (8-Bit Class Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 00.. = Logical Segment Type: Class ID (0)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Class: Assembly (0x04)]
                [Path Segment: 0x24 (8-Bit Instance Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 01.. = Logical Segment Type: Instance ID (1)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Instance: 0x70]
                [Path Segment: 0x30 (8-Bit Attribute Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...1 00.. = Logical Segment Type: Attribute ID (4)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Attribute: 3]
            Get Attribute Single (Response)
                Data: 000000000000000000000000000000000000000000000000000000000000000000000000…

            -------------------------------------

            0000   6f 00 2c 01 03 00 00 00 00 00 00 00 00 00 00 00
            0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
            0020   00 00 00 00 b2 00 1c 01 8e 00 00 00 00 00 00 00
            0030   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0040   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0050   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0060   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0070   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0080   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0090   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00a0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00b0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00c0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00d0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00e0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00f0   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0100   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0110   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0120   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0130   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            0140   00 00 00 00
        */

        let raw_bytes: Vec<CipByte> = vec![
            0x6f, 0x00, 0x2c, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1c, 0x01, 0x8e, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        let expected_output_assembly_response = ResponseObjectAssembly {
            packet_description: EnIpPacketDescription {
                header: EncapsulationHeader {
                    command: EnIpCommand::SendRrData,
                    length: Some(300),
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
                        packet_length: Some(284),
                    },
                }),
            },
            cip_message: Some(MessageRouterResponse {
                service_container: ServiceContainer::new(ServiceCode::GetAttributeSingle, true),
                response_data: ResponseData {
                    status: ResponseStatusCode::Success,
                    additional_status_size: 0,
                    data: CipDataOpt::Typed(Box::new(OutputAssemblyObject {
                        io_output_data: IOOutputData::new_digital_outputs(DigitalOutputs::new(
                            false,
                            false,
                            false,
                            false,
                            false,
                            false,
                            u10::new(0x0),
                        )),
                        motor0_output_data: MotorOutputData::new(),
                        motor1_output_data: MotorOutputData::new(),
                        motor2_output_data: MotorOutputData::new(),
                        motor3_output_data: MotorOutputData::new(),
                        serial_ascii_output_data: SerialAsciiOutputData::new(),
                    })),
                },
            }),
        };

        let byte_cursor = std::io::Cursor::new(raw_bytes);
        let mut buf_reader = std::io::BufReader::new(byte_cursor);

        let response_object =
            ResponseObjectAssembly::read(&mut buf_reader).unwrap();

        assert_eq!(expected_output_assembly_response, response_object);
    }

    /// For ADAPTER testing
    #[test]
    fn test_read_output_assembly_object_raw_request() {
        let raw_byte_array: Vec<CipByte> = vec![
            0x6f, 0x00, 0x30, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x20, 0x01, 0x10, 0x03,
            0x20, 0x04, 0x24, 0x70, 0x30, 0x03, 0x00, 0x00, 0x00, // 0x02
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // Read the object_assembly binary data from the buffer
        let byte_cursor = std::io::Cursor::new(raw_byte_array);
        let mut buf_reader = std::io::BufReader::new(byte_cursor);

        let request_object = RequestObjectAssembly::read_le(&mut buf_reader).unwrap();

        let expected_output_assembly_request = RequestObjectAssembly {
            packet_description: EnIpPacketDescription {
                header: EncapsulationHeader {
                    command: EnIpCommand::SendRrData,
                    // NOTE: For some reason the serialized length is 300... But the Wireshark data said 300
                    //  Could be an internal subtraction?
                    length: Some(304),
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
                        // NOTE: For some reason the serialized length is 288... But the Wireshark data said 284
                        //  Could be an internal subtraction?
                        packet_length: Some(288),
                    },
                }),
            },
            cip_message: Some(MessageRouterRequest {
                service_container: ServiceContainer::new(ServiceCode::SetAttributeSingle, false),
                request_data: RequestData::new(
                    Some(0x3),
                    CipPath::new_full(0x4, 0x70, 0x3),
                    Some(Box::new(OutputAssemblyObject {
                        io_output_data: IOOutputData {
                            aop_value: 0x00, // 0x02
                            dop_value: DigitalOutputs::new(
                                false,
                                false,
                                false,
                                false,
                                false,
                                false,
                                u10::new(0x0),
                            ),
                            dop_pwm: [0x0; 6],
                            ccio_output_data: 0x0,
                            encoder_add_to_position: 0x0,
                        },
                        motor0_output_data: MotorOutputData::new(),
                        motor1_output_data: MotorOutputData::new(),
                        motor2_output_data: MotorOutputData::new(),
                        motor3_output_data: MotorOutputData::new(),
                        serial_ascii_output_data: SerialAsciiOutputData::new(),
                    })),
                ),
            }),
        };

        // Assert equality
        assert_eq!(request_object, expected_output_assembly_request);
        assert_eq!(
            request_object.cip_message,
            expected_output_assembly_request.cip_message
        );
    }
}

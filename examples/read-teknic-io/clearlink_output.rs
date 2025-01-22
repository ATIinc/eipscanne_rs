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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct IOOutputData {
    aop_value: CipInt,
    dop_value: DigitalOutputs,
    dop_pwm: [CipUsint; 6],
    #[brw(pad_before = 2)]
    ccio_output_data: CipUlint,
    encoder_add_to_position: CipDint,
}

// ======= Start of private IOOutputData impl ========

impl IOOutputData {
    #[allow(dead_code)]
    fn new() -> Self {
        IOOutputData::new_digital_outputs(DigitalOutputs::new(
            false,
            false,
            false,
            false,
            false,
            false,
            u10::new(0x0),
        ))
    }

    #[allow(dead_code)]
    fn new_digital_outputs(digital_outputs: DigitalOutputs) -> Self {
        IOOutputData {
            aop_value: 0x0,
            dop_value: digital_outputs,
            dop_pwm: [0x0; 6],
            ccio_output_data: 0x0,
            encoder_add_to_position: 0x0,
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
    io_output_data: IOOutputData,
    motor0_output_data: MotorOutputData,
    motor1_output_data: MotorOutputData,
    motor2_output_data: MotorOutputData,
    motor3_output_data: MotorOutputData,
    serial_ascii_output_data: SerialAsciiOutputData,
}

#[cfg(test)]
mod tests {
    use binrw::BinWrite;

    use bilge::prelude::u10;

    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::message::request::MessageRouterRequest;
    use eipscanne_rs::cip::message::shared::ServiceCode;
    use eipscanne_rs::cip::path::CipPath;
    use eipscanne_rs::cip::types::CipByte;
    use eipscanne_rs::eip::packet::EnIpPacketDescription;

    use crate::clearlink_output::{
        DigitalOutputs, IOOutputData, MotorOutputData, OutputAssemblyObject, SerialAsciiOutputData,
    };

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
                General Status: Success (0x00)
                Additional Status Size: 0 words
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
        let expected_byte_array: Vec<CipByte> = vec![
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

        let provided_session_handle = 0x3;

        let set_digital_output_message = MessageRouterRequest::new_data(
            ServiceCode::SetAttributeSingle,
            CipPath::new_full(0x4, 0x70, 0x3),
            Some(OutputAssemblyObject {
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
            }),
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
}

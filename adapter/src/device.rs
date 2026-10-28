//! What a device implements: its answers to Get_Attribute and Set_Attribute requests, and the
//! assemblies a class 1 connection exchanges.

use std::io::Cursor;

use binrw::BinRead;

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;

/// A device behind [`crate::server::serve`]; calls can overlap, and a service it leaves out is not supported
pub trait Device: Send + Sync {
    /// Get_Attributes_All
    fn handle_get_attributes_all(
        &self,
        _class: u16,
        _instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Get_Attribute_Single
    fn handle_get_attribute_single(
        &self,
        _class: u16,
        _instance: u16,
        _attribute: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Set_Attributes_All
    fn handle_set_attributes_all(
        &self,
        _class: u16,
        _instance: u16,
        _data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// Set_Attribute_Single
    fn handle_set_attribute_single(
        &self,
        _class: u16,
        _instance: u16,
        _attribute: u16,
        _data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        Err(ResponseStatusCode::ServiceNotSupported)
    }

    /// The device's implicit messaging, when it does it
    fn as_implicit_device(&self) -> Option<&dyn ImplicitDevice> {
        None
    }
}

/// The device seen by a class 1 connection: the assemblies it exchanges; calls can overlap
pub trait ImplicitDevice: Send + Sync {
    /// UDP port of the I/O
    fn io_port(&self) -> u16 {
        ETHERNET_IP_IO_UDP_PORT
    }

    /// Data size and real-time format of an assembly a connection may consume or produce; `None`
    /// refuses the Forward_Open
    fn implicit_assembly(&self, instance: u16) -> Option<(u16, RealTimeFormat)>;

    /// The outputs of an O->T packet in run mode; [`decode_request`] reads them as a type
    fn consume_outputs(&self, instance: u16, data: &[u8]);

    /// The inputs of the next T->O packet: raw bytes or a `binrw` struct
    fn produce_inputs(&self, instance: u16) -> CipDataOpt;
}

/// The data of a Set as `T`, refused with the General Status the scanner receives; mirrors the
/// scanner's `decode_reply`
pub fn decode_request<T>(data: &[u8]) -> Result<T, ResponseStatusCode>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut reader = Cursor::new(data);
    let value = T::read_le(&mut reader).map_err(|error| {
        if error.is_eof() {
            ResponseStatusCode::NotEnoughData
        } else {
            ResponseStatusCode::InvalidAttributeValue
        }
    })?;
    if reader.position() < data.len() as u64 {
        return Err(ResponseStatusCode::TooMuchData);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_request_reads_exactly_the_data() {
        assert_eq!(
            decode_request::<u32>(&[0xef, 0xbe, 0xad, 0xde]),
            Ok(0xdead_beef)
        );
    }

    #[test]
    fn decode_request_refuses_short_and_long_data() {
        assert_eq!(
            decode_request::<u32>(&[0xef, 0xbe]),
            Err(ResponseStatusCode::NotEnoughData)
        );
        assert_eq!(
            decode_request::<u32>(&[0xef, 0xbe, 0xad, 0xde, 0x00]),
            Err(ResponseStatusCode::TooMuchData)
        );
    }
}

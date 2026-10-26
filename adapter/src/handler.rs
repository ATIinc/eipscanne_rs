//! What a device implements: its answers to Get_Attribute and Set_Attribute requests.

use std::io::Cursor;

use binrw::BinRead;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;

/// A device behind [`crate::server::serve`]; calls can overlap
pub trait Handler: Send + Sync {
    /// Get_Attribute_Single when `attribute` is `Some`, Get_Attributes_All when `None`
    fn handle_get_attribute(
        &self,
        class: u16,
        instance: u16,
        attribute: Option<u16>,
    ) -> Result<CipDataOpt, ResponseStatusCode>;

    /// Set_Attribute_Single when `attribute` is `Some`, Set_Attributes_All when `None`
    fn handle_set_attribute(
        &self,
        class: u16,
        instance: u16,
        attribute: Option<u16>,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode>;
}

/// The data of a Set as `T`, refused with the General Status the scanner receives
pub fn decode<T>(data: &[u8]) -> Result<T, ResponseStatusCode>
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
    fn decode_reads_exactly_the_data() {
        assert_eq!(decode::<u32>(&[0xef, 0xbe, 0xad, 0xde]), Ok(0xdead_beef));
    }

    #[test]
    fn decode_refuses_short_and_long_data() {
        assert_eq!(
            decode::<u32>(&[0xef, 0xbe]),
            Err(ResponseStatusCode::NotEnoughData)
        );
        assert_eq!(
            decode::<u32>(&[0xef, 0xbe, 0xad, 0xde, 0x00]),
            Err(ResponseStatusCode::TooMuchData)
        );
    }
}

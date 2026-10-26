//! What a device implements: the answers to Get_Attribute and Set_Attribute requests. A refusal
//! is the General Status the scanner receives.

use std::io::Cursor;

use binrw::BinRead;

use eipscanne_rs::cip::message::data::CipData;
use eipscanne_rs::cip::message::response::ResponseStatusCode;

/// A device behind [`crate::server::serve`]. Connections are served concurrently, so calls can
/// overlap.
pub trait Handler: Send + Sync {
    /// Get_Attribute_Single when `attribute` is `Some`, Get_Attributes_All when `None`
    fn get(
        &self,
        class: u16,
        instance: u16,
        attribute: Option<u16>,
    ) -> Result<Box<dyn CipData>, ResponseStatusCode>;

    /// Set_Attribute_Single when `attribute` is `Some`, Set_Attributes_All when `None`; `data` is
    /// the request's data as it came, see [`decode`]
    fn set(
        &self,
        class: u16,
        instance: u16,
        attribute: Option<u16>,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode>;
}

/// The data of a Set decoded as `T`, which must take all of it. Too few bytes answer
/// [`ResponseStatusCode::NotEnoughData`], too many [`ResponseStatusCode::TooMuchData`] and
/// bytes `T` does not accept [`ResponseStatusCode::InvalidAttributeValue`].
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

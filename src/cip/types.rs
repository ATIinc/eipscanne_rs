// use std::fmt;
// use std::mem;

// use std::marker::PhantomData;

// use serde::de::Visitor;
// use serde::ser::SerializeTuple;
// use serde::ser::{SerializeSeq, Serializer};
use serde::{self, Deserialize, Serialize};

// This file contains the basic types used in the CIP protocol
pub type CipOctet = u8; // 8-bit value that indicates particular data type
pub type CipBool = u8; // Boolean data type
pub type CipByte = u8; // 8-bit unsigned integer
pub type CipWord = u16; // 16-bit unsigned integer
pub type CipDword = u32; // 32-bit unsigned integer
pub type CipUsint = u8; // 8-bit unsigned integer
pub type CipUint = u16; // 16-bit unsigned integer
pub type CipUdint = u32; // 32-bit unsigned integer
pub type CipSint = i8; // 8-bit signed integer
pub type CipInt = i16; // 16-bit signed integer
pub type CipDint = i32; // 32-bit signed integer
pub type CipReal = f32; // 32-bit IEEE 754 floating point
pub type CipLreal = f64; // 64-bit IEEE 754 floating point
pub type CipLword = u64; // 64-bit unsigned integer
pub type CipLint = i64; // 64-bit signed integer
pub type CipUlint = u64; // 64-bit unsigned integer

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CipRevision {
    pub major_revision: CipUsint,
    pub minor_revision: CipUsint,
}

// NOTE: Need to make sure that the length of the string is correct
#[derive(Debug, PartialEq, Serialize)]
pub struct CipShortString {
    pub length: CipUint,
    pub value: Vec<CipByte>,
}

impl CipShortString {
    pub fn new(string_val: String) -> Self {
        CipShortString {
            length: string_val.len() as CipUint,
            value: string_val.as_bytes().to_vec(),
        }
    }
}

// // Custom Deserialize implementation
// impl<'de, T> Deserialize<'de> for CipBaseString<T>
// where
//     T: Deserialize<'de>,
// {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: de::Deserializer<'de>,
//     {
//         struct CipBaseStringVisitor<T> {
//             phantom: PhantomData<T>,
//         }

//         impl<'de, T> Visitor<'de> for CipBaseStringVisitor<T>
//         where
//             T: Deserialize<'de>,
//         {
//             type Value = CipBaseString<T>;

//             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//                 formatter.write_str("a CipBaseString")
//             }

//             fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//             where
//                 A: de::SeqAccess<'de>,
//             {
//                 let length: CipUint = match seq.next_element()? {
//                     Some(val) => val,
//                     None => return Err(de::Error::invalid_length(0, &self)),
//                 };

//                 let mut value = Vec::new();
//                 while let Some(item) = seq.next_element()? {
//                     value.push(item);
//                 }

//                 // Check if the number of elements matches the length.
//                 if value.len() != length as usize {
//                     return Err(de::Error::invalid_length(value.len(), &self));
//                 }

//                 Ok(CipBaseString { length, value })
//             }
//         }

//         deserializer.deserialize_seq(CipBaseStringVisitor {
//             phantom: PhantomData,
//         })
//     }
// }

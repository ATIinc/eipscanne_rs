use std::fmt;
use std::mem;

use serde::de::{Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use bilge::prelude::{bitsize, u7, Bitsized, DebugBits, Number, TryFromBits};

use super::path::CipPath;
use super::types::CipUint;

#[bitsize(7)]
#[derive(TryFromBits, PartialEq, Debug)]
pub enum ServiceCode {
    None = 0x00,
    /* Start CIP common services */
    GetAttributeAll = 0x01,
    SetAttributeAll = 0x02,
    GetAttributeList = 0x03,
    SetAttributeList = 0x04,
    Reset = 0x05,
    Start = 0x06,
    Stop = 0x07,
    CreateObjectInstance = 0x08,
    DeleteObjectInstance = 0x09,
    MultipleServicePacket = 0x0A,
    ApplyAttributes = 0x0D,
    GetAttributeSingle = 0x0E,
    SetAttributeSingle = 0x10,
    FindNextObjectInstance = 0x11,
    ErrorResponse = 0x14, //DeviceNet only
    Restore = 0x15,
    Save = 0x16,
    GetMember = 0x18,
    NoOperation = 0x17,
    SetMember = 0x19,
    InsertMember = 0x1A,
    RemoveMember = 0x1B,
    GroupSync = 0x1C, /* End CIP common services */
}

#[bitsize(8)]
#[derive(TryFromBits, PartialEq, DebugBits)]
pub struct ServiceContainer {
    service: ServiceCode,
    // NOTE: This bit is at the front of the byte in testing
    response: bool,
}

impl Serialize for ServiceContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the u32 as little endian
        serializer.serialize_u8(self.value)
    }
}

struct ServiceContainerVisitor;

impl<'de> Visitor<'de> for ServiceContainerVisitor {
    type Value = ServiceContainer;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer between 0 and 2^32")
    }

    fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let logical_segment = ServiceContainer::try_from(value).unwrap();
        Ok(logical_segment)
    }
}

impl<'de> Deserialize<'de> for ServiceContainer {
    fn deserialize<D>(deserializer: D) -> Result<ServiceContainer, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u8(ServiceContainerVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MessageRouter {
    pub service_container: ServiceContainer,
    pub data_word_size: u8,

    // TODO: Create a generic so different types of requests can be handled
    pub data: CipPath,
}

impl MessageRouter {
    pub fn byte_size(&self) -> CipUint {
        // Creating a manual function because std::mem::size_of_val not playing nice
        let service_container_size = mem::size_of_val(&self.service_container);
        let data_word_size = mem::size_of_val(&self.data_word_size);
        let data_size = mem::size_of_val(&self.data);

        (service_container_size + data_word_size + data_size) as CipUint
    }
}

impl MessageRouter {
    pub fn new_request(service_code: ServiceCode, cip_path: CipPath) -> MessageRouter {
        let total_data_size = mem::size_of_val(&cip_path);
        let total_data_word_size = total_data_size / mem::size_of::<u16>();

        MessageRouter {
            service_container: ServiceContainer::new(service_code, false),
            data_word_size: total_data_word_size.try_into().unwrap(),
            data: cip_path,
        }
    }
}

// NOTE: A RouterResponse sets the first bit to 0x1

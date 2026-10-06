//! Class 1 I/O packets: what the two ends of an open connection exchange over UDP port 2222
//! (`ETHERNET_IP_IO_UDP_PORT`).
//!
//! How Wireshark shows such a packet and how that maps onto the Rust types:
//!
//! ```text
//! Wireshark                                       Rust
//! ----------------------------------------------  --------------------------------------------------
//! EtherNet/IP (Industrial Protocol)               IoPacket
//!     Item Count                                    (not stored: written from .items.len())
//!         Type ID: Sequenced Address Item (0x8002)  .items[0]: CommonPacketItem::SequencedAddressItem(SequencedAddress)
//!             Length                                  (not stored: always 8)
//!             Connection ID                           SequencedAddress.connection_id
//!             Encapsulation Sequence Number           SequencedAddress.encapsulation_sequence_number
//!         Type ID: Connected Data Item (0x00b1)     .items[1]: CommonPacketItem::ConnectedDataItem(..)
//!             Length                                  (not stored: size of the written data)
//! Common Industrial Protocol, I/O                 the data of the Connected Data Item (see IoData)
//! ```
//!
//! There is no encapsulation header: the packet starts with the item count. The data of the
//! Connected Data Item is read raw, because its layout depends on the connection it belongs to;
//! `IoData` decodes it once the connection is looked up by its ID.

use binrw::binrw;

use crate::cip::message::data::CipDataOpt;
use crate::cip::types::{CipUdint, CipUint};

use super::description::CommonPacketItem;

/// Length of the data carried by a Sequenced Address Item
pub const SEQUENCED_ADDRESS_LENGTH: CipUint = 8;

/// The data of a Sequenced Address Item: the connection an I/O packet belongs to and the
/// packet's number on that connection
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct SequencedAddress {
    pub connection_id: CipUdint,
    pub encapsulation_sequence_number: CipUdint,
}

/// A class 1 I/O packet: the Common Packet Format items without an encapsulation header,
/// normally exactly a Sequenced Address Item followed by a Connected Data Item
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct IoPacket {
    // Read from the wire; written from the number of items actually serialized
    #[br(temp)]
    #[bw(calc = items.len() as CipUint)]
    item_count: CipUint,

    #[br(count = item_count)]
    pub items: Vec<CommonPacketItem>,
}

// ======= Start of IoPacket impl ========

impl IoPacket {
    /// A packet on the connection `connection_id`: the Sequenced Address Item followed by the
    /// Connected Data Item carrying `data` (typed, e.g. an `IoData`, or raw)
    pub fn new(
        connection_id: CipUdint,
        encapsulation_sequence_number: CipUdint,
        data: CipDataOpt,
    ) -> Self {
        IoPacket {
            items: vec![
                CommonPacketItem::SequencedAddressItem(SequencedAddress {
                    connection_id,
                    encapsulation_sequence_number,
                }),
                CommonPacketItem::ConnectedDataItem(data),
            ],
        }
    }

    /// The Sequenced Address Item, if any
    pub fn sequenced_address(&self) -> Option<&SequencedAddress> {
        self.items.iter().find_map(|item| match item {
            CommonPacketItem::SequencedAddressItem(address) => Some(address),
            _ => None,
        })
    }

    /// The data of the Connected Data Item, if any
    pub fn connected_data(&self) -> Option<&CipDataOpt> {
        self.items.iter().find_map(|item| match item {
            CommonPacketItem::ConnectedDataItem(data) => Some(data),
            _ => None,
        })
    }
}

// ^^^^^^^^ End of IoPacket impl ^^^^^^^^

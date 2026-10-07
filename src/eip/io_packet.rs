//! Class 1 I/O packets: what the two ends of an open connection exchange over UDP port 2222
//! (`ETHERNET_IP_IO_UDP_PORT`).
//!
//! How Wireshark shows such a packet and how that maps onto the Rust types:
//!
//! ```text
//! Wireshark                                       Rust
//! ----------------------------------------------  --------------------------------------------------
//! EtherNet/IP (Industrial Protocol)               IoPacket
//!     Item Count                                    (not stored: always 2)
//!         Type ID: Sequenced Address Item (0x8002)  .sequenced_address: SequencedAddress
//!             Length                                  (not stored: always 8)
//!             Connection ID                         .sequenced_address.connection_id
//!             Encapsulation Sequence Number         .sequenced_address.encapsulation_sequence_number
//!         Type ID: Connected Data Item (0x00b1)     (not stored)
//!             Length                                  (not stored: size of the written data)
//! Common Industrial Protocol, I/O                 .connected_data (see IoData)
//! ```
//!
//! There is no encapsulation header: the packet starts with the item count. The data of the
//! Connected Data Item is read raw, because its layout depends on the connection it belongs to;
//! `IoData` decodes it once the connection is looked up by its ID.

use binrw::binrw;

use crate::cip::message::data::CipDataOpt;
use crate::cip::types::{CipUdint, CipUint};

use super::description::{CommonPacketItemId, read_item, write_item};

/// Items of an I/O packet: an address item followed by a data item
const IO_PACKET_ITEM_COUNT: CipUint = 2;
/// Length of the data carried by a Sequenced Address Item
const SEQUENCED_ADDRESS_LENGTH: CipUint = 8;

/// The data of a Sequenced Address Item: the connection an I/O packet belongs to and the
/// packet's number on that connection
#[binrw]
#[brw(little)]
#[br(import(length: CipUint), assert(
    length == SEQUENCED_ADDRESS_LENGTH,
    "a Sequenced Address Item has a Length of 8"
))]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct SequencedAddress {
    pub connection_id: CipUdint,
    pub encapsulation_sequence_number: CipUdint,
}

/// A class 1 I/O packet, sent only over UDP: a Sequenced Address Item followed by a Connected
/// Data Item, with no encapsulation header
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct IoPacket {
    #[br(temp, assert(
        item_count == IO_PACKET_ITEM_COUNT,
        "an I/O packet has exactly 2 items"
    ))]
    #[bw(calc = IO_PACKET_ITEM_COUNT)]
    item_count: CipUint,

    #[br(parse_with = read_item, args(CommonPacketItemId::SequencedAddressItem))]
    #[bw(write_with = write_item, args(CommonPacketItemId::SequencedAddressItem))]
    pub sequenced_address: SequencedAddress,

    #[br(parse_with = read_item, args(CommonPacketItemId::ConnectedTransportPacket))]
    #[bw(write_with = write_item, args(CommonPacketItemId::ConnectedTransportPacket))]
    pub connected_data: CipDataOpt,
}

// ======= Start of IoPacket impl ========

impl IoPacket {
    /// A packet on the connection `connection_id` carrying `data` (typed, e.g. an `IoData`, or raw)
    pub fn new(
        connection_id: CipUdint,
        encapsulation_sequence_number: CipUdint,
        data: CipDataOpt,
    ) -> Self {
        IoPacket {
            sequenced_address: SequencedAddress {
                connection_id,
                encapsulation_sequence_number,
            },
            connected_data: data,
        }
    }
}

// ^^^^^^^^ End of IoPacket impl ^^^^^^^^

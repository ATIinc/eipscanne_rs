//! Class 1 I/O packets: what the two ends of an open connection exchange over UDP port 2222
//! (`ETHERNET_IP_IO_UDP_PORT`).
//!
//! How Wireshark shows such a packet and how that maps onto the Rust types:
//!
//! ```text
//! Wireshark                                       Rust
//! ----------------------------------------------  --------------------------------------------------
//! EtherNet/IP (Industrial Protocol)               EnIpIoPacket
//!     Item Count                                    (not stored: always 2)
//!         Type ID: Sequenced Address Item (0x8002)  .sequenced_address_item.type_id
//!             Length                                  .sequenced_address_item.packet_length (8)
//!             Connection ID                         .sequenced_address.connection_id
//!             Encapsulation Sequence Number         .sequenced_address.encapsulation_sequence_number
//!         Type ID: Connected Data Item (0x00b1)     .connected_data_item.type_id
//!             Length                                  .connected_data_item.packet_length
//!                                                     (computed on write when None)
//! Common Industrial Protocol, I/O                 .connected_data (see CipIoData)
//! ```
//!
//! There is no encapsulation header: the packet starts with the item count. The data of the
//! Connected Data Item is read raw, because its layout depends on the connection it belongs to;
//! `CipIoData` decodes it once the connection is looked up by its ID.

use binrw::binrw;

use crate::cip::message::data::CipDataOpt;
use crate::cip::types::{CipUdint, CipUint};

use super::description::{CommonPacketDescriptor, CommonPacketItemId, serialized_length};

/// Items of an I/O packet: an address item followed by a data item
const IO_PACKET_ITEM_COUNT: CipUint = 2;
/// Length of the data carried by a Sequenced Address Item
const SEQUENCED_ADDRESS_LENGTH: CipUint = 8;

/// The data of a Sequenced Address Item: the connection an I/O packet belongs to and the
/// packet's number on that connection
#[binrw]
#[brw(little)]
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
pub struct EnIpIoPacket {
    #[br(temp, assert(
        item_count == IO_PACKET_ITEM_COUNT,
        "an I/O packet has exactly 2 items"
    ))]
    #[bw(calc = IO_PACKET_ITEM_COUNT)]
    item_count: CipUint,

    #[br(assert(
        sequenced_address_item.type_id == CommonPacketItemId::SequencedAddressItem
            && sequenced_address_item.packet_length == Some(SEQUENCED_ADDRESS_LENGTH),
        "expected a Sequenced Address Item with a Length of 8"
    ))]
    pub sequenced_address_item: CommonPacketDescriptor,

    pub sequenced_address: SequencedAddress,

    #[br(assert(
        connected_data_item.type_id == CommonPacketItemId::ConnectedTransportPacket,
        "expected a Connected Data Item"
    ))]
    #[bw(args { data_length: serialized_length(connected_data)? })]
    pub connected_data_item: CommonPacketDescriptor,

    #[br(args(connected_data_item.packet_length.unwrap_or_default()))]
    pub connected_data: CipDataOpt,
}

// ======= Start of EnIpIoPacket impl ========

impl EnIpIoPacket {
    /// A packet on the connection `connection_id` carrying `data` (typed, e.g. an `CipIoData`, or raw)
    pub fn new(
        connection_id: CipUdint,
        encapsulation_sequence_number: CipUdint,
        data: CipDataOpt,
    ) -> Self {
        EnIpIoPacket {
            sequenced_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::SequencedAddressItem,
                packet_length: Some(SEQUENCED_ADDRESS_LENGTH),
            },
            sequenced_address: SequencedAddress {
                connection_id,
                encapsulation_sequence_number,
            },
            connected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectedTransportPacket,
                packet_length: None,
            },
            connected_data: data,
        }
    }
}

// ^^^^^^^^ End of EnIpIoPacket impl ^^^^^^^^

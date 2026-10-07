//! The data of a Connected Data Item once the connection it belongs to is known.

use bilge::prelude::{BuilderBits, DebugBits, FromBits, bitsize, u2, u28};
use binrw::{BinRead, BinWrite, binrw};

use crate::cip::connection_manager::parameters::{RealTimeFormat, TransportClass, connection_size};
use crate::cip::message::data::CipDataOpt;
use crate::cip::types::CipUint;

/// 32-bit Header: the word in front of the application data of a direction whose real-time
/// format is [`RealTimeFormat::Header32Bit`].
#[bitsize(32, new = pub)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct RunIdleHeader {
    /// Run/Idle: `true` while the sender is running, so the receiver applies the data; `false`
    /// while it is idle, so the receiver ignores the data and puts its outputs in their configured
    /// idle state. Idle packets keep the connection open; only missing packets time it out.
    pub run_idle: bool,
    /// Claim Output Ownership (shown as COO by Wireshark), used by redundant-owner connections;
    /// the scanner sends 0
    pub claim_output_ownership: bool,
    /// Ready for Ownership of Outputs (shown as ROO by Wireshark), used by redundant-owner
    /// connections; the scanner sends 0
    pub ready_for_ownership_of_outputs: u2,
    reserved: u28,
}

/// The data of a Connected Data Item. What precedes the application data depends on the
/// connection, so reading takes the length of the item, the transport class and the real-time
/// format of the direction, which the receiver looks up by the connection ID of the Sequenced
/// Address Item. On the wire the sequence count comes first, then the header, then the data.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[br(import(byte_len: u16, transport_class: TransportClass, real_time_format: RealTimeFormat))]
pub struct IoData {
    /// CIP Sequence Count: carried by transport classes 1, 2 and 3. It changes only with new data
    /// (a resend keeps it), so a receiver can drop duplicates.
    #[br(if(matches!(
        transport_class,
        TransportClass::Class1 | TransportClass::Class2 | TransportClass::Class3
    )))]
    pub cip_sequence_count: Option<CipUint>,

    /// 32-bit Header: carried when the direction's real-time format is `Header32Bit`
    #[br(if(real_time_format == RealTimeFormat::Header32Bit))]
    pub run_idle_header: Option<RunIdleHeader>,

    /// The application data (an assembly), declared by the caller: whatever is left of the item
    /// after the fields above, whose size is the connection size of zero data bytes
    #[br(args(byte_len - connection_size(0, transport_class, real_time_format)))]
    pub data: CipDataOpt,
}

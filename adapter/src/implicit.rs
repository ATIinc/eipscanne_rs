//! Implicit messaging: the assemblies a device exchanges, over up to `max_io_connections` class 1
//! I/O connections.
//!
//! ```text
//! Stage              What happens on the wire                                       Module
//! -----------------  -------------------------------------------------------------  --------------
//! Open               SendRRData(Forward_Open) -> the reply, the exchange starts     message_router
//! Exchange           UDP, two independent directions:
//!      O->T            the outputs: routed by connection id, checked, decoded,       o2t
//!                      handed to the device
//!      T->O            the inputs: one packet every T->O interval                    t2o
//! Close              SendRRData(Forward_Close) -> the reply, the exchange stops     message_router
//! ```
//!
//! A Forward_Open is refused when every place is taken or another connection owns its outputs; a
//! connection whose outputs stop times out and frees its place.

use std::ops::RangeInclusive;

use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::io_data::RunOrIdle;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;

use crate::message_router::MessageRouter;

mod connection;
pub(crate) mod message_router;
mod o2t;
mod t2o;

/// A [`MessageRouter`] that also exchanges assemblies over a class 1 connection; calls can overlap
pub trait ImplicitDevice: MessageRouter {
    /// The outputs an O->T packet carries
    type Outputs: for<'a> BinRead<Args<'a> = ()>;
    /// The inputs a T->O packet carries
    type Inputs: for<'a> BinWrite<Args<'a> = ()>;

    /// Data size and real-time format of assembly `instance`; `None` when the device has no such
    /// assembly
    fn assembly_layout(&self, instance: u16) -> Option<(u16, RealTimeFormat)>;

    /// The outputs of a new O->T packet in run mode
    fn consume_outputs(&self, instance: u16, outputs: Self::Outputs);

    /// The inputs of the next T->O packet
    fn produce_inputs(&self, instance: u16) -> Self::Inputs;

    /// Idles the outputs of `instance` until the next `consume_outputs`: a switch to idle, a
    /// timeout or a Forward_Close
    fn enter_idle(&self, _instance: u16) {}

    /// The device's run or idle mode, sent in the T->O 32-bit header of `instance`
    fn read_run_or_idle(&self, _instance: u16) -> RunOrIdle {
        RunOrIdle::Run
    }
}

/// The device's class 1 connections, as its EDS declares them, and the UDP port of their I/O
pub struct ImplicitOptions {
    /// UDP port of the I/O
    pub io_port: u16,
    /// How many class 1 connections can be open at once (EDS MaxIOConnections)
    pub max_io_connections: usize,
    /// The RPIs a Forward_Open may request, in µs
    pub packet_interval: RangeInclusive<u32>,
}

impl Default for ImplicitOptions {
    fn default() -> Self {
        ImplicitOptions {
            io_port: ETHERNET_IP_IO_UDP_PORT,
            max_io_connections: 1,
            packet_interval: 1..=u32::MAX,
        }
    }
}

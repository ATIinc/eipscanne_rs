//! Implicit messaging: the assemblies a device exchanges, and one class 1 I/O connection at a
//! time.
//!
//! ```text
//! Stage              What happens on the wire                                       Module
//! -----------------  -------------------------------------------------------------  --------------
//! Open               SendRRData(Forward_Open) -> the reply, the exchange starts     message_router
//! Exchange           UDP, two independent directions:
//!      O->T            the outputs: check, decode, hand to the device                o2t
//!      T->O            the inputs: one packet every T->O interval                    t2o
//! Close              SendRRData(Forward_Close) -> the reply, the exchange stops     message_router
//! ```
//!
//! A Forward_Open while a connection is open is refused; a connection whose outputs stop times
//! out, and the next Forward_Open opens.

use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
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

    /// UDP port of the I/O
    fn io_port(&self) -> u16 {
        ETHERNET_IP_IO_UDP_PORT
    }

    /// Data size and real-time format of assembly `instance`; `None` when the device has no such
    /// assembly
    fn assembly_layout(&self, instance: u16) -> Option<(u16, RealTimeFormat)>;

    /// The outputs of a new O->T packet in run mode
    fn consume_outputs(&self, instance: u16, outputs: Self::Outputs);

    /// The inputs of the next T->O packet
    fn produce_inputs(&self, instance: u16) -> Self::Inputs;
}

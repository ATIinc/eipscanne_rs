//! Implicit messaging: one class 1 I/O connection at a time.
//!
//! ```text
//! Stage              What happens on the wire                                       Module
//! -----------------  -------------------------------------------------------------  ----------
//! Open               SendRRData(Forward_Open) -> the reply, the I/O task starts     connection
//! Exchange           UDP, two independent directions:                               connection
//!      O->T            the outputs: screen, decode, hand to the device               o2t
//!      T->O            the inputs: one packet every T->O interval                    t2o
//! Close              SendRRData(Forward_Close) -> the reply, the I/O task stops     connection
//! ```
//!
//! A Forward_Open while a connection is open is refused; a connection whose outputs stop times
//! out and frees the Connection Manager for the next one.

pub mod connection;
mod o2t;
mod t2o;

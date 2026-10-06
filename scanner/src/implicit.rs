//! Implicit messaging: a class 1 I/O connection, from the Forward_Open to the Forward_Close.
//!
//! ```text
//! Stage              What happens on the wire                                   Module
//! -----------------  ---------------------------------------------------------  -------
//! 2. Open            SendRRData(Forward_Open), read the reply -> OpenConnection  open
//! 3. Exchange        UDP 2222, two independent directions:                      udp
//!      3a. Produce     O->T: one packet every O->T interval (the outputs)        produce
//!      3b. Consume     T->O: screen, decode, watch the timeout (the inputs)      consume
//! 4. Close           SendRRData(Forward_Close), read the reply                  close
//! ```
//!
//! What the caller decides before opening is a [`ConnectionConfig`]. The Forward_Open and
//! Forward_Close are themselves unconnected messages, but they exist only to set up and tear
//! down the I/O connection, so they live here rather than in `explicit`. The sending and the
//! receiving direction of stage 3 share no state, so they are two types, [`Producer`] and
//! [`Consumer`]; neither touches the network. Only `open`, `close`, `udp` and the caller's loop
//! do (see the `implicit-io` example).

pub mod close;
pub mod config;
pub mod consume;
pub mod open;
pub mod produce;
pub mod udp;

pub use close::{CloseError, forward_close};
pub use config::{ConfigError, ConnectionConfig, DirectionConfig};
pub use consume::{Consumer, Discarded, Input};
pub use open::{OpenConnection, OpenError, forward_open};
pub use produce::{Producer, SizeError};
pub use udp::{bind_io_socket, recv_io_packet, send_io_packet};

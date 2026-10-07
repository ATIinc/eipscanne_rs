//! Implicit messaging: a class 1 I/O connection, from the Forward_Open to the Forward_Close.
//!
//! ```text
//! Stage              What happens on the wire                                   Module
//! -----------------  ---------------------------------------------------------  -------
//! 2. Open            SendRRData(Forward_Open), read the reply -> OpenConnection  open
//! 3. Exchange        UDP 2222, two independent directions:                      exchange
//!      Outputs         O->T: one packet every O->T interval
//!      Inputs          T->O: screen, decode, watch the timeout
//! 4. Close           SendRRData(Forward_Close), read the reply                  close
//! ```
//!
//! The caller builds the Forward_Open request field by field and names the real-time format of
//! each direction, the one thing both ends agree on without the wire; [`OpenConnection`] keeps
//! exactly that and the adapter's reply. The Forward_Open and Forward_Close are themselves
//! unconnected messages, but they exist only to set up and tear down the I/O connection, so they
//! live here rather than in `explicit`.
//!
//! Stage 3 keeps no state: [`output_packet`] frames the outputs, [`accept_input`] screens and
//! reads an input packet, and the caller's loop owns the sequence numbers and the deadline (see
//! the `implicit-io` example).

pub mod close;
pub mod exchange;
pub mod open;

pub use close::forward_close;
pub use exchange::{
    Discarded, FIRST_PACKET_GRACE, accept_input, bind_io_socket, input_timeout, output_packet,
    recv_io_packet, send_io_packet,
};
pub use open::{OpenConnection, forward_open};

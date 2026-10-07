//! Implicit messaging: a class 1 I/O connection, from the Forward_Open to the Forward_Close.
//!
//! ```text
//! Stage              What happens on the wire                                   Module
//! -----------------  ---------------------------------------------------------  ----------
//! 2. Open            SendRRData(Forward_Open), read the reply -> OpenConnection  connection
//! 3. Exchange        UDP 2222, two independent directions:
//!      O->T            the outputs: one packet every O->T interval               o2t
//!      T->O            the inputs: screen, decode, watch the timeout             t2o
//! 4. Close           SendRRData(Forward_Close), read the reply                  connection
//! ```
//!
//! The caller builds the Forward_Open request field by field and names the real-time format of
//! each direction, the one thing both ends agree on without the wire; `OpenConnection` keeps
//! exactly that and the adapter's reply. The Forward_Open and Forward_Close are themselves
//! unconnected messages, but they exist only to set up and tear down the I/O connection, so they
//! live here rather than in `explicit`.
//!
//! Stage 3 keeps no state: the caller's loop owns the sequence numbers and the deadline (see the
//! `implicit-io` example).

pub mod connection;
pub mod o2t;
pub mod t2o;

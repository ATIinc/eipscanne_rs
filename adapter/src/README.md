# The path of a packet

## TCP: explicit messaging

```text
server::serve
└─ server::serve_session                     one task and one session per TCP connection
   ├─ packet::read_packet
   ├─ NOP, a request with a Status           no reply
   ├─ Unregister Session                     the session ends
   ├─ Register Session
   │  └─ server::handle_register_session
   ├─ Send RR Data
   │  └─ explicit::unconnected::handle_send_rr_data
   │     └─ explicit::handle_request
   │        ├─ Get, Set
   │        │  └─ MessageRouter::handle_get_* / handle_set_*
   │        ├─ class 3 Forward_Open, Forward_Close
   │        │  └─ explicit::connected::open_close::handle_connected_forward_open / _close
   │        └─ any other Forward_Open, Forward_Close
   │           └─ MessageRouter::handle_forward_open / _close
   ├─ Send Unit Data
   │  └─ explicit::connected::send_unit_data::handle_send_unit_data
   │     ├─ consume_request                  a discarded request has no reply
   │     ├─ explicit::handle_request         as above, without class 3 Forward_Open, Forward_Close
   │     └─ reply_unit_data
   ├─ any other command
   │  └─ server::error_reply
   └─ packet::write_packet
```

## UDP: implicit messaging

```text
server::serve_implicit
├─ implicit::message_router::ImplicitMessageRouter::bind    binds the I/O socket
└─ server::serve                                            the path above

ImplicitMessageRouter                        the MessageRouter behind serve_implicit
├─ Get, Set                                  the device's
├─ handle_forward_open
│  ├─ implicit::connection::connection_from_forward_open
│  └─ the I/O exchange task
│     ├─ O->T: implicit::o2t::receive_outputs
│     │  ├─ check_output_packet, packet::decode_data, ImplicitDevice::consume_outputs
│     │  └─ ImplicitDevice::idle_outputs     on a switch to idle and on the timeout
│     └─ T->O: implicit::t2o::send_inputs
│        └─ ImplicitDevice::produce_inputs, one packet every T->O interval
└─ handle_forward_close                      stops the task, then ImplicitDevice::idle_outputs
```

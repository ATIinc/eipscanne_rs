# The path of a packet

## TCP: explicit messaging

```text
server::serve
└─ server::serve_session                     one task and one session per TCP connection
   ├─ ConnectedMessageRouter::new            the session's Message Router, below
   ├─ packet::read_packet                    none within the inactivity timeout: the session ends
   ├─ NOP, a request with a Status           no reply
   ├─ Unregister Session                     the session ends
   ├─ Register Session
   │  └─ server::handle_register_session
   ├─ Send RR Data
   │  └─ explicit::unconnected::handle_send_rr_data
   │     └─ explicit::handle_message_router_request
   │        ├─ Get, Set
   │        │  └─ MessageRouter::handle_get_* / handle_set_*
   │        └─ Forward_Open, Forward_Close
   │           └─ MessageRouter::handle_forward_open / _close
   ├─ Send Unit Data
   │  └─ explicit::connected::send_unit_data::handle_send_unit_data
   │     ├─ ConnectedMessageRouter::consume_request  a discarded request has no reply
   │     ├─ explicit::handle_message_router_request  as above
   │     └─ reply_unit_data
   ├─ any other command
   │  └─ packet::error_reply
   └─ packet::write_packet

explicit::connected::message_router::ConnectedMessageRouter    the MessageRouter of each session
├─ Get, Set                                  the server's
├─ handle_forward_open                       class 3 when the server accepts it, otherwise the server's
│  └─ explicit_connection_from_forward_open
└─ handle_forward_close                      the session's connection, otherwise the server's
```

## UDP: implicit messaging

```text
server::serve_implicit
├─ implicit::message_router::ImplicitMessageRouter::bind    binds the I/O socket
│  └─ implicit::o2t::route_outputs                          hands each O->T packet to its connection
└─ server::serve                                            the path above

ImplicitMessageRouter                        the MessageRouter behind serve_implicit
├─ Get, Set                                  the device's
├─ handle_forward_open
│  ├─ implicit::connection::connection_from_forward_open
│  └─ the I/O exchange task, one per connection
│     ├─ O->T: implicit::o2t::receive_outputs
│     │  ├─ extract_cip_io_data, packet::decode_data, ImplicitDevice::consume_outputs
│     │  └─ ImplicitDevice::enter_idle       on a switch to idle, the timeout or the close
│     └─ T->O: implicit::t2o::send_inputs
│        └─ ImplicitDevice::produce_inputs and read_run_or_idle, one packet every T->O interval
└─ handle_forward_close                      drops the sender; the task idles the outputs and ends
```

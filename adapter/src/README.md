# The path of a packet

## TCP: explicit messaging

```text
server::serve
└─ server::serve_session                     one task and one session per TCP connection
   ├─ packet::read_packet                    none within the inactivity timeout: the session ends
   ├─ NOP, a request with a Status           no reply
   ├─ Unregister Session                     the session ends
   ├─ Register Session
   │  └─ server::handle_register_session
   ├─ Send RR Data
   │  └─ explicit::unconnected::handle_send_rr_data
   │     └─ explicit::handle_request
   │        └─ Get, Set
   │           └─ MessageRouter::handle_get_* / handle_set_*
   ├─ any other command
   │  └─ packet::error_reply
   └─ packet::write_packet
```

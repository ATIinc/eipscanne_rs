# eipScanne-RS (Ethernet/IP Scanner - rust)

This repository is an implementation of the Ethernet/IP **Explicit Messaging** protocol.

This was created by using the [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) library to communicate with an Ethernet/IP Adapter while monitoring the network traffic with Wireshark.

The struct definitions/names heavily correlate to their Wireshark counterparts. See the [captures](./captures/) directory for some examples of Ethernet/IP traffic. 

See the [examples](./examples/) directory for ideas on how to implement an Ethernet/IP Explicit Messaging Scanner

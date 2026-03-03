# eipScanne-RS (Ethernet/IP Scanner - rust)

This repository is an implementation of the Ethernet/IP **Explicit Messaging** protocol.

This was created by using the [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) library to communicate with an Ethernet/IP Adapter while monitoring the network traffic with Wireshark.

The struct definitions/names heavily correlate to their Wireshark counterparts. See the [captures](./captures/) directory for some examples of Ethernet/IP traffic. 

See the [examples](./examples/) directory for ideas on how to implement an Ethernet/IP Explicit Messaging Scanner

## Development

### Dev Container

The recommended way to develop is inside the provided [Dev Container](https://containers.dev/), which gives you a ready-to-use Rust toolchain and the correct network configuration for communicating with hardware.

**Prerequisites:**
- [Docker](https://docs.docker.com/get-docker/)
- [VS Code](https://code.visualstudio.com/) with the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers), **or** the [DevContainer CLI](https://github.com/devcontainers/cli)

**Open in VS Code:**
1. Clone the repository and open it in VS Code
2. When prompted, click **Reopen in Container** — or run the command `Dev Containers: Reopen in Container` from the command palette

The container runs with `--network=host` so it can reach devices on the local network directly (required for EtherNet/IP communication). A `postCreateCommand` installs `iputils-ping` for basic network diagnostics.

Once inside the container, build and run as normal:
```sh
cargo build
cargo run --example <example-name>
```

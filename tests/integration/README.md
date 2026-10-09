# Integration Testing with OpEnEr

This integration test draws inspiration from the EIPScanner [docker-compose.yaml configuration](https://github.com/nimbuscontrols/EIPScanner/blob/master/docker-compose.yml).

The Dockerfile to built the OpENer Ethernet/IP Adapter has been updated to work again. 
* The updates follow the build instructions in the OpENer [repository](https://github.com/EIPStackGroup/OpENer).


## Running an Integration Test

### Quick start

On the host, `tests/integration/start-opener.sh` creates the network, builds the image and runs
the adapter at `172.28.0.10` in the foreground (Ctrl+C stops it). The devcontainer's host
networking reaches it through the network's bridge, so the examples run with their defaults, e.g.
`cargo run --example implicit-io`. The steps below do the same by hand.

### Creating an Ethernet/IP Test Network

1. Check that the network doesn't already exist
    * `docker network ls`
1. Create the new network if it doesn't exist
    * _NOTE_: Assign it a subnet that won't interfere with anything else
    * `docker network create eip-network -d bridge --subnet <subnet-range>`
    * i.e. `docker network create eip-network -d bridge --subnet 172.28.0.0/16`

<!-- Look into using an "ipvlan" driver instead of the default "bridge" for more control over the IP addresses -->

### Building the Ethernet/IP ADAPTER Docker image

1. Use the host computer terminal
1. Find the Dockerfile directory
    * `cd ~/src/ati/eipscanne-rs/tests/integration`
1. Build the image
    * `docker build --tag eip-adapter OpENer/.`


### Running the Ethernet/IP ADAPTER Docker container 

1. Run the newly built image using the newly created network
    * `docker run -it --network <network-name> --name <container-name> --ip <chosen-ip-addr> --publish <eip-port> <image-name>`
    * i.e. `docker run -it --network eip-network --name adapter1 --ip 172.28.0.10 --publish 44818:44818 eip-adapter`

_NOTES_:
* It's critical to define an ip address so that the Ethernet/IP Adapter can be found
* It's critical to use the defined network (or share host network) so the Ethernet/IP Adapter can be found


## Running the Ethernet/IP SCANNER Docker container

### Option 1 (easier):

**NOTE**: This devcontainer is an Ethernet/IP Scanner
* For the integration test, however, the container needs to be on the integration test network

1. Update the runArgs in the `eipscanne-rs/devcontainer/devcontainer.json` file to use the correct network

* This uses the host network (to test real connected devices)
```json
"runArgs": [
		"--network=host"
		// "--network=eip-scanner"
	]
```

* This uses the eip-testing network (to test with the OpENer mocked device)
```json
"runArgs": [
		// "--network=host"
		"--network=eip-scanner"
	]
```


### OPTION 2 (harder and usually not necessary):

**NOTE**: This devcontainer is an Ethernet/IP Scanner
* For the integration test, however, the container needs to be on the integration test network

1. Close the active VSCode devcontainer window
1. Find the appropriate docker image for the devcontainer
    * `docker image ls`
    * Should be something like: `vsc-eipscanne-rs-<uuid>-features-uid`
1. Start another container using the appropriate network
    * i.e:
        * `cd ~/src/ati/eipscanne-rs/`
        * `docker run -it --network eip-network --name eip_scanner -v .:/workspaces/eipscanne_rs --ip 172.28.0.15 vsc-eipscanne-rs-<uuid>-features-uid`
    * _NOTE_:
        * The network must be the same
        * The name will change to reflect the container as an Ethernet/IP Scanner
        * The current project workspace is mounted
        * The IP address must change
        * There is no port forwarding

1. Connect to the started container using VSCode
    * i.e. https://code.visualstudio.com/docs/devcontainers/attach-container

1. Validate that the two containers can communicate with one another
    * Install "ping"
        * `sudo apt update && apt installl iputils-ping`
    * ping the adapter ip-address
        * `ping 172.28.0.10`

1. Run the `read-identity` example, which registers a session with the Ethernet/IP adapter and then requests its identity
    * `cd /workspaces/eipscanne_rs`
    * `cargo run --example read-identity -- --host 172.28.0.10`


## Running the implicit messaging example against OpENer

The `implicit-io` example opens a class 1 connection to the OpENer sample application, exchanges
cyclic I/O with it and closes the connection again. OpENer copies the outputs it receives on
assembly 150 into the inputs it sends from assembly 100, so every input packet echoes the last
output packet.

1. Start the OpENer container (`start-opener.sh` or the steps above). Class 1 I/O travels over UDP
   port 2222 in both directions, so the scanner must reach the adapter's network (the host network
   does, through the bridge); publishing TCP 44818 alone is not enough
1. Run the example with the adapter's address; the defaults (configuration assembly 151, output
   assembly 150, input assembly 100, 32 bytes each, a 1 s packet interval, 10 cycles) match the
   OpENer sample application
    * `cargo run --example implicit-io -- --host 172.28.0.10`
    * `--rpi 100 --cycles 50` for a faster exchange, `--large` for a Large_Forward_Open
1. Expected output: the O->T and T->O connection IDs and packet intervals the adapter granted, one
   `SENT` line per cycle and one `RECEIVED` line per input packet carrying the same bytes, then
   `CLOSING the connection` and `UNREGISTERING the session`
1. Expected traffic in Wireshark: a Forward Open request and reply on TCP 44818, Connected Data
   Items on UDP 2222 in both directions (Wireshark decodes the I/O data once it has seen the
   Forward Open), a Forward Close request and reply, then Unregister Session

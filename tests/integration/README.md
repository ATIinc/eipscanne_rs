# Integration Testing with OpEnEr

This integration test draws inspiration from the EIPScanner [docker-compose.yaml configuration](https://github.com/nimbuscontrols/EIPScanner/blob/master/docker-compose.yml).

The Dockerfile to built the OpENer Ethernet/IP Adapter has been updated to work again. 
* The updates follow the build instructions in the OpENer [repository](https://github.com/EIPStackGroup/OpENer).


## Running an Integration Test

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

1. Run the eipscanne_rs executable which registers a session with the Ethernet/IP adapter and then requests it's identity
    * `cd /workspaces/eipscanne_rs`
    * `cargo run`

    * _NOTE_: The IP address of the adapter should already be hard-coded in the main.rs file
        * Feel free to update that if necessary

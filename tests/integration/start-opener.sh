#!/usr/bin/env bash
# Starts the OpENer adapter the integration tests and examples talk to, on the host (not in the
# devcontainer): creates the `eip-network` Docker network when it is missing (a fixed --ip needs a
# user-defined network), builds the `eip-adapter` image from OpENer/ and runs it in the foreground
# at 172.28.0.10. Ctrl+C stops the adapter and removes its container.
#
#   tests/integration/start-opener.sh
#
# A scanner on the host network (the devcontainer runs with --network=host) reaches the adapter
# through the network's bridge, so the examples' default --host 172.28.0.10 works as is and no
# port is published.
#
# Overrides: NETWORK, SUBNET, ADAPTER_IP, IMAGE, CONTAINER (defaults below).
set -euo pipefail

NETWORK="${NETWORK:-eip-network}"
SUBNET="${SUBNET:-172.28.0.0/16}"
ADAPTER_IP="${ADAPTER_IP:-172.28.0.10}"
IMAGE="${IMAGE:-eip-adapter}"
CONTAINER="${CONTAINER:-adapter1}"

if ! command -v docker >/dev/null 2>&1; then
    echo "docker not found; run this script on the host, not in the devcontainer" >&2
    exit 1
fi

cd "$(dirname "$0")"

if ! docker network inspect "$NETWORK" >/dev/null 2>&1; then
    docker network create "$NETWORK" --driver bridge --subnet "$SUBNET"
fi

docker build --tag "$IMAGE" OpENer/

if docker container inspect "$CONTAINER" >/dev/null 2>&1; then
    echo "A container called $CONTAINER already exists; remove it with: docker rm -f $CONTAINER" >&2
    exit 1
fi

# A terminal gets -t, so Ctrl+C reaches the adapter
terminal=()
if [ -t 0 ]; then
    terminal=(--tty)
fi
exec docker run --rm --interactive "${terminal[@]}" \
    --network "$NETWORK" --ip "$ADAPTER_IP" --name "$CONTAINER" "$IMAGE"

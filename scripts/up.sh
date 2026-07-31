#!/usr/bin/env bash
set -euo pipefail

RUNTIME="${ZTASK_CONTAINER_RUNTIME:-docker}"
NETWORK="ztask-net"
IMAGE="ztask-router:local"

if ! "$RUNTIME" network inspect "$NETWORK" >/dev/null 2>&1; then
  "$RUNTIME" network create "$NETWORK"
fi

"$RUNTIME" build -f docker/router/Dockerfile -t "$IMAGE" .

"$RUNTIME" run --rm -d \
  --name zenoh-router \
  --network "$NETWORK" \
  -p 7447:7447 \
  -v ztask-data:/data \
  "$IMAGE"

echo "Router running as 'zenoh-router' on network '$NETWORK', published on localhost:7447."
echo "Local CLI: export ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447"
echo "In-network agent containers: export ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router:7447 (--network $NETWORK)"

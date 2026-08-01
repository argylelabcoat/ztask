#!/usr/bin/env bash
set -euo pipefail

RUNTIME="${ZTASK_CONTAINER_RUNTIME:-docker}"
NETWORK="ztask-net"
ROUTER_IMAGE="ztask-router:local"
WEB_IMAGE="ztask-web:local"

if ! "$RUNTIME" network inspect "$NETWORK" >/dev/null 2>&1; then
  "$RUNTIME" network create "$NETWORK"
fi

"$RUNTIME" build -f docker/router/Dockerfile -t "$ROUTER_IMAGE" .

"$RUNTIME" run --rm -d \
  --name zenoh-router \
  --network "$NETWORK" \
  -p 7447:7447 \
  -v ztask-data:/data \
  "$ROUTER_IMAGE"

"$RUNTIME" build -f docker/web/Dockerfile -t "$WEB_IMAGE" .

"$RUNTIME" run --rm -d \
  --name ztask-web \
  --network "$NETWORK" \
  -e ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router:7447 \
  -p 8080:8080 \
  "$WEB_IMAGE"

echo "Router running as 'zenoh-router' on network '$NETWORK', published on localhost:7447."
echo "Web UI running as 'ztask-web', published on http://localhost:8080"
echo "Local CLI: export ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447"
echo "In-network agent containers: export ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router:7447 (--network $NETWORK)"

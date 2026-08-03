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

# Resolve the router's container IP rather than relying on container-name
# DNS resolution between siblings on $NETWORK: Docker's default bridge
# provides that out of the box, but Apple's `container` CLI does not
# (confirmed empirically — sibling containers get no /etc/hosts entry for
# each other and no shared resolver, so a name-based endpoint here silently
# fails every zenoh connect/put/get with no error surfaced to the client).
# Resolving the IP explicitly works under both runtimes.
if [ "$RUNTIME" = "docker" ]; then
  ROUTER_IP="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' zenoh-router)"
else
  if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq is required to resolve the router's IP under '$RUNTIME'" >&2
    exit 1
  fi
  ROUTER_IP="$("$RUNTIME" inspect zenoh-router | jq -r '.[0].status.networks[0].ipv4Address' | cut -d/ -f1)"
fi

"$RUNTIME" build -f docker/web/Dockerfile -t "$WEB_IMAGE" .

"$RUNTIME" run --rm -d \
  --name ztask-web \
  --network "$NETWORK" \
  -e ZTASK_ZENOH_ENDPOINT="tcp/${ROUTER_IP}:7447" \
  -p 8080:8080 \
  "$WEB_IMAGE"

echo "Router running as 'zenoh-router' on network '$NETWORK', published on localhost:7447 (in-network IP: $ROUTER_IP)."
echo "Web UI running as 'ztask-web', published on http://localhost:8080"
echo "Local CLI: export ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447"
echo "In-network agent containers: export ZTASK_ZENOH_ENDPOINT=tcp/${ROUTER_IP}:7447 (--network $NETWORK)"
echo "  (the router's container name may not resolve via DNS depending on your container runtime; the IP above always works)"

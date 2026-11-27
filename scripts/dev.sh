#!/bin/sh
# Run Cargo on the host when GTK is installed there, otherwise inside Toolbox.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONTAINER=${LUMEN_TOOLBOX:-yeelight-dev}

usage() {
  cat <<EOF
Usage: scripts/dev.sh [run|test|check|fmt|bootstrap] [cargo arguments...]

  (no command)   Build and launch Lumen
  run            Same as above
  test           Run the workspace tests
  check          Compile without launching
  fmt            Format the Rust code (pass --check to verify only)
  bootstrap      Build the Toolbox image and replace the container

If pkg-config can see gtk4 and libadwaita-1, Cargo runs on the host.
Otherwise Cargo runs in the Toolbox container named by LUMEN_TOOLBOX
(default: yeelight-dev).
EOF
}

require_toolbox() {
  if ! command -v toolbox >/dev/null 2>&1 || ! command -v podman >/dev/null 2>&1; then
    echo "error: toolbox and podman are required." >&2
    exit 127
  fi
}

bootstrap_container() {
  require_toolbox
  podman build \
    -t localhost/yeelight-dev:latest \
    -f "${ROOT}/build-aux/yeelight-dev.Containerfile" \
    "$ROOT"
  if podman container exists "$CONTAINER"; then
    toolbox rm --force "$CONTAINER"
  fi
  toolbox create \
    --image localhost/yeelight-dev:latest \
    --container "$CONTAINER"
}

if ! command -v pkg-config >/dev/null 2>&1 \
  || ! pkg-config --exists gtk4 \
  || ! pkg-config --exists libadwaita-1; then
  host_gtk=0
else
  host_gtk=1
fi

cd "$ROOT"

cmd=run
if [ "$#" -gt 0 ]; then
  cmd=$1
  shift
fi

case "$cmd" in
  run)
    set -- run -p lumen "$@"
    ;;
  test)
    set -- test --workspace "$@"
    ;;
  check)
    set -- check --workspace "$@"
    ;;
  fmt)
    set -- fmt --all "$@"
    ;;
  bootstrap)
    bootstrap_container
    exit 0
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    echo "error: unknown command '${cmd}'" >&2
    usage >&2
    exit 2
    ;;
esac

if [ "$host_gtk" -eq 1 ]; then
  exec cargo "$@"
fi

if ! command -v toolbox >/dev/null 2>&1 || ! command -v podman >/dev/null 2>&1; then
  echo "error: gtk4 and libadwaita-1 are not visible to pkg-config, and Toolbox is not available." >&2
  echo "Install those libraries on this system, or install toolbox and podman and create the '${CONTAINER}' container." >&2
  exit 127
fi

if ! podman container exists "$CONTAINER"; then
  echo "error: gtk4 and libadwaita-1 are not visible to pkg-config, and Toolbox container '${CONTAINER}' does not exist." >&2
  echo "Set LUMEN_TOOLBOX to an existing container, or create this one with: ./scripts/dev.sh bootstrap" >&2
  exit 1
fi

exec toolbox run -c "$CONTAINER" \
  env "CARGO_TARGET_DIR=${ROOT}/target/yeelight-dev" \
  cargo "$@"

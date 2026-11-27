#!/bin/sh
# Run Cargo on the host when GTK is installed there, otherwise inside Toolbox.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONTAINER=${LUMEN_TOOLBOX:-yeelight-dev}

usage() {
  cat <<EOF
Usage: scripts/dev.sh [run|test|check|fmt|sim|bootstrap] [cargo arguments...]

  (no command)   Build and launch Lumen
  run            Same as above
  test           Run the workspace tests
  check          Compile without launching
  fmt            Format the Rust code (pass --check to verify only)
  sim            Run only the Yeelight simulator
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

simulator_running() {
  mode=$1
  if [ "$mode" = host ]; then
    if pgrep -f '[y]eelight-sim' >/dev/null 2>&1; then
      return 0
    fi
    return 1
  fi
  if toolbox run -c "$CONTAINER" sh -c 'pgrep -f "[y]eelight-sim" >/dev/null 2>&1'; then
    return 0
  fi
  return 1
}

maybe_start_simulator() {
  mode=$1
  started_sim=0
  if simulator_running "$mode"; then
    echo "Yeelight simulator is already running." >&2
    return
  fi
  echo "Yeelight simulator is not running, so remembered lights refuse the connection." >&2
  echo "Starting it in the background. It stops when Lumen exits." >&2
  echo "To run it in its own terminal: ./scripts/dev.sh sim" >&2
  sim_log="${ROOT}/target/yeelight-dev/simulator.log"
  mkdir -p "${ROOT}/target/yeelight-dev"
  : > "$sim_log"
  if [ "$mode" = host ]; then
    cargo run -p yeelight-sim >"$sim_log" 2>&1 &
  else
    toolbox run -c "$CONTAINER" \
      env "CARGO_TARGET_DIR=${ROOT}/target/yeelight-dev" \
      cargo run -p yeelight-sim >"$sim_log" 2>&1 &
  fi
  sim_pid=$!
  started_sim=1
  tries=0
  while [ "$tries" -lt 300 ]; do
    if grep -q "Press Ctrl-C to stop." "$sim_log" 2>/dev/null; then
      echo "Yeelight simulator is ready." >&2
      return
    fi
    if ! kill -0 "$sim_pid" 2>/dev/null; then
      echo "error: the simulator exited before it was ready. See ${sim_log}" >&2
      exit 1
    fi
    tries=$((tries + 1))
    sleep 0.2
  done
  echo "error: the simulator did not become ready. See ${sim_log}" >&2
  exit 1
}

stop_simulator() {
  if [ "${started_sim:-0}" -eq 1 ] && [ -n "${sim_pid:-}" ]; then
    kill "$sim_pid" 2>/dev/null || true
  fi
}

report_fmt() {
  status=$1
  if [ "$cmd" != fmt ]; then
    return 0
  fi
  if [ "$status" -eq 0 ]; then
    if [ "$fmt_check" -eq 1 ]; then
      echo "Rust formatting is up to date."
    else
      echo "Formatted the Rust code."
    fi
    return 0
  fi
  if [ "$fmt_check" -eq 1 ]; then
    echo "error: Rust formatting differs. Run ./scripts/dev.sh fmt to fix it." >&2
  fi
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
fmt_check=0
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
    for arg in "$@"; do
      if [ "$arg" = --check ]; then
        fmt_check=1
      fi
    done
    set -- fmt --all "$@"
    ;;
  sim)
    set -- run -p yeelight-sim "$@"
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
  if [ "$cmd" = run ]; then
    maybe_start_simulator host
  fi
  status=0
  cargo "$@" || status=$?
  report_fmt "$status"
  stop_simulator
  exit "$status"
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

if [ "$cmd" = run ]; then
  maybe_start_simulator toolbox
fi

status=0
toolbox run -c "$CONTAINER" \
  env "CARGO_TARGET_DIR=${ROOT}/target/yeelight-dev" \
  cargo "$@" || status=$?
report_fmt "$status"
stop_simulator
exit "$status"

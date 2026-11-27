#!/bin/sh
# Run Cargo on the host when GTK is installed there, otherwise inside Toolbox.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONTAINER=${LUMEN_TOOLBOX:-yeelight-dev}

usage() {
  cat <<EOF
Usage: scripts/dev.sh [run|test|check|fmt|sim|status|stop|bootstrap] [cargo arguments...]

  (no command)   Build and launch Lumen
  run            Same as above
  test           Run the workspace tests
  check          Compile without launching
  fmt            Format the Rust code (pass --check to verify only)
  sim            Run only the Yeelight simulator
  status         Show whether the simulator and dev Lumen are running
  stop           Stop a Yeelight simulator left running
  bootstrap      Build the Toolbox image and replace the container

If pkg-config can see gtk4 and libadwaita-1, Cargo runs on the host.
Otherwise Cargo runs in the Toolbox container named by LUMEN_TOOLBOX
(default: yeelight-dev).

A simulator left running is found by the installed app on refresh.
./scripts/dev.sh status shows it, and ./scripts/dev.sh stop ends it.
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

say_environment() {
  if [ "$host_gtk" -eq 1 ]; then
    echo "Using the host." >&2
  else
    echo "Using Toolbox container ${CONTAINER}." >&2
  fi
}

sim_pids() {
  pgrep -f '[y]eelight-sim' || true
}

lumen_pids() {
  {
    pgrep -f '[c]argo run -p lumen' || true
    pgrep -f '[t]arget/yeelight-dev/debug/lumen' || true
    pgrep -f '[t]arget/debug/lumen' || true
  } | awk 'NF && !seen[$0]++'
}

short_process() {
  pid=$1
  args=$(ps -p "$pid" -o args= 2>/dev/null || true)
  case "$args" in
    *'cargo run -p yeelight-sim'*)
      echo "pid ${pid}  cargo run -p yeelight-sim"
      ;;
    *'cargo run -p lumen'*)
      echo "pid ${pid}  cargo run -p lumen"
      ;;
    *yeelight-sim*)
      echo "pid ${pid}  yeelight-sim"
      ;;
    */lumen|*/lumen\ *)
      echo "pid ${pid}  lumen"
      ;;
    *)
      echo "pid ${pid}  ${args}"
      ;;
  esac
}

simulator_running() {
  if [ -n "$(sim_pids)" ]; then
    return 0
  fi
  return 1
}

print_simulator_bulbs() {
  awk '
    /^discovery udp / { show = 1 }
    show && !/^Press Ctrl-C to stop\./ { print "  " $0 }
  ' "$sim_log" >&2
}

maybe_start_simulator() {
  started_sim=0
  if simulator_running; then
    echo "A Yeelight simulator is already running, so this session will not stop it when Lumen exits." >&2
    echo "Refresh will keep finding its bulbs. Stop it with ./scripts/dev.sh stop" >&2
    return
  fi
  echo "Starting the Yeelight simulator. It answers discovery for every Lumen on this machine, including the installed Flatpak, until this script exits." >&2
  sim_log="${ROOT}/target/yeelight-dev/simulator.log"
  mkdir -p "${ROOT}/target/yeelight-dev"
  : > "$sim_log"
  if [ "$1" = host ]; then
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
      print_simulator_bulbs
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
  if [ "${started_sim:-0}" -ne 1 ]; then
    return 0
  fi
  started_sim=0
  if [ -n "${sim_pid:-}" ]; then
    kill "$sim_pid" 2>/dev/null || true
    pkill -P "$sim_pid" 2>/dev/null || true
  fi
  # The Toolbox binary can outlive the shell that launched it.
  pkill -f '[y]eelight-sim' 2>/dev/null || true
  left=$(sim_pids | tr '\n' ' ')
  if [ -n "$left" ]; then
    # shellcheck disable=SC2086
    kill -9 $left 2>/dev/null || true
    left=$(sim_pids | tr '\n' ' ')
  fi
  if [ -n "$left" ]; then
    echo "Yeelight simulator is still running (pid ${left}). Stop it with ./scripts/dev.sh stop" >&2
  else
    echo "Stopped the Yeelight simulator." >&2
  fi
}

stop_yeelight_sim() {
  say_environment
  pids=$(sim_pids | tr '\n' ' ')
  if [ -z "$pids" ]; then
    echo "Yeelight simulator is not running." >&2
    return 0
  fi
  echo "Stopping the Yeelight simulator." >&2
  for pid in $pids; do
    short_process "$pid" >&2
    kill "$pid" 2>/dev/null || true
  done
  pkill -f '[y]eelight-sim' 2>/dev/null || true
  left=$(sim_pids | tr '\n' ' ')
  if [ -n "$left" ]; then
    # shellcheck disable=SC2086
    kill -9 $left 2>/dev/null || true
    left=$(sim_pids | tr '\n' ' ')
  fi
  if [ -n "$left" ]; then
    echo "Yeelight simulator is still running (pid ${left})." >&2
    return 1
  fi
  echo "Stopped the Yeelight simulator." >&2
}

show_status() {
  say_environment
  pids=$(sim_pids)
  if [ -z "$pids" ]; then
    echo "Yeelight simulator: not running" >&2
  else
    echo "Yeelight simulator: running" >&2
    for pid in $pids; do
      short_process "$pid" >&2
    done
    echo "A running simulator is found by the installed app on refresh. Stop it with ./scripts/dev.sh stop" >&2
  fi
  pids=$(lumen_pids)
  if [ -z "$pids" ]; then
    echo "Lumen dev process: not running" >&2
  else
    echo "Lumen dev process: running" >&2
    for pid in $pids; do
      short_process "$pid" >&2
    done
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

say_sim_stopped() {
  if [ "${sim_stopped_said:-0}" -eq 1 ]; then
    return 0
  fi
  sim_stopped_said=1
  echo "Yeelight simulator stopped." >&2
}

on_signal() {
  stop_simulator
  if [ "${cmd:-}" = sim ]; then
    say_sim_stopped
  fi
  exit 130
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
  status)
    show_status
    exit 0
    ;;
  stop)
    stop_yeelight_sim
    exit 0
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

trap stop_simulator EXIT
trap on_signal INT HUP TERM

say_environment

if [ "$host_gtk" -eq 1 ]; then
  if [ "$cmd" = run ]; then
    maybe_start_simulator host
  elif [ "$cmd" = sim ]; then
    echo "Running the Yeelight simulator in the foreground. The installed Lumen will see these bulbs until you press Ctrl-C." >&2
  fi
  status=0
  if [ "$cmd" = test ]; then
    python3 "$ROOT/scripts/test-report.py" cargo "$@" || status=$?
  else
    cargo "$@" || status=$?
  fi
  report_fmt "$status"
  if [ "$cmd" = sim ]; then
    say_sim_stopped
  fi
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
elif [ "$cmd" = sim ]; then
  echo "Running the Yeelight simulator in the foreground. The installed Lumen will see these bulbs until you press Ctrl-C." >&2
fi

status=0
if [ "$cmd" = test ]; then
  python3 "$ROOT/scripts/test-report.py" \
    toolbox run -c "$CONTAINER" \
    env "CARGO_TARGET_DIR=${ROOT}/target/yeelight-dev" \
    cargo "$@" || status=$?
else
  toolbox run -c "$CONTAINER" \
    env "CARGO_TARGET_DIR=${ROOT}/target/yeelight-dev" \
    cargo "$@" || status=$?
fi
report_fmt "$status"
if [ "$cmd" = sim ]; then
  say_sim_stopped
fi
stop_simulator
exit "$status"

#!/usr/bin/env bash
# SPIKE (v0.3.0 Track D, #696/#683): one private Sail service for a run.
#
#   scripts/spike-v030-bugs-env.sh ROOT start     fresh cache, engine, `sail serve`
#   scripts/spike-v030-bugs-env.sh ROOT stop
#   scripts/spike-v030-bugs-env.sh ROOT env       print the exports for other commands
#
# ROOT must be short: it holds the daemon sockets. SAIL_SPIKE_BIN names the
# directory with `sail` and its shim (default: this checkout's target/release).
set -euo pipefail
root="${1:?ROOT}"; action="${2:?start|stop|env}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${SAIL_SPIKE_BIN:-$here/target/release}"
exports() {
  cat <<EOF
export SAIL_CACHE_DIR=$root/cache
export SAIL_DAEMON_SOCKET=$root/d.sock
export CARGO_HOME=$root/cargo-home
export SAIL=$bin/sail
EOF
}
case "$action" in
  env) exports ;;
  start)
    mkdir -p "$root/cache" "$root/cargo-home" "$root/work"
    eval "$(exports)"
    "$SAIL" engine status >/dev/null 2>&1 || "$SAIL" engine install >"$root/engine-install.log" 2>&1
    "$SAIL" engine status | head -5
    setsid nohup "$SAIL" serve --socket "$SAIL_DAEMON_SOCKET" --cache "$SAIL_CACHE_DIR" \
      >"$root/daemon.stdout" 2>"$root/daemon.stderr" </dev/null &
    echo $! >"$root/daemon.pid"
    for _ in $(seq 1 100); do
      "$SAIL" status --socket "$SAIL_DAEMON_SOCKET" >/dev/null 2>&1 && break
      sleep 0.2
    done
    "$SAIL" status --socket "$SAIL_DAEMON_SOCKET" | head -5
    ;;
  stop)
    [ -f "$root/daemon.pid" ] && kill "$(cat "$root/daemon.pid")" 2>/dev/null || true
    rm -f "$root/daemon.pid"
    ;;
esac

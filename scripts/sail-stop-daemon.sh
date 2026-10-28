#!/usr/bin/env bash
# Stop the Sail daemon that serves the current environment's cache.
#
#   scripts/sail-stop-daemon.sh
#
# Uses the `sail` on PATH with the caller's SAIL_CACHE_DIR and
# SAIL_DAEMON_SOCKET, one of which must be set. `sail status --json` reports
# the daemon's actual socket, which is not always under SAIL_CACHE_DIR (a cache path too long for a Unix
# socket gets a short socket under the temporary directory instead), and the
# daemon's pid. The script signals that pid, then any `sail serve` or
# `sail _serve-jobs` process still bound to that exact socket or to any socket
# inside SAIL_CACHE_DIR (a daemon of another protocol version, such as a
# baseline binary's, names its socket differently), and waits for them to
# exit. It is a no-op when no daemon is running. Benchmark scripts use
# it so a daemon never outlives the cache it served.
set -uo pipefail

# Never fall back to the default cache's daemon, which may serve other work.
if [ -z "${SAIL_CACHE_DIR:-}" ] && [ -z "${SAIL_DAEMON_SOCKET:-}" ]; then
  echo "sail-stop-daemon: set SAIL_CACHE_DIR or SAIL_DAEMON_SOCKET" >&2
  exit 2
fi

status="$(sail status --json 2>/dev/null)" || true
socket="$(printf '%s\n' "$status" | sed -n 's/^  "socket": "\(.*\)",\{0,1\}$/\1/p' | head -n 1)"
pid="$(printf '%s\n' "$status" | sed -n 's/^    "pid": \([0-9][0-9]*\),\{0,1\}$/\1/p' | head -n 1)"

# escape TEXT: TEXT as a literal extended regular expression.
escape() { printf '%s' "$1" | sed 's/[][\\.^$*+?(){}|]/\\&/g'; }

pids=()
[ -z "$pid" ] || pids+=("$pid")
patterns=()
# Job servers started beside a running cache daemon carry the same socket.
[ -z "$socket" ] || patterns+=("--socket $(escape "$socket")( |\$)")
# Daemons with a socket under the cache itself, whatever its version.
cache="${SAIL_CACHE_DIR:-}"
[ -z "$cache" ] || patterns+=("--socket $(escape "${cache%/}")/[^ /]+\.sock( |\$)")
for pattern in ${patterns[@]+"${patterns[@]}"}; do
  while read -r other; do
    [ -n "$other" ] && pids+=("$other")
  done < <(pgrep -f -- "$pattern" 2>/dev/null || true)
done
[ "${#pids[@]}" -gt 0 ] || exit 0

kill "${pids[@]}" 2>/dev/null || true
for ((attempt = 0; attempt < 100; attempt++)); do
  alive=0
  for p in "${pids[@]}"; do
    if kill -0 "$p" 2>/dev/null; then alive=1; fi
  done
  [ "$alive" = 1 ] || exit 0
  sleep 0.05
done
kill -KILL "${pids[@]}" 2>/dev/null || true
exit 0

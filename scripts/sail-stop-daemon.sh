#!/usr/bin/env bash
# Stop the Sail daemon that serves the current environment's cache.
#
#   scripts/sail-stop-daemon.sh
#
# Uses the `sail` on PATH with the caller's SAIL_CACHE_DIR and
# SAIL_DAEMON_SOCKET. `sail status --json` reports the daemon's actual socket,
# which is not always under SAIL_CACHE_DIR (a cache path too long for a Unix
# socket gets a short socket under the temporary directory instead), and the
# daemon's pid. The script signals that pid, then any `sail serve` or
# `sail _serve-jobs` process still bound to that exact socket, and waits for
# them to exit. It is a no-op when no daemon is running. Benchmark scripts use
# it so a daemon never outlives the cache it served.
set -uo pipefail

status="$(sail status --json 2>/dev/null)" || true
socket="$(printf '%s\n' "$status" | sed -n 's/^  "socket": "\(.*\)",\{0,1\}$/\1/p' | head -n 1)"
pid="$(printf '%s\n' "$status" | sed -n 's/^    "pid": \([0-9][0-9]*\),\{0,1\}$/\1/p' | head -n 1)"
[ -n "$socket" ] || exit 0

pids=()
[ -z "$pid" ] || pids+=("$pid")
# Job servers started beside a running cache daemon carry the same socket.
pattern="--socket $(printf '%s' "$socket" | sed 's/[][\\.^$*+?(){}|]/\\&/g')( |\$)"
while read -r other; do
  [ -n "$other" ] && pids+=("$other")
done < <(pgrep -f -- "$pattern" 2>/dev/null || true)
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

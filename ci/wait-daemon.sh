#!/usr/bin/env bash
# Bounded socket readiness, including slow local amd64 emulation.
set -euo pipefail
sail_wait_socket="${1:?socket required}"
sail_wait_pid="${2:?daemon pid required}"
sail_wait_deadline=$((SECONDS + 60))
while true; do
  if ! kill -0 "$sail_wait_pid" 2>/dev/null; then
    echo "daemon exited before its socket became ready" >&2
    exit 1
  fi
  [ ! -S "$sail_wait_socket" ] || exit 0
  if [ "$SECONDS" -ge "$sail_wait_deadline" ]; then
    echo "daemon socket did not become ready within 60 seconds" >&2
    exit 1
  fi
  sleep 0.1
done

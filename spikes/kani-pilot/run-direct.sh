#!/usr/bin/env bash
# Direct `cargo kani`, one harness per process, with wall/CPU/peak-RSS from
# GNU time. Writes OUT/<round>/<harness>.{log,time,exit}.
#
#   spikes/kani-pilot/run-direct.sh PILOT_DIR OUT ROUND HARNESS... [-- KANI_ARGS...]
set -uo pipefail
export RUSTC_WRAPPER=
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
pilot=${1:?pilot dir}; out=${2:?out dir}; round=${3:?round}; shift 3
harnesses=()
while [ $# -gt 0 ] && [ "$1" != -- ]; do harnesses+=("$1"); shift; done
[ "${1:-}" = -- ] && shift
mkdir -p "$out/$round"
cd "$pilot" || exit 1
for h in "${harnesses[@]}"; do
  /usr/bin/time -o "$out/$round/$h.time" \
    -f 'wall_s=%e\nuser_s=%U\nsys_s=%S\nmax_rss_kb=%M\nexit=%x' \
    cargo kani --harness "$h" --exact "$@" >"$out/$round/$h.log" 2>&1
  echo $? >"$out/$round/$h.exit"
  printf '%s exit=%s %s\n' "$h" "$(cat "$out/$round/$h.exit")" \
    "$(grep -E '^VERIFICATION|^Verification Time' "$out/$round/$h.log" | tr '\n' ' ')"
done

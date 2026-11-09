#!/usr/bin/env bash
# Prototype sail path against direct Kani: cold (all misses), warm (all hits),
# then the invalidation and never-cache cases. Writes WORK/proto/*.json.
#
#   spikes/kani-pilot/run-proto.sh WORK
set -uo pipefail
export RUSTC_WRAPPER=
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=${1:?work dir}
out="$work/proto"; cache="$work/proto-cache"
rm -rf "$out" "$cache"; mkdir -p "$out"
names="p1_push_pop_roundtrip p2_try_push_full_is_err p3_insert_remove_inverse p4_truncate_keeps_prefix p5_retain_filters p6_array_string_push_char f1_push_never_panics f2_drain_any_range_never_panics e1_retain_unwind_too_small"
hs=(); for n in $names; do hs+=(--harness "harnesses::$n"); done
run() { # NAME ROOT ARGS...
  local name=$1 root=$2; shift 2
  /usr/bin/time -o "$out/$name.time" -f 'wall_s=%e user_s=%U sys_s=%S max_rss_kb=%M' \
    "$here/sail_kani_proto.py" --root "$root" --package pilot --cache "$cache" "$@" \
    >"$out/$name.json" 2>"$out/$name.err"
  echo "$name: $(tr '\n' ' ' <"$out/$name.time") $(python3 -I -c '
import json,sys
d=json.load(open(sys.argv[1]))
print("served=%d solved=%d"%(d["served"],d["solved"]), " ".join("%s=%s/%s"%(f["function"][:2],f["verdict"],f["evidence_tier"]) for f in d["functions"]))' "$out/$name.json" 2>&1)"
}
run cold "$work/pristine" "${hs[@]}"
run warm "$work/pristine" "${hs[@]}"
run warm2 "$work/pristine" "${hs[@]}"
# Seeded source: a different digest, so nothing may be served.
run seeded-cold "$work/seeded" --harness harnesses::p2_try_push_full_is_err
run seeded-warm "$work/seeded" --harness harnesses::p2_try_push_full_is_err
# A different solver flag is a different key; kissat completes, z3 breaks CBMC.
run kissat-cold "$work/pristine" --harness harnesses::p3_insert_remove_inverse --kani-arg=--solver --kani-arg=kissat
run kissat-warm "$work/pristine" --harness harnesses::p3_insert_remove_inverse --kani-arg=--solver --kani-arg=kissat
run z3-toolerror-1 "$work/pristine" --harness harnesses::p3_insert_remove_inverse --kani-arg=--solver --kani-arg=z3
run z3-toolerror-2 "$work/pristine" --harness harnesses::p3_insert_remove_inverse --kani-arg=--solver --kani-arg=z3
# A one-second wall-clock limit on the most expensive harness: never stored.
run timeout-1 "$work/pristine" --harness harnesses::p3_insert_remove_inverse --kani-arg=--default-unwind --kani-arg=7 --timeout 1
run timeout-2 "$work/pristine" --harness harnesses::p3_insert_remove_inverse --kani-arg=--default-unwind --kani-arg=7 --timeout 1
echo "stored entries: $(find "$cache/kani-v1" -name '*.json' | wc -l)"
echo PROTODONE

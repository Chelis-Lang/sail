#!/usr/bin/env bash
# The whole pilot on a native host, from the committed sources:
# setup, direct Kani twice (pristine and seeded), extras, prototype.
#
#   spikes/kani-pilot/run-all.sh WORK
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=${1:?work dir}
"$here/host-setup.sh" "$work" || exit 1
rm -rf "$work/out"
hs=""
for n in p1_push_pop_roundtrip p2_try_push_full_is_err p3_insert_remove_inverse \
  p4_truncate_keeps_prefix p5_retain_filters p6_array_string_push_char \
  f1_push_never_panics f2_drain_any_range_never_panics e1_retain_unwind_too_small; do
  hs="$hs harnesses::$n"
done
for r in r1 r2; do
  # shellcheck disable=SC2086
  "$here/run-direct.sh" "$work/pristine/pilot" "$work/out" "$r" $hs
  "$here/run-direct.sh" "$work/seeded/pilot" "$work/out" "$r-seeded" harnesses::p2_try_push_full_is_err
done
"$here/run-extras.sh" "$work"
"$here/run-proto.sh" "$work"
echo ALLDONE

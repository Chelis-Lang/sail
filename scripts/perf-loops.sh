#!/usr/bin/env bash
# Edit-loop timings for docs/performance.md: plain `cargo verus verify`,
# `sail verify` and `sail verify --incremental` on one project, plus MCP
# `verify` on unchanged code.
#
#   scripts/perf-loops.sh demo|vest [ROUNDS]       (default 3 rounds)
#
# Each round gives each mode a fresh copy of the project and, for Sail, a
# fresh cache (SAIL_CACHE_DIR), then runs the phases in order:
#   cold   first verification of the new copy (builds and verifies vstd)
#   warm   again, unchanged
#   edit   a body edit that still verifies
#   fail   a failing edit (a broken postcondition), on top of `edit`
#   fix    the fix: the postcondition restored and the proof body changed,
#          so the fixed function's query is new
#   warm2  again, unchanged
# After the `--incremental` phases, one `sail mcp` session calls `verify`
# twice on the unchanged code (mcp1, mcp2). Mode order rotates by round.
#
# Projects:
#   demo   `cargo verus new --lib demo` plus four small functions
#   vest   secure-foundations/vest 11af01dd, `-p vest_lib` (3,237 functions);
#          the edits are the frozen repair catalogue's (corpus/repair-edits.json)
#
# Needs `sail`, `sail-z3` and `cargo verus` on PATH, plus git, perl and awk.
# Environment:
#   SAIL_PERF_OUT       output directory (default ~/sail-perf/PROJECT-TIMESTAMP);
#                       keep it short, it holds the daemon sockets
#   SAIL_PERF_VEST_SRC  an existing vest clone with commit 11af01dd (else cloned)
#   SAIL_PERF_MACHINE   free-text machine description for the header
#
# Prints a table of median (min-max) wall seconds per mode and phase, and
# keeps every log and results.tsv in the output directory.
set -euo pipefail

project="${1:-}"
rounds="${2:-3}"
case "$project" in demo | vest) ;; *) sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;; esac
case "$rounds" in '' | *[!0-9]*) echo "ROUNDS must be a number" >&2; exit 2 ;; esac

vest_commit=11af01ddaef76cceded03517cd48764adb9d35b1
out="${SAIL_PERF_OUT:-$HOME/sail-perf/$project-$(date +%Y%m%dT%H%M%S)}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
results="$out/results.tsv"
: >"$results"

# Inherited SAIL_* selectors would change what is measured.
for v in $(env | sed -n 's/^\(SAIL_[A-Z0-9_]*\)=.*/\1/p'); do
  case "$v" in SAIL_PERF_*) ;; *) unset "$v" ;; esac
done

for tool in sail sail-z3 cargo-verus verus git perl awk; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH" >&2; exit 1; }
done

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }

if [ "$project" = vest ]; then
  pkg_args=(-p vest_lib)
  mcp_args=',"cargo_args":["-p","vest_lib"]'
  edit_file=vest_lib/src/primitives/base128.rs
else
  pkg_args=()
  mcp_args=''
  edit_file=src/lib.rs
fi

# replace FILE FIND REPLACE: exactly one occurrence, or fail.
replace() {
  FIND="$2" REPL="$3" perl -0777 -i -pe '
    my $n = () = /\Q$ENV{FIND}\E/g;
    die "expected one match, found $n\n" unless $n == 1;
    s/\Q$ENV{FIND}\E/$ENV{REPL}/;
  ' "$1"
}

apply_edit() { # DIR PHASE
  local f="$1/$edit_file"
  if [ "$project" = vest ]; then
    case "$2" in
      edit) replace "$f" $'    assert(bytes.push(b).drop_last() == bytes);\n' \
        $'    assert(bytes.push(b).len() == bytes.len() + 1);\n    assert(bytes.push(b).drop_last() == bytes);\n' ;;
      fail) replace "$f" $'        pow(128, exp + 1) == pow(128, exp) * 128,\n' \
        $'        pow(128, exp + 1) == pow(128, exp) * 128 + 1,\n' ;;
      fix)
        replace "$f" $'        pow(128, exp + 1) == pow(128, exp) * 128 + 1,\n' \
          $'        pow(128, exp + 1) == pow(128, exp) * 128,\n'
        replace "$f" $'    lemma_pow_adds(128, exp, 1);\n    lemma_pow1(128);\n' \
          $'    lemma_pow1(128);\n    lemma_pow_adds(128, exp, 1);\n' ;;
    esac
  else
    case "$2" in
      edit) replace "$f" $'} // verus!\n' \
        $'pub fn min(a: u64, b: u64) -> (r: u64)\n    ensures\n        r <= a && r <= b,\n        r == a || r == b,\n{\n    if a <= b { a } else { b }\n}\n\n} // verus!\n' ;;
      fail) replace "$f" $'        r >= a && r >= b,\n' $'        r > a && r >= b,\n' ;;
      fix)
        replace "$f" $'        r > a && r >= b,\n' $'        r >= a && r >= b,\n'
        replace "$f" $'    if a >= b { a } else { b }\n' $'    if b > a { b } else { a }\n' ;;
    esac
  fi
}

demo_source() {
  cat <<'EOF'
use vstd::prelude::*;

verus! {

pub open spec fn sum_to(n: nat) -> nat
    decreases n,
{
    if n == 0 { 0 } else { n + sum_to((n - 1) as nat) }
}

pub proof fn lemma_sum_to_monotone(a: nat, b: nat)
    requires
        a <= b,
    ensures
        sum_to(a) <= sum_to(b),
    decreases b,
{
    if a < b {
        lemma_sum_to_monotone(a, (b - 1) as nat);
    }
}

pub fn max(a: u64, b: u64) -> (r: u64)
    ensures
        r >= a && r >= b,
        r == a || r == b,
{
    if a >= b { a } else { b }
}

pub fn find(v: &Vec<u64>, x: u64) -> (r: Option<usize>)
    ensures
        match r {
            Some(i) => i < v.len() && v[i as int] == x,
            None => forall|j: int| 0 <= j < v.len() ==> v[j] != x,
        },
{
    let mut i = 0;
    while i < v.len()
        invariant
            i <= v.len(),
            forall|j: int| 0 <= j < i ==> v[j] != x,
        decreases v.len() - i,
    {
        if v[i] == x {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub fn count_to(n: u64) -> (r: u64)
    ensures
        r == n,
{
    let mut i: u64 = 0;
    while i < n
        invariant
            i <= n,
        decreases n - i,
    {
        i += 1;
    }
    i
}

} // verus!
EOF
}

vest_src=""
if [ "$project" = vest ]; then
  vest_src="${SAIL_PERF_VEST_SRC:-$out/vest-src}"
  if ! git -C "$vest_src" cat-file -e "$vest_commit^{commit}" 2>/dev/null; then
    git clone -q https://github.com/secure-foundations/vest "$vest_src"
  fi
fi

fresh_copy() { # DIR
  rm -rf "$1"
  if [ "$project" = vest ]; then
    mkdir -p "$1"
    git -C "$vest_src" archive "$vest_commit" | tar -x -C "$1"
  else
    local parent name
    parent="$(dirname "$1")" name="$(basename "$1")"
    (cd "$parent" && cargo verus new --lib "$name" >/dev/null 2>&1)
    demo_source >"$1/src/lib.rs"
  fi
}

stop_daemons() { # CACHE
  pkill -f -- "--socket $1/" 2>/dev/null || true
  sleep 1
}

record() { # ROUND MODE PHASE SECONDS EXIT
  printf '%s\t%s\t%s\t%s\t%s\n' "$@" >>"$results"
  printf '  round %s %-11s %-5s %8.2f s  exit %s\n' "$@"
}

run_phase() { # ROUND MODE DIR PHASE
  local round="$1" mode="$2" dir="$3" phase="$4" log t0 t1 code=0
  log="$out/r$round/$mode.$phase.log"
  t0=$(now)
  case "$mode" in
    plain) (cd "$dir" && cargo verus verify ${pkg_args[@]+"${pkg_args[@]}"}) >"$log" 2>&1 || code=$? ;;
    sail) (cd "$dir" && sail verify ${pkg_args[@]+"${pkg_args[@]}"}) >"$log" 2>&1 || code=$? ;;
    incremental) (cd "$dir" && sail verify --incremental ${pkg_args[@]+"${pkg_args[@]}"}) >"$log" 2>&1 || code=$? ;;
  esac
  t1=$(now)
  record "$round" "$mode" "$phase" "$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f", b - a }')" "$code"
}

# One MCP session over named pipes: initialize, then `verify` twice.
run_mcp() { # ROUND DIR
  local round="$1" dir="$2" d="$out/r$1/mcp" line t0 t1 id code pid
  mkdir -p "$d"
  rm -f "$d/in" "$d/out"
  mkfifo "$d/in" "$d/out"
  (cd "$dir" && exec sail mcp <"$d/in" >"$d/out" 2>"$d/stderr") &
  pid=$!
  exec 3>"$d/in" 4<"$d/out"
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"perf-loops","version":"1"}}}' >&3
  while IFS= read -r line <&4; do case "$line" in *'"id":1'[,}]*) break ;; esac; done
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&3
  for id in 2 3; do
    t0=$(now)
    printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"verify","arguments":{"project_dir":"%s"%s}}}\n' \
      "$id" "$dir" "$mcp_args" >&3
    while IFS= read -r line <&4; do case "$line" in *"\"id\":$id"[,}]*) break ;; esac; done
    t1=$(now)
    printf '%s\n' "$line" >"$d/reply-$id.json"
    code=0
    case "$line" in *'"isError":true'*) code=1 ;; esac
    record "$round" mcp "mcp$((id - 1))" "$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f", b - a }')" "$code"
  done
  exec 3>&- 4<&-
  wait "$pid" 2>/dev/null || true
}

run_mode() { # ROUND MODE
  local round="$1" mode="$2" dir cache
  dir="$out/r$round/$mode-proj"
  cache="$out/r$round/c-$mode"
  rm -rf "$cache"
  mkdir -p "$cache"
  fresh_copy "$dir"
  export SAIL_CACHE_DIR="$cache"
  run_phase "$round" "$mode" "$dir" cold
  run_phase "$round" "$mode" "$dir" warm
  for phase in edit fail fix; do
    apply_edit "$dir" "$phase"
    run_phase "$round" "$mode" "$dir" "$phase"
  done
  run_phase "$round" "$mode" "$dir" warm2
  if [ "$mode" = incremental ]; then run_mcp "$round" "$dir"; fi
  stop_daemons "$cache"
  unset SAIL_CACHE_DIR
}

cpu() {
  if [ -r /proc/cpuinfo ]; then sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -1
  else sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown; fi
}
cores() { getconf _NPROCESSORS_ONLN 2>/dev/null || echo '?'; }
load() { uptime | sed 's/.*load average[s]*: //'; }

{
  echo "project: $project ($([ "$project" = vest ] && echo "vest $vest_commit, -p vest_lib" || echo 'cargo verus new demo + 4 functions'))"
  echo "machine: ${SAIL_PERF_MACHINE:-unspecified}; $(uname -sm); $(cpu); $(cores) cores"
  echo "sail: $(sail --version 2>/dev/null | head -1) at $(command -v sail)"
  echo "verus: $(verus --version 2>&1 | sed -n "s/^ *Version: //p")"
  echo "rounds: $rounds; output: $out"
  echo "load average at start: $(load)"
} | tee "$out/header.txt"

modes=(plain sail incremental)
for ((r = 1; r <= rounds; r++)); do
  mkdir -p "$out/r$r"
  for ((i = 0; i < 3; i++)); do
    run_mode "$r" "${modes[$(((r - 1 + i) % 3))]}"
  done
done
echo "load average at end: $(load)" | tee -a "$out/header.txt"

# Report: median (min-max) seconds per mode and phase.
table() {
  awk -F'\t' '
    function med(a, n,   i, j, t) {
      for (i = 2; i <= n; i++) { t = a[i]; for (j = i - 1; j >= 1 && a[j] > t; j--) a[j + 1] = a[j]; a[j + 1] = t }
      return (n % 2) ? a[(n + 1) / 2] : (a[n / 2] + a[n / 2 + 1]) / 2
    }
    function fmt(x) { return (x < 10) ? sprintf("%.2f", x) : sprintf("%.1f", x) }
    { k = $2 SUBSEP $3; n[k]++; v[k, n[k]] = $4; if (!(k in lo) || $4 < lo[k]) lo[k] = $4; if (!(k in hi) || $4 > hi[k]) hi[k] = $4 }
    END {
      split("cold warm edit fail fix warm2", ph, " ")
      print "| Mode | cold | warm | edit | fail | fix | warm2 |"
      print "|---|---:|---:|---:|---:|---:|---:|"
      split("plain sail incremental", md, " ")
      label["plain"] = "`cargo verus verify`"; label["sail"] = "`sail verify`"; label["incremental"] = "`sail verify --incremental`"
      for (m = 1; m <= 3; m++) {
        row = "| " label[md[m]]
        for (p = 1; p <= 6; p++) {
          k = md[m] SUBSEP ph[p]
          if (!(k in n)) { row = row " | -"; continue }
          delete a; for (i = 1; i <= n[k]; i++) a[i] = v[k, i]
          row = row " | " fmt(med(a, n[k])) " (" fmt(lo[k]) "-" fmt(hi[k]) ")"
        }
        print row " |"
      }
      for (p = 1; p <= 2; p++) {
        k = "mcp" SUBSEP "mcp" p
        if (!(k in n)) continue
        delete a; for (i = 1; i <= n[k]; i++) a[i] = v[k, i]
        printf "\nMCP `verify` on unchanged code, call %d of one session: %s s (%s-%s)\n", p, fmt(med(a, n[k])), fmt(lo[k]), fmt(hi[k])
      }
    }' "$results"
}

echo
echo "Wall seconds, median (min-max) over $rounds round(s):"
echo
table | tee "$out/table.md"

# Exit codes must agree across modes (MCP reports a failure inside a
# successful call, so it is checked separately).
echo
mismatch=$(awk -F'\t' '$2 != "mcp" { k = $1 SUBSEP $3; if ((k in c) && c[k] != $5) bad[k] = 1; c[k] = $5 }
  END { for (k in bad) { split(k, x, SUBSEP); print "round " x[1] " phase " x[2] } }' "$results")
if [ -n "$mismatch" ]; then
  echo "EXIT CODES DIFFER between modes:"; echo "$mismatch"; exit 1
fi
expected="fail"
bad=$(awk -F'\t' -v f="$expected" '$2 != "mcp" && (($3 == f) != ($5 != 0)) { print "round " $1 " " $2 " " $3 " exit " $5 }' "$results")
if [ -n "$bad" ]; then echo "UNEXPECTED EXIT CODES:"; echo "$bad"; exit 1; fi
echo "exit codes: identical across modes; only the failing edit fails"

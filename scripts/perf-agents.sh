#!/usr/bin/env bash
# Several agents on one Sail daemon, for docs/performance.md.
#
#   scripts/perf-agents.sh demo|vest [AGENTS] [ROUNDS]   (defaults 4 and 3)
#   scripts/perf-agents.sh report OUT_DIR [AGENTS]        (table of an earlier run)
#
# AGENTS separate copies of the project (separate Cargo workspaces) share one
# daemon and one cache. Each copy is verified once per arm as a warm-up (the
# job environment, including SAIL_JOB_WORKERS, is part of the proof context,
# so each arm has its own compiled dependencies and receipts). In each round,
# every agent appends a comment to a source file and submits
# `sail verify --background` at the same moment, then follows its job to the
# end. The round's time is from submission until the last job finishes.
# Two arms alternate by round, each with a freshly started daemon:
#   serial    SAIL_JOB_WORKERS=1 (one job at a time, the behaviour before L15)
#   default   SAIL_JOB_WORKERS unset (a quarter of the cores, 1 to 4)
#
# Projects and environment are those of scripts/perf-loops.sh
# (SAIL_PERF_OUT, SAIL_PERF_VEST_SRC, SAIL_PERF_MACHINE).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

# report RESULTS AGENTS: the table from a results.tsv.
report() {
  awk -F'\t' -v n="$2" '
  function med(a, k,   i, j, t) {
    for (i = 2; i <= k; i++) { t = a[i]; for (j = i - 1; j >= 1 && a[j] > t; j--) a[j + 1] = a[j]; a[j + 1] = t }
    return (k % 2) ? a[(k + 1) / 2] : (a[k / 2] + a[k / 2 + 1]) / 2
  }
  { c[$2]++; v[$2, c[$2]] = $3; if (!($2 in lo) || $3 < lo[$2]) lo[$2] = $3; if (!($2 in hi) || $3 > hi[$2]) hi[$2] = $3 }
  END {
    print "| Daemon | " n " agents finish, median (min-max) s |"
    print "|---|---:|"
    split("serial default", arm, " ")
    for (x = 1; x <= 2; x++) {
      delete a; for (i = 1; i <= c[arm[x]]; i++) a[i] = v[arm[x], i]
      m[x] = med(a, c[arm[x]])
      label = (arm[x] == "serial") ? "one job at a time (`SAIL_JOB_WORKERS=1`)" : "default workers"
      printf "| %s | %.1f (%.1f-%.1f) |\n", label, m[x], lo[arm[x]], hi[arm[x]]
    }
    printf "\nSpeedup of the default over one at a time: %.2fx\n", m[1] / m[2]
  }' "$1"
}

if [ "${1:-}" = report ] && [ -f "${2:-}/results.tsv" ]; then
  report "$2/results.tsv" "${3:-4}"
  exit
fi

project="${1:-}"
agents="${2:-4}"
rounds="${3:-3}"
case "$project" in demo | vest) ;; *) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;; esac
case "$agents$rounds" in *[!0-9]*) echo "AGENTS and ROUNDS must be numbers" >&2; exit 2 ;; esac

vest_commit=11af01ddaef76cceded03517cd48764adb9d35b1
out="${SAIL_PERF_OUT:-$HOME/sail-perf/agents-$project-$(date +%Y%m%dT%H%M%S)}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
results="$out/results.tsv"
: >"$results"

# Sail's Verus (ADR 0043): an explicit build, else the CI image's runtime,
# else the one in the Sail cache.
sail_perf_verus="${SAIL_VERUS_BUILD:-${SAIL_ENGINE_BUILD:-}}"
for v in $(env | sed -n 's/^\(SAIL_[A-Z0-9_]*\)=.*/\1/p'); do
  case "$v" in SAIL_PERF_*) ;; *) unset "$v" ;; esac
done
[ -z "$sail_perf_verus" ] || export SAIL_VERUS_BUILD="$sail_perf_verus"
for tool in sail sail-z3 cargo-verus git perl awk; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH" >&2; exit 1; }
done

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }

if [ "$project" = vest ]; then
  pkg_args=(-p vest_lib)
  edit_file=vest_lib/src/primitives/base128.rs
  vest_src="${SAIL_PERF_VEST_SRC:-$out/vest-src}"
  if ! git -C "$vest_src" cat-file -e "$vest_commit^{commit}" 2>/dev/null; then
    git clone -q https://github.com/secure-foundations/vest "$vest_src"
  fi
else
  pkg_args=()
  edit_file=src/lib.rs
fi

export SAIL_CACHE_DIR="$out/c"
mkdir -p "$SAIL_CACHE_DIR"

stop_daemon() {
  "$here/sail-stop-daemon.sh"
  sleep 1
}

# submit DIR LOG: submit a background job, follow it, print its status line.
submit() {
  local id
  id=$(cd "$1" && sail verify --background --output json ${pkg_args[@]+"${pkg_args[@]}"} |
    sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  [ -n "$id" ] || { echo "no job id" >"$2"; return 1; }
  sail jobs logs "$id" --follow >"$2" 2>&1 || true
  sail jobs status "$id" >>"$2" 2>&1
  tail -1 "$2"
}

echo "setting up $agents copies"
for ((a = 1; a <= agents; a++)); do
  d="$out/a$a"
  rm -rf "$d"
  if [ "$project" = vest ]; then
    mkdir -p "$d"
    git -C "$vest_src" archive "$vest_commit" | tar -x -C "$d"
  else
    (cd "$out" && cargo verus new --lib "a$a" >/dev/null 2>&1)
  fi
done

{
  echo "project: $project; agents: $agents; rounds: $rounds; output: $out"
  echo "machine: ${SAIL_PERF_MACHINE:-unspecified}; $(uname -sm); $(getconf _NPROCESSORS_ONLN) cores"
  echo "load average at start: $(uptime | sed 's/.*load average[s]*: //')"
} | tee "$out/header.txt"

arms=(serial default)
for ((r = 1; r <= rounds; r++)); do
  for ((i = 0; i < 2; i++)); do
    arm="${arms[$(((r - 1 + i) % 2))]}"
    stop_daemon
    if [ "$arm" = serial ]; then export SAIL_JOB_WORKERS=1; else unset SAIL_JOB_WORKERS; fi
    if [ "$r" = 1 ]; then
      # The job environment is part of the proof context, so each arm warms up.
      echo "warm-up ($arm): one job per copy"
      for ((a = 1; a <= agents; a++)); do submit "$out/a$a" "$out/warmup-$arm-a$a.log" >/dev/null & done
      wait
    fi
    for ((a = 1; a <= agents; a++)); do
      printf '// perf-agents round %s arm %s\n' "$r" "$arm" >>"$out/a$a/$edit_file"
    done
    t0=$(now)
    for ((a = 1; a <= agents; a++)); do
      (submit "$out/a$a" "$out/r$r-$arm-a$a.log" >/dev/null; now >"$out/r$r-$arm-a$a.end") &
    done
    wait
    t1=$(awk '{ if ($1 > m) m = $1 } END { print m }' "$out"/r"$r"-"$arm"-a*.end)
    ok=$(grep -l 'Completed' "$out"/r"$r"-"$arm"-a*.log | wc -l | tr -d ' ')
    total=$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.2f", b - a }')
    printf '%s\t%s\t%s\t%s\n' "$r" "$arm" "$total" "$ok" >>"$results"
    printf '  round %s %-7s %7.2f s, %s/%s jobs completed\n' "$r" "$arm" "$total" "$ok" "$agents"
  done
done
stop_daemon
echo "load average at end: $(uptime | sed 's/.*load average[s]*: //')" | tee -a "$out/header.txt"

echo
report "$results" "$agents" | tee "$out/table.md"
if awk -F'\t' -v n="$agents" '$4 != n { bad = 1 } END { exit !bad }' "$results"; then
  echo "SOME JOBS DID NOT COMPLETE"; exit 1
fi

#!/usr/bin/env bash
# Edit loops on pinned real projects: plain `cargo verus` against `sail verify`,
# with cache hits, misses and Verus's own time breakdown for every run.
#
#   scripts/perf-real.sh run PROJECT [ROUNDS]     timed edit loops (default 3 rounds)
#   scripts/perf-real.sh det PROJECT [RUNS]       SMT logs: unchanged reruns and each
#                                                 edit compared with the first run
#   scripts/perf-real.sh report OUT_DIR           print a run's tables again
#
# Projects (corpus/corpus.toml pins; sources from `sail-conformance fetch`):
#   vest        vest_lib                     (secure-foundations/vest)
#   capybarakv  capybaraKV/capybarakv        (microsoft/verified-storage)
#   verismo     source/verismo, which also verifies its dependency verismo_tspec
#   ironkv      ironsht                      (verus-lang/verified-ironkv)
#   anvil       --lib, --verify-module vdeployment_controller
# Each copy has its Verus dependencies patched to the pinned Verus source, as
# the conformance harness does (sail_verus::patch).
#
# `run`: each round gives each mode a fresh copy and, for Sail, an empty cache,
# then runs, in order:
#   cold        first verification (dependencies, vstd and the project)
#   noop1..N    a new comment at the end of the edit file: no query changes,
#               so every check should hit (the all-hits floor, and unchanged
#               reruns for nondeterminism)
#   each edit   applied alone to the pristine source (see `edits_PROJECT`)
#   noopend     the pristine source plus a comment, after the edits
# Modes (SAIL_PERF_MODES, default "plain sail"):
#   plain        cargo verus verify (focus with --verify-module)
#   sail         sail verify
#   incremental  explicit legacy alias for sail verify --incremental
#   fork-plain, fork-sail  the same with the Verus in SAIL_PERF_FORK: first on
#                PATH, and for fork-sail also as SAIL_CARGO_VERUS (Sail does not
#                search PATH, ADR 0043)
# Every run passes `--time-expanded` to Verus. Sail runs record the daemon's
# counters (`sail status --json`) before and after, and the shim's per-session
# provenance (SAIL_PROVENANCE_DIR: replies served, solver started).
#
# `det`: plain runs with `--log smt` into separate directories: RUNS unchanged
# reruns, then each edit once. Every SMT file is compared with the first run's:
# identical, or the number of (check-sat)s before the first differing line,
# with that line's context kept under det/diffs/.
#
# Environment:
#   SAIL_PERF_OUT      output directory (default ~/sail-perf/real-PROJECT-TIMESTAMP);
#                      keep it short, it holds the daemon sockets
#   SAIL_PERF_MODES    modes for `run`
#   SAIL_PERF_FORK     directory holding a fork build's cargo-verus, verus and z3
#                      (fork-* modes; for `det`, every run uses it)
#   SAIL_PERF_EDITS    edits to run (default: all of the project's)
#   SAIL_PERF_NOOPS    unchanged reruns after cold (default 3)
#   SAIL_PERF_CORPUS   fetched corpus (default ~/.cache/sail/corpus)
#   SAIL_PERF_VERUS_ARGS  extra Verus arguments for every run (e.g. "-V spinoff-all")
#   SAIL_PERF_MACHINE  free-text machine description for the header
# Needs sail, sail-z3, sail-conformance, cargo verus, python3, perl, git, jq.
set -euo pipefail

cmd="${1:-}"
case "$cmd" in run | det | report) ;; *) sed -n '2,52p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;; esac
shift
here="$(cd "$(dirname "$0")" && pwd)"
sail_root="$(cd "$here/.." && pwd)"

if [ "$cmd" = report ]; then
  python3 -I "$here/perf-real-report.py" "${1:?OUT_DIR}"
  exit
fi

project="${1:-}"
count="${2:-3}"
case "$count" in '' | *[!0-9]*) echo "count must be a number" >&2; exit 2 ;; esac

# Inherited SAIL_* selectors would change what is measured.
for v in $(env | sed -n 's/^\(SAIL_[A-Z0-9_]*\)=.*/\1/p'); do
  case "$v" in SAIL_PERF_*) ;; *) unset "$v" ;; esac
done

corpus="${SAIL_PERF_CORPUS:-$HOME/.cache/sail/corpus}"
verus_src="$corpus/verus-426d8b01e7ff"

# Project definitions: corpus entry, source directory, package directory,
# cargo and Verus arguments, the file noop comments go to, and the edits.
# Edit lines are "NAME OP FILE LINE EXPECTED_EXIT", LINE being the line of
# the target `fn` (body, fail, fix, spec), or the insertion point (comment,
# lemma). See apply_op for the operations.
cargs=() vargs=()
case "$project" in
  vest)
    entry=vest src=vest-11af01ddaef7 pkg=vest_lib
    noop_file=vest_lib/src/asn1/der_ord.rs
    edits() { cat <<'EOF'
catalog catalog vest_lib/src/primitives/base128.rs 0 0
body-early body vest_lib/src/asn1/der_ord.rs 245 0
body-late body vest_lib/src/asn1/der_ord.rs 2341 0
comment-top comment vest_lib/src/asn1/der_ord.rs 1 0
lemma-mid lemma vest_lib/src/asn1/der_ord.rs 1770 0
spec-wide rename vest_lib/src/core/spec.rs 24 0
fail fail vest_lib/src/asn1/der_ord.rs 1784 1
fix fix vest_lib/src/asn1/der_ord.rs 1784 0
EOF
    } ;;
  capybarakv)
    entry=verified-storage src=verified-storage-41cab9505122 pkg=capybaraKV/capybarakv
    cargs=(--no-default-features)
    noop_file=capybaraKV/capybarakv/src/kv2/rwkv_v.rs
    edits() { cat <<'EOF'
body-early body capybaraKV/capybarakv/src/kv2/rwkv_v.rs 68 0
body-late body capybaraKV/capybarakv/src/kv2/rwkv_v.rs 1073 0
comment-top comment capybaraKV/capybarakv/src/kv2/rwkv_v.rs 1 0
lemma-mid lemma capybaraKV/capybarakv/src/kv2/rwkv_v.rs 673 0
spec-wide rename capybaraKV/capybarakv/src/common/subrange_v.rs 31 0
fail fail capybaraKV/capybarakv/src/kv2/rwkv_v.rs 375 1
fix fix capybaraKV/capybarakv/src/kv2/rwkv_v.rs 375 0
EOF
    } ;;
  verismo)
    entry=verismo-main src=verismo-6bc6dd326d8b pkg=source/verismo
    noop_file=source/verismo/src/security/mem.rs
    # fail and fix sit in tspec_e/array/sort.rs, not security/mem.rs: verismo's
    # trusted axiom_size_from_cast_bytes with SpecSize for char (1) contradicts
    # vstd's size_of::<char>() == 4, so `assert(false)` verifies in any
    # function whose context keeps both facts, including all of mem.rs (#160).
    edits() { cat <<'EOF'
body-early body source/verismo/src/security/mem.rs 191 0
body-late body source/verismo/src/security/mem.rs 1264 0
comment-top comment source/verismo/src/security/mem.rs 1 0
lemma-mid lemma source/verismo/src/security/mem.rs 627 0
lemma-early lemma source/verismo/src/arch/rmp/db_u.rs 53 0
spec-wide rename source/verismo/src/arch/rmp/db_u.rs 53 0
fail fail source/verismo/src/tspec_e/array/sort.rs 9 1
fix fix source/verismo/src/tspec_e/array/sort.rs 9 0
upstream-body body source/verismo_tspec/src/security/sectype.rs 221 0
upstream-spec rename source/verismo_tspec/src/range_set.rs 66 0
EOF
    } ;;
  ironkv)
    entry=verified-ironkv src=verified-ironkv-7e35ed097c40 pkg=ironsht
    noop_file=ironsht/src/host_impl_v.rs
    edits() { cat <<'EOF'
body-early body ironsht/src/host_impl_v.rs 151 0
body-late body ironsht/src/host_impl_v.rs 1780 0
comment-top comment ironsht/src/host_impl_v.rs 1 0
lemma-mid lemma ironsht/src/host_impl_v.rs 1336 0
spec-wide rename ironsht/src/app_interface_t.rs 24 0
fail fail ironsht/src/host_impl_v.rs 639 1
fix fix ironsht/src/host_impl_v.rs 639 0
EOF
    } ;;
  anvil)
    entry=anvil src=anvil-a597f9de7dbe pkg=.
    cargs=(--lib) vargs=(--verify-module vdeployment_controller)
    noop_file=src/controllers/vdeployment_controller/proof/liveness/resource_match.rs
    edits() { cat <<'EOF'
body-early body src/controllers/vdeployment_controller/proof/liveness/resource_match.rs 23 0
body-late body src/controllers/vdeployment_controller/proof/liveness/resource_match.rs 2025 0
comment-top comment src/controllers/vdeployment_controller/proof/liveness/resource_match.rs 1 0
lemma-mid lemma src/controllers/vdeployment_controller/proof/liveness/resource_match.rs 1106 0
spec-wide rename src/controllers/vdeployment_controller/trusted/liveness_theorem.rs 161 0
fail fail src/controllers/vdeployment_controller/proof/liveness/resource_match.rs 1874 1
fix fix src/controllers/vdeployment_controller/proof/liveness/resource_match.rs 1874 0
EOF
    } ;;
  *) echo "unknown project \`$project\` (vest, capybarakv, verismo, ironkv, anvil)" >&2; exit 2 ;;
esac

# Extra Verus arguments for every mode (for example "-V spinoff-all").
# shellcheck disable=SC2206
vargs+=(${SAIL_PERF_VERUS_ARGS:-})

selected_edits() {
  if [ -n "${SAIL_PERF_EDITS:-}" ]; then
    local e
    for e in $SAIL_PERF_EDITS; do edits | awk -v e="$e" '$1 == e'; done
  else
    edits
  fi
}

out="${SAIL_PERF_OUT:-$HOME/sail-perf/real-$project-$(date +%Y%m%dT%H%M%S)}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

for tool in sail sail-z3 sail-conformance cargo-verus verus python3 perl git jq; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH" >&2; exit 1; }
done
fork="${SAIL_PERF_FORK:-}"
if [ -n "$fork" ] && [ ! -x "$fork/cargo-verus" ]; then
  echo "SAIL_PERF_FORK=$fork has no cargo-verus" >&2; exit 1
fi

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }

# --- Source preparation ------------------------------------------------------

if [ ! -d "$corpus/$src" ] || [ ! -d "$verus_src" ]; then
  sail-conformance fetch --tier standard --project "$entry" >&2
fi

prepare_pristine() {
  local p="$out/pristine" patcher="$out/patcher"
  [ -f "$p/.sail-perf-ready" ] && return 0
  mkdir -p "$patcher/src"
  cat >"$patcher/Cargo.toml" <<EOF
[package]
name = "sail-perf-patcher"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
sail-verus = { path = "$sail_root/crates/sail-verus" }

[workspace]
EOF
  cat >"$patcher/src/main.rs" <<'EOF'
//! Patches a project's Verus dependencies exactly as the conformance harness does.
fn main() -> std::process::ExitCode {
    let a: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    let [package, root, verus] = a.as_slice() else {
        return std::process::ExitCode::from(2);
    };
    match sail_verus::patch::patch_verus_deps(package, root, verus) {
        Ok(p) => {
            println!("patched {:?}: {}", p.manifest, p.crates.join(", "));
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
EOF
  # From the Sail checkout, so that its pinned Rust toolchain builds it.
  (cd "$sail_root" && CARGO_TARGET_DIR="$patcher/target" cargo build --release -q --manifest-path "$patcher/Cargo.toml")
  rm -rf "$p"
  cp -r "$corpus/$src" "$p"
  rm -rf "$p/.git"
  "$patcher/target/release/sail-perf-patcher" "$p/$pkg" "$p" "$verus_src"
  (cd "$p/$pkg" && CARGO_TARGET_DIR="$out/metadata-target" cargo metadata --format-version 1 >/dev/null)
  rm -rf "$out/metadata-target"
  touch "$p/.sail-perf-ready"
}

fresh_copy() { # DIR
  rm -rf "$1"
  cp -r "$out/pristine" "$1"
}

# apply_op DIR OP FILE LINE: edit FILE (relative to DIR), starting from the
# pristine copy of it.
#   body     `assert(true);` as the first statement of the fn at LINE
#   fail     `assert(1 + 1 == 3);` there (fails)
#   fix      `assert(1 + 1 == 2);` there (verifies; new query bytes)
#   spec     the body of the module-level bool spec fn at LINE wrapped as
#            `sail_perf_true() && ({ ... })`, with that spec fn added before it
#            (a semantic no-op, but it can push dependent proofs over rlimit)
#   rename   the first parameter of the fn at LINE renamed throughout the fn
#            (a pure refactoring; used for `spec-wide`)
#   comment  a comment line inserted before LINE
#   lemma    a new small lemma inserted before the item at LINE
#   noop     a comment appended at the end (LINE is a counter)
#   catalog  vest only: the frozen catalogue's edit (scripts/perf-loops.sh)
apply_op() {
  local dir="$1" op="$2" file="$3" line="$4"
  cp "$out/pristine/$file" "$dir/$file"
  OP="$op" LINE="$line" perl -0777 -e '
    my $src = do { local $/; <> };
    my ($op, $line) = ($ENV{OP}, $ENV{LINE});
    my @l = split /^/m, $src;
    my $pos = 0; $pos += length($l[$_]) for 0 .. $line - 2;
    sub body_open {
      my ($s, $from) = @_;
      my $fn = index($s, "fn ", $from);
      die "no fn at line $line\n" if $fn < 0 || $fn > $from + length((split /\n/, substr($s, $from))[0]);
      my ($par, $brk) = (0, 0);
      for (my $i = $fn; $i < length($s); $i++) {
        my $c = substr($s, $i, 1);
        if ($c eq "/" && substr($s, $i, 2) eq "//") { $i = index($s, "\n", $i); next }
        $par++ if $c eq "("; $par-- if $c eq ")";
        $brk++ if $c eq "["; $brk-- if $c eq "]";
        return $i if $c eq "{" && $par == 0 && $brk == 0;
      }
      die "no body\n";
    }
    sub body_close {
      my ($s, $open) = @_;
      my $d = 0;
      for (my $i = $open; $i < length($s); $i++) {
        my $c = substr($s, $i, 1);
        if ($c eq "/" && substr($s, $i, 2) eq "//") { $i = index($s, "\n", $i); next }
        $d++ if $c eq "{";
        if ($c eq "}") { $d--; return $i if $d == 0 }
      }
      die "unbalanced\n";
    }
    if ($op =~ /^(body|fail|fix)$/) {
      my %stmt = (body => "assert(true);", fail => "assert(1 + 1 == 3);", fix => "assert(1 + 1 == 2);");
      my $o = body_open($src, $pos);
      substr($src, $o + 1, 0) = " $stmt{$op}";
    } elsif ($op eq "spec") {
      # Verus drops a literal `true &&`, so the conjunct is a new spec fn.
      my $o = body_open($src, $pos);
      my $c = body_close($src, $o);
      substr($src, $c, 0) = "})";
      substr($src, $o + 1, 0) = " sail_perf_true() && ({";
      my $i = $line - 1;
      $i-- while $i > 0 && $l[$i - 1] =~ /^\s*(#\[|\/\/)/;
      my $p = 0; $p += length($l[$_]) for 0 .. $i - 1;
      substr($src, $p, 0) = "pub open spec fn sail_perf_true() -> bool {\n    true\n}\n\n";
    } elsif ($op eq "rename") {
      my $o = body_open($src, $pos);
      my $c = body_close($src, $o);
      my $fn = index($src, "fn ", $pos);
      my $region = substr($src, $fn, $c - $fn + 1);
      $region =~ /^fn\s+\w+\s*(?:<[^(]*>)?\s*\(\s*(\w+)\s*:/ or die "rename: no first parameter\n";
      my $name = $1;
      my $n = ($region =~ s/(?<![.\w])\Q$name\E(?!\w)/${name}_renamed/g);
      die "rename: $name used once\n" unless $n > 1;
      substr($src, $fn, $c - $fn + 1) = $region;
    } elsif ($op eq "comment") {
      substr($src, $pos, 0) = "// sail-perf: an inserted comment line\n";
    } elsif ($op eq "lemma") {
      my $i = $line - 1;
      $i-- while $i > 0 && $l[$i - 1] =~ /^\s*(#\[|\/\/)/;
      my $p = 0; $p += length($l[$_]) for 0 .. $i - 1;
      substr($src, $p, 0) = "pub proof fn sail_perf_new_lemma(x: int)\n    ensures\n        x + 0 == x,\n{\n}\n\n";
    } elsif ($op eq "noop") {
      $src .= "// sail-perf noop $line\n";
    } elsif ($op eq "catalog") {
      my $f = "    assert(bytes.push(b).drop_last() == bytes);\n";
      my $n = () = $src =~ /\Q$f\E/g; die "catalog: $n matches\n" unless $n == 1;
      $src =~ s/\Q$f\E/    assert(bytes.push(b).len() == bytes.len() + 1);\n$f/;
    } else { die "unknown op $op\n" }
    print $src;
  ' "$out/pristine/$file" >"$dir/$file.tmp"
  mv "$dir/$file.tmp" "$dir/$file"
}

reset_files() { # DIR: every edited file back to pristine
  local f
  for f in $(edits | awk '{print $3}' | sort -u) "$noop_file"; do
    cmp -s "$out/pristine/$f" "$1/$f" || cp "$out/pristine/$f" "$1/$f"
  done
}

# --- Running -----------------------------------------------------------------

partial=0
for a in ${vargs[@]+"${vargs[@]}"}; do case "$a" in --verify-module | --verify-function) partial=1 ;; esac; done

verus_cmd() { # MODE EXTRA_VERUS_ARGS...
  local mode="$1"; shift
  local sub=verify
  [ "$partial" = 1 ] && sub=focus
  case "$mode" in
    plain | fork-plain) cargo verus "$sub" ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    sail) sail verify ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    # Sail never searches PATH for Verus (ADR 0043); the development
    # override is the only way to run another build under Sail.
    fork-sail) SAIL_CARGO_VERUS="$fork/cargo-verus" sail verify ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    incremental) sail verify --incremental ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
  esac
}

mode_path() { # MODE: PATH for the mode
  case "$1" in fork-*) echo "$fork:$PATH" ;; *) echo "$PATH" ;; esac
}

stats() { # -> "hellos gets hits puts" of the daemon for SAIL_CACHE_DIR, or zeros
  { sail status --json 2>/dev/null || true; } |
    jq -r 'if .running then "\(.stats.hellos) \(.stats.gets) \(.stats.hits) \(.stats.puts)" else "0 0 0 0" end' 2>/dev/null |
    grep . || echo "0 0 0 0"
}

# Verus's --time output, summed over every crate verified in the run:
# total rust vir verify-crate (wall ms), verify and smt-run (ms summed over threads).
times_of() { # LOG
  awk '
    /^total-time:/ { t += $2 }
    /^    rust-time:/ { r += $2 }
    /^        vir-time:/ { v += $2 }
    /^        verify-crate-time:/ { c += $2 }
    /^    total verify-time:/ { vs += $3 }
    /^            total smt-run:/ { s += $3 }
    END { printf "%d\t%d\t%d\t%d\t%d\t%d", t, r, v, c, vs, s }' "$1"
}

results="$out/results.tsv"

run_one() { # ROUND MODE DIR PHASE
  local round="$1" mode="$2" dir="$3" phase="$4" log prov t0 t1 code=0 s0 s1
  log="$out/r$round/$mode.$phase.log"
  prov="$out/r$round/prov/$mode.$phase"
  mkdir -p "$prov"
  s0=$(stats)
  # Quietness: settle, then record the 1-minute load average (which still
  # carries the previous run's own work) and the processes running right now.
  local load1
  sleep "${SAIL_PERF_SETTLE:-5}"
  if [ -r /proc/loadavg ]; then
    load1=$(awk '{split($4, p, "/"); print $1 "/" p[1] - 1}' /proc/loadavg)
  else
    load1=$(uptime | sed 's/.*load average[s]*: //; s/,.*//')
  fi
  t0=$(now)
  (cd "$dir/$pkg" && PATH="$(mode_path "$mode")" SAIL_PROVENANCE_DIR="$prov" verus_cmd "$mode" --time-expanded) >"$log" 2>&1 || code=$?
  t1=$(now)
  s1=$(stats)
  local sessions started served
  sessions=$(find "$prov" -name '*.json' | wc -l | tr -d ' ')
  # One JSON object per file, without a trailing newline.
  started=$( (cat "$prov"/*.json 2>/dev/null || true) | (grep -o '"solver_started":true' || true) | wc -l | tr -d ' ')
  served=$( (cat "$prov"/*.json 2>/dev/null || true) | (grep -o '"replies_served":[0-9]*' || true) | cut -d: -f2 | awk '{s += $1} END {print s + 0}')
  local d
  d=$(echo "$s0 $s1" | awk '{printf "%d\t%d\t%d\t%d", $5-$1, $6-$2, $7-$3, $8-$4}')
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$round" "$mode" "$phase" \
    "$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f", b - a }')" "$code" \
    "$sessions" "$started" "$served" "$d" "$(times_of "$log")" "$load1" >>"$results"
  printf '  r%s %-11s %-14s %8.2f s exit %s  load %s  sessions %s (solver %s)  daemon %s\n' "$round" "$mode" "$phase" \
    "$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f", b - a }')" "$code" "$load1" "$sessions" "$started" "$d"
}

run_mode() { # ROUND MODE
  local round="$1" mode="$2" dir="$out/r$1/$2-proj" cache="$out/r$1/c-$2" i
  rm -rf "$cache"; mkdir -p "$cache"
  fresh_copy "$dir"
  export SAIL_CACHE_DIR="$cache"
  run_one "$round" "$mode" "$dir" cold
  for ((i = 1; i <= ${SAIL_PERF_NOOPS:-3}; i++)); do
    apply_op "$dir" noop "$noop_file" "$i"
    run_one "$round" "$mode" "$dir" "noop$i"
  done
  while read -r name op file line _; do
    reset_files "$dir"
    apply_op "$dir" "$op" "$file" "$line"
    run_one "$round" "$mode" "$dir" "$name"
  done < <(selected_edits)
  reset_files "$dir"
  apply_op "$dir" noop "$noop_file" end
  run_one "$round" "$mode" "$dir" noopend
  pkill -f -- "--socket $cache/" 2>/dev/null || true
  sleep 1
  unset SAIL_CACHE_DIR
  rm -rf "$dir"
}

cpu() { sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo 2>/dev/null | head -1 || sysctl -n machdep.cpu.brand_string; }
header() {
  echo "project: $project ($src, package $pkg, cargo args: ${cargs[*]:-none}, verus args: ${vargs[*]:-none})"
  echo "machine: ${SAIL_PERF_MACHINE:-unspecified}; $(uname -sm); $(cpu); $(getconf _NPROCESSORS_ONLN) cores"
  echo "sail: $(sail --version 2>/dev/null | head -1) at $(command -v sail)"
  echo "verus: $(verus --version 2>&1 | sed -n 's/^ *Version: //p')"
  [ -n "$fork" ] && echo "fork: $(sed -n 's/^ *//p' "$fork/version.txt" 2>/dev/null | head -1) at $fork"
  echo "load average at start: $(uptime | sed 's/.*load average[s]*: //')"
}

prepare_pristine

if [ "$cmd" = det ]; then
  # Plain runs with SMT logs, compared file by file with the first. With
  # SAIL_PERF_FORK set, the fork's Verus runs them.
  d="$out/det"
  mkdir -p "$d"
  [ -n "$fork" ] && export PATH="$fork:$PATH"
  { header; echo "det: $count unchanged reruns, then each edit; verus: $(command -v cargo-verus)"; } | tee "$d/header.txt"
  dir="$d/proj"
  fresh_copy "$dir"
  (cd "$dir/$pkg" && verus_cmd plain) >"$d/cold.log" 2>&1 || true
  i=0
  logrun() { # NAME
    rm -rf "$d/smt-$1"
    local code=0
    (cd "$dir/$pkg" && verus_cmd plain --log smt --log-dir "$d/smt-$1") >"$d/$1.log" 2>&1 || code=$?
    echo "  det $1: exit $code, $(find "$d/smt-$1" -type f | wc -l | tr -d ' ') SMT files"
  }
  for ((i = 1; i <= count; i++)); do
    apply_op "$dir" noop "$noop_file" "$i"
    logrun "rerun$i"
  done
  while read -r name op file line _; do
    reset_files "$dir"
    apply_op "$dir" noop "$noop_file" "e$name"
    apply_op "$dir" "$op" "$file" "$line"
    logrun "$name"
  done < <(selected_edits)
  python3 -I "$here/perf-real-report.py" --det "$d" | tee "$d/table.md"
  rm -rf "$dir"
  exit 0
fi

: >"$results"
modes=(${SAIL_PERF_MODES:-plain sail})
header | tee "$out/header.txt"
echo "modes: ${modes[*]}; rounds: $count; edits: $(selected_edits | awk '{print $1}' | tr '\n' ' ')" | tee -a "$out/header.txt"
for ((r = 1; r <= count; r++)); do
  mkdir -p "$out/r$r"
  n=${#modes[@]}
  for ((i = 0; i < n; i++)); do
    run_mode "$r" "${modes[$(((r - 1 + i) % n))]}"
  done
done
echo "load average at end: $(uptime | sed 's/.*load average[s]*: //')" | tee -a "$out/header.txt"
python3 -I "$here/perf-real-report.py" "$out" | tee "$out/table.md"

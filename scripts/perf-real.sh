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
# the conformance harness does (sail_verus::patch), unless SAIL_PERF_VSTD is
# `registry` (below). verismo needs the x86_64-unknown-none Rust target on
# the pinned toolchain (corpus `requires`); the run refuses without it. An
# extra target changes the toolchain's identity, so Sail's shipped vstd
# proof cannot apply while it is installed.
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
#   sail         sail verify (public --output json for partial selectors)
#   incremental  explicit legacy alias for sail verify --incremental
# Partial Sail modes use their supported public JSON route; this changes output
# scope, not the Verus module selector. Their JSON omits compiler stage totals,
# so those six table fields are unavailable (NA), never invented zero timings.
#   fork-plain, fork-sail  the same with the Verus in SAIL_PERF_FORK: first on
#                PATH, and for fork-sail also as SAIL_CARGO_VERUS (Sail does not
#                search PATH, ADR 0043)
#   fork-shim    cargo verus verify/focus with sail-z3 and its cache daemon,
#                using SAIL_PERF_FORK; measures the shim, not managed Sail CLI
# Every run passes `--time-expanded` to Verus unless SAIL_PERF_VERUS_TIMES=0
# (then the six stage fields are NA). Sail runs record the daemon's
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
#   SAIL_PERF_VERUS_BUILD  Sail's Verus for the Sail modes, passed as
#                      SAIL_VERUS_BUILD (each Sail run has its own empty cache,
#                      so Sail cannot find a runtime installed in ~/.cache/sail)
#   SAIL_PERF_EDITS    edits to run (default: all of the project's)
#   SAIL_PERF_CATALOGUE  incidental: frozen comment/blank/helper/rename edits
#   SAIL_PERF_BASELINE directory holding the old fork for `sail` mode only;
#                      `plain` stays the stock conformance oracle on PATH
#   SAIL_PERF_NOOPS    comment-only edits after cold (default 3)
#   SAIL_PERF_UNCHANGED 1: measure an unmodified rerun immediately after cold
#   SAIL_PERF_STOCK_HOME stock release install root for corpus fetching only
#   SAIL_PERF_CORPUS   fetched corpus (default ~/.cache/sail/corpus)
#   SAIL_PERF_VERUS_ARGS  extra Verus arguments for every run (e.g. "-V spinoff-all")
#   SAIL_PERF_MACHINE  free-text machine description for the header
#   SAIL_PERF_VSTD     where the copies' Verus dependencies come from:
#                      `source` (default) patches them to the pinned Verus
#                      source on disk; `registry` pins vstd and the verus_*
#                      crates to the crates.io versions of the shipped vstd
#                      build's Cargo.lock, as a user moving the project to
#                      the release's Verus would (git sources become registry
#                      versions; changes go to vstd-adjustments.txt, and
#                      vstd-setup.txt lists the resolved versions that differ
#                      from the shipped lock). Sail runs then record, after
#                      `cold`, whether the shipped vstd proof was served
#                      (r*/MODE.vstd-proof.txt)
#   SAIL_PERF_VERUS_TIMES  0 drops `--time-expanded`: cargo-verus forwards
#                      Verus flags to vstd as well, and the shipped vstd proof
#                      binds them, so the release matrix runs without it, as
#                      a user would
#   SAIL_PERF_VSTD_LOCK  the shipped build's Cargo.lock for `registry`
#                      (default: the one in the release beside `sail`)
# Needs sail, sail-z3, sail-conformance, cargo verus, python3, perl, git, jq.
set -euo pipefail

cmd="${1:-}"
case "$cmd" in run | det | report) ;; *) sed -n '2,/^set -euo pipefail/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 2 ;; esac
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
[ "$count" -gt 0 ] || { echo "count must be positive" >&2; exit 2; }

# Inherited SAIL_* selectors would change what is measured.
for v in $(env | sed -n 's/^\(SAIL_[A-Z0-9_]*\)=.*/\1/p'); do
  case "$v" in SAIL_PERF_*) ;; *) unset "$v" ;; esac
done

corpus="${SAIL_PERF_CORPUS:-$HOME/.cache/sail/corpus}"
verus_src="$corpus/verus-7d829946231d"
rust_toolchain=$(sed -n 's/^rust_toolchain = "\(.*\)"/\1/p' "$sail_root/corpus/toolchain.toml" | head -1)
vstd_mode="${SAIL_PERF_VSTD:-source}"
case "${SAIL_PERF_VERUS_TIMES:-1}" in 1) time_args=(--time-expanded) ;; 0) time_args=() ;; *) echo "SAIL_PERF_VERUS_TIMES must be 0 or 1" >&2; exit 2 ;; esac
case "$vstd_mode" in source | registry) ;; *) echo "SAIL_PERF_VSTD must be source or registry" >&2; exit 2 ;; esac

# Project definitions: corpus entry, source directory, package directory,
# cargo and Verus arguments, the file noop comments go to, and the edits.
# Edit lines are "NAME OP FILE LINE EXPECTED_EXIT", LINE being the line of
# the target `fn` (body, fail, fix, spec), or the insertion point (comment,
# lemma). See apply_op for the operations.
cargs=() vargs=() rust_targets=()
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
    # source/.cargo/config.toml builds for it (corpus `requires`).
    rust_targets=(x86_64-unknown-none)
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

# The incidental acceptance catalogue is opt-in; historical L40 edits and
# explicit legacy modes retain their existing meaning.
if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ]; then
  edits() { python3 -I "$here/perf-incidental.py" list "$project"; }
  edits >/dev/null
elif [ -n "${SAIL_PERF_CATALOGUE:-}" ]; then
  echo "unknown SAIL_PERF_CATALOGUE=$SAIL_PERF_CATALOGUE" >&2; exit 2
fi

# Extra Verus arguments for every mode (for example "-V spinoff-all").
# shellcheck disable=SC2206
vargs+=(${SAIL_PERF_VERUS_ARGS:-})

selected_edits() {
  if [ -n "${SAIL_PERF_EDITS:-}" ]; then
    local e
    for e in $SAIL_PERF_EDITS; do
      local selected
      selected=$(edits | awk -v e="$e" '$1 == e')
      [ -n "$selected" ] || { echo "unknown edit $e for $project" >&2; return 2; }
      printf '%s\n' "$selected"
    done
  else
    edits
  fi
}

out="${SAIL_PERF_OUT:-$HOME/sail-perf/real-$project-$(date +%Y%m%dT%H%M%S)}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
selected_edits >"$out/selected-edits.txt"

for tool in sail sail-z3 sail-conformance cargo-verus verus python3 perl git jq; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH" >&2; exit 1; }
done
if [ ${#rust_targets[@]} -gt 0 ]; then
  installed=$(rustup target list --installed --toolchain "$rust_toolchain" 2>/dev/null || true)
  for t in "${rust_targets[@]}"; do
    printf '%s\n' "$installed" | grep -qx "$t" || {
      echo "$project needs the $t target: rustup target add --toolchain $rust_toolchain $t" >&2; exit 1; }
  done
fi
vstd_lock=""
if [ "$vstd_mode" = registry ]; then
  vstd_lock="${SAIL_PERF_VSTD_LOCK:-}"
  if [ -z "$vstd_lock" ]; then
    locks=("$(dirname "$(command -v sail)")"/../deps/*/Cargo.lock)
    [ ${#locks[@]} -eq 1 ] && [ -f "${locks[0]}" ] ||
      { echo "SAIL_PERF_VSTD=registry: no single shipped deps/*/Cargo.lock beside sail; set SAIL_PERF_VSTD_LOCK" >&2; exit 1; }
    vstd_lock="${locks[0]}"
  fi
  [ -f "$vstd_lock" ] || { echo "SAIL_PERF_VSTD_LOCK=$vstd_lock does not exist" >&2; exit 1; }
fi
fork="${SAIL_PERF_FORK:-}"
if [ -n "$fork" ] && [ ! -x "$fork/cargo-verus" ]; then
  echo "SAIL_PERF_FORK=$fork has no cargo-verus" >&2; exit 1
fi
baseline="${SAIL_PERF_BASELINE:-}"
if [ -n "$baseline" ] && [ ! -x "$baseline/cargo-verus" ]; then
  echo "SAIL_PERF_BASELINE=$baseline has no cargo-verus" >&2; exit 1
fi
if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ] && [ "$cmd" = run ] \
    && { [ -z "$baseline" ] || [ -z "$fork" ]; }; then
  echo "incidental timing requires explicit SAIL_PERF_BASELINE and SAIL_PERF_FORK runtimes" >&2
  exit 2
fi

if [ "$cmd" = run ]; then
  modes=(${SAIL_PERF_MODES:-plain sail})
  if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ]; then
    modes=(${SAIL_PERF_MODES:-plain sail fork-sail})
  fi
  for mode in "${modes[@]}"; do
    case "$mode" in
      plain | sail | incremental | fork-plain | fork-sail | fork-shim) ;;
      *) echo "unknown performance mode: $mode" >&2; exit 2 ;;
    esac
    case "$mode" in
      fork-*) [ -n "$fork" ] || { echo "$mode requires SAIL_PERF_FORK" >&2; exit 2; } ;;
    esac
  done
fi

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }

# --- Source preparation ------------------------------------------------------

if [ ! -d "$corpus/$src" ] || [ ! -d "$verus_src" ]; then
  if [ -n "${SAIL_PERF_STOCK_HOME:-}" ]; then
    SAIL_VERUS_HOME="$SAIL_PERF_STOCK_HOME" sail-conformance fetch --tier standard --project "$entry" >&2
  else
    sail-conformance fetch --tier standard --project "$entry" >&2
  fi
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
//! `source PACKAGE ROOT VERUS`: patches a project's Verus dependencies to the
//! pinned Verus source exactly as the conformance harness does.
//! `registry PACKAGE ROOT GIT_NAMES NAME=VERSION...`: pins them to crates.io
//! versions instead (GIT_NAMES: comma-separated crates to patch in Verus's
//! git source), printing one line per change.
use std::collections::{BTreeMap, BTreeSet};
fn main() -> std::process::ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let result = match a.as_slice() {
        [mode, package, root, verus] if mode == "source" => {
            sail_verus::patch::patch_verus_deps(package.as_ref(), root.as_ref(), verus.as_ref())
                .map(|p| vec![format!("patched {:?}: {}", p.manifest, p.crates.join(", "))])
        }
        [mode, package, root, git, versions @ ..] if mode == "registry" => {
            let git: BTreeSet<String> =
                git.split(',').filter(|n| !n.is_empty()).map(ToOwned::to_owned).collect();
            let versions: BTreeMap<String, String> = versions
                .iter()
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect();
            sail_verus::patch::pin_verus_deps_to_registry(package.as_ref(), root.as_ref(), &versions, &git)
        }
        _ => return std::process::ExitCode::from(2),
    };
    match result {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
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
  if [ "$vstd_mode" = source ]; then
    "$patcher/target/release/sail-perf-patcher" source "$p/$pkg" "$p" "$verus_src"
    (cd "$p/$pkg" && CARGO_TARGET_DIR="$out/metadata-target" cargo metadata --format-version 1 >/dev/null)
  else
    pin_registry "$patcher/target/release/sail-perf-patcher" "$p"
  fi
  rm -rf "$out/metadata-target"
  touch "$p/.sail-perf-ready"
}

# The Verus crates of verus-lang/verus's git source in DIR's resolution,
# comma-separated.
verus_git_crates() { # DIR
  (cd "$1/$pkg" && CARGO_TARGET_DIR="$out/metadata-target" cargo metadata --format-version 1) |
    jq -r '.packages[] | select((.source // "") | startswith("git+https://github.com/verus-lang/verus.git")) | .name' |
    sort -u | paste -sd, -
}

pin_registry() { # PATCHER DIR: Verus crates to the shipped lock's crates.io versions
  local patcher="$1" p="$2" git versions=()
  while read -r v; do versions+=("$v"); done < <(python3 -I "$here/perf-matrix.py" vstd-versions "$vstd_lock")
  "$patcher" registry "$p/$pkg" "$p" "" "${versions[@]}" >"$out/vstd-adjustments.txt"
  # A git dependency's own Verus git dependencies (anvil's verus-tla, for
  # example) follow through Verus's git source. When they cannot even
  # resolve beside the registry crates (Verus's git head can pin other
  # versions of crates that declare the same `links`), patch every Verus
  # crate of the shipped lock there: a git vstd brings all of them.
  if ! git=$(verus_git_crates "$p" 2>"$out/metadata.err"); then
    git=$(printf '%s\n' "${versions[@]}" | cut -d= -f1 | paste -sd, -)
  fi
  if [ -n "$git" ]; then
    "$patcher" registry "$p/$pkg" "$p" "$git" "${versions[@]}" >>"$out/vstd-adjustments.txt"
    git=$(verus_git_crates "$p" 2>"$out/metadata.err") || { cat "$out/metadata.err" >&2; return 1; }
    [ -z "$git" ] || { echo "Verus git crates remain after pinning: $git" >&2; return 1; }
    # Every added patch must be used, or each build would warn about it.
    if grep -i "was not used in the crate graph" "$out/metadata.err" >&2; then return 1; fi
  fi
  local root
  root=$(cd "$p/$pkg" && CARGO_TARGET_DIR="$out/metadata-target" cargo metadata --format-version 1 --no-deps | jq -r .workspace_root)
  python3 -I "$here/perf-matrix.py" lock-diff "$vstd_lock" "$root/Cargo.lock" >"$out/vstd-setup.txt"
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
  if [ "$op" = incidental ]; then
    python3 -I "$here/perf-incidental.py" apply "$project" "$line" "$out/pristine" "$dir" >>"$out/source-edits.jsonl"
    return
  fi
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
  if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ]; then
    python3 -I "$here/perf-incidental.py" restore "$project" "$1"
  fi
}

# --- Running -----------------------------------------------------------------

partial=0
for a in ${vargs[@]+"${vargs[@]}"}; do
  case "$a" in
    --verify-module | --verify-module=* | --verify-only-module | --verify-only-module=* | \
    --verify-function | --verify-function=* | --verify-root) partial=1 ;;
  esac
done

verus_cmd() { # MODE EXTRA_VERUS_ARGS...
  local mode="$1"; shift
  local sub=verify sail_output=()
  if [ "$partial" = 1 ]; then sub=focus; sail_output=(--output json); fi
  case "$mode" in
    fork-shim)
      VERUS_Z3_PATH="$(command -v sail-z3)" SAIL_Z3_PATH="$fork/z3" \
        cargo verus "$sub" ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    plain | fork-plain) cargo verus "$sub" ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    sail)
      if [ -n "$baseline" ]; then
        SAIL_CARGO_VERUS="$baseline/cargo-verus" sail verify ${sail_output[@]+"${sail_output[@]}"} ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@"
      else
        sail verify ${sail_output[@]+"${sail_output[@]}"} ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@"
      fi ;;
    # Sail never searches PATH for Verus (ADR 0043); the development
    # override is the only way to run another build under Sail.
    fork-sail) SAIL_CARGO_VERUS="$fork/cargo-verus" sail verify ${sail_output[@]+"${sail_output[@]}"} ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    incremental) sail verify --incremental ${sail_output[@]+"${sail_output[@]}"} ${cargs[@]+"${cargs[@]}"} -- ${vargs[@]+"${vargs[@]}"} "$@" ;;
    *) echo "unknown performance mode: $mode" >&2; return 2 ;;
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
times_of() { # LOG MODE
  if [ "${SAIL_PERF_VERUS_TIMES:-1}" = 0 ]; then
    printf 'NA\tNA\tNA\tNA\tNA\tNA'
    return
  fi
  if [ "$partial" = 1 ]; then
    case "$2" in
      sail | fork-sail | incremental)
        # The public Sail JSON report does not contain Verus's times-ms.
        # Retain raw output, and make absent stage measurements explicit.
        printf 'NA\tNA\tNA\tNA\tNA\tNA'
        return ;;
    esac
  fi
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
sample_failures=0

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
  (cd "$dir/$pkg" && if [ -n "${SAIL_PERF_VERUS_BUILD:-}" ]; then export SAIL_VERUS_BUILD="$SAIL_PERF_VERUS_BUILD"; fi &&
    PATH="$(mode_path "$mode")" SAIL_PROVENANCE_DIR="$prov" verus_cmd "$mode" ${time_args[@]+"${time_args[@]}"}) >"$log" 2>&1 || code=$?
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
    "$sessions" "$started" "$served" "$d" "$(times_of "$log" "$mode")" "$load1" >>"$results"
  printf '  r%s %-11s %-14s %8.2f s exit %s  load %s  sessions %s (solver %s)  daemon %s\n' "$round" "$mode" "$phase" \
    "$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f", b - a }')" "$code" "$load1" "$sessions" "$started" "$d"
  # Every incidental edit must verify, including cold/unchanged controls.
  # Retain a failed sample and finish the table, then refuse acceptance.
  if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ] && [ "$code" -ne 0 ]; then
    sample_failures=$((sample_failures + 1))
  fi
}

shim_pid=""
cleanup_shim() {
  if [ -n "$shim_pid" ]; then
    kill "$shim_pid" 2>/dev/null || true
    wait "$shim_pid" 2>/dev/null || true
    shim_pid=""
  fi
}
trap cleanup_shim EXIT

run_mode() { # ROUND MODE
  local round="$1" mode="$2" dir="$out/r$1/$2-proj" cache="$out/r$1/c-$2" i
  rm -rf "$cache"; mkdir -p "$cache"
  fresh_copy "$dir"
  reset_files "$dir"
  export SAIL_CACHE_DIR="$cache"
  if [ "$mode" = fork-shim ]; then
    [ -n "$fork" ] && [ -x "$fork/z3" ] || { echo "fork-shim requires SAIL_PERF_FORK with z3" >&2; exit 2; }
    export SAIL_DAEMON_SOCKET="$cache/d.sock"
    sail serve --socket "$SAIL_DAEMON_SOCKET" >"$out/r$round/$mode.daemon.log" 2>&1 &
    shim_pid=$!
    local ready=0 attempt
    for ((attempt = 0; attempt < 100; attempt++)); do
      if sail status --json 2>/dev/null | jq -e '.running' >/dev/null; then ready=1; break; fi
      kill -0 "$shim_pid" 2>/dev/null || break
      sleep 0.05
    done
    [ "$ready" = 1 ] || { echo "fork-shim daemon did not start; see $out/r$round/$mode.daemon.log" >&2; exit 2; }
  fi
  run_one "$round" "$mode" "$dir" cold
  if [ "${SAIL_PERF_UNCHANGED:-0}" = 1 ]; then
    run_one "$round" "$mode" "$dir" unchanged
  fi
  case "$mode" in
    sail | fork-sail | incremental)
      python3 -I "$here/perf-matrix.py" vstd-proof "$cache" "$out/r$round/$mode.cold.log" \
        >"$out/r$round/$mode.vstd-proof.txt" || true ;;
  esac
  for ((i = 1; i <= ${SAIL_PERF_NOOPS:-3}; i++)); do
    apply_op "$dir" noop "$noop_file" "$i"
    run_one "$round" "$mode" "$dir" "noop$i"
  done
  while read -r name op file line _; do
    reset_files "$dir"
    apply_op "$dir" "$op" "$file" "$line"
    run_one "$round" "$mode" "$dir" "$name"
  done <"$out/selected-edits.txt"
  reset_files "$dir"
  apply_op "$dir" noop "$noop_file" end
  run_one "$round" "$mode" "$dir" noopend
  reset_files "$dir"
  if [ "$mode" = fork-shim ]; then
    cleanup_shim
    unset SAIL_DAEMON_SOCKET
  else
    pkill -f -- "--socket $cache/" 2>/dev/null || true
  fi
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
  echo "vstd: $vstd_mode${vstd_lock:+ (shipped lock $vstd_lock)}"
  [ ! -s "$out/vstd-adjustments.txt" ] || sed 's/^/  adjusted: /' "$out/vstd-adjustments.txt"
  [ ! -s "$out/vstd-setup.txt" ] || sed 's/^/  /' "$out/vstd-setup.txt"
  [ ${#rust_targets[@]} -eq 0 ] ||
    echo "rust targets: ${rust_targets[*]} on $rust_toolchain (changes the toolchain identity: the shipped vstd proof cannot apply)"
  [ -n "$fork" ] && echo "fork: $(sed -n 's/^ *//p' "$fork/version.txt" 2>/dev/null | head -1) at $fork"
  if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ]; then
    python3 -I "$here/perf-incidental.py" check "$corpus" >"$out/catalogue-manifest.json"
    python3 - "$sail_root" "$baseline" "$fork" <<'PY'
import hashlib, json, pathlib, shutil, sys
files = [pathlib.Path(sys.argv[1]) / 'corpus/incidental-edits.json']
files += [pathlib.Path(p) for n in ('sail', 'sail-z3', 'cargo-verus', 'verus') if (p := shutil.which(n))]
for directory in sys.argv[2:]:
    if directory:
        files += [pathlib.Path(directory) / n for n in ('cargo-verus', 'verus', 'z3', 'version.txt')]
print('acceptance identities: ' + json.dumps({str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in files if p.is_file()}))
PY
  fi
  echo "load average at start: $(uptime | sed 's/.*load average[s]*: //')"
}

prepare_pristine

if [ "$cmd" = det ]; then
  # Plain runs with SMT logs, compared file by file with the first. With
  # SAIL_PERF_FORK set, the fork's Verus runs them.
  d="$out/det"
  mkdir -p "$d"
  if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ] && [ -e "$d/results.tsv" ]; then
    echo "incidental SMT capture needs a fresh output directory" >&2; exit 2
  fi
  : >"$d/results.tsv"
  [ -n "$fork" ] && export PATH="$fork:$PATH"
  { header; echo "det: $count unchanged reruns, then each edit; verus: $(command -v cargo-verus)"; } | tee "$d/header.txt"
  dir="$d/proj"
  fresh_copy "$dir"
  cold_code=0
  (cd "$dir/$pkg" && verus_cmd plain) >"$d/cold.log" 2>&1 || cold_code=$?
  printf 'cold\t%s\t-\n' "$cold_code" >>"$d/results.tsv"
  if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ] && [ "$cold_code" -ne 0 ]; then
    sample_failures=$((sample_failures + 1))
  fi
  i=0
  logrun() { # NAME
    rm -rf "$d/smt-$1"
    mkdir -p "$d/smt-$1"
    local code=0
    (cd "$dir/$pkg" && verus_cmd plain --log smt --log-dir "$d/smt-$1") >"$d/$1.log" 2>&1 || code=$?
    local files
    files=$(find "$d/smt-$1" -type f | wc -l | tr -d ' ')
    echo "  det $1: exit $code, $files SMT files"
    printf '%s\t%s\t%s\n' "$1" "$code" "$files" >>"$d/results.tsv"
    if [ "${SAIL_PERF_CATALOGUE:-}" = incidental ] && { [ "$code" -ne 0 ] || [ "$files" -eq 0 ]; }; then
      sample_failures=$((sample_failures + 1))
    fi
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
  done <"$out/selected-edits.txt"
  python3 -I "$here/perf-real-report.py" --det "$d" | tee "$d/table.md"
  reset_files "$dir"
  rm -rf "$dir"
  [ "$sample_failures" -eq 0 ] || { echo "incidental SMT capture failed; not acceptance evidence" >&2; exit 1; }
  exit 0
fi

: >"$results"
header | tee "$out/header.txt"
echo "modes: ${modes[*]}; rounds: $count; edits: $(awk '{print $1}' "$out/selected-edits.txt" | tr '\n' ' ')" | tee -a "$out/header.txt"
for ((r = 1; r <= count; r++)); do
  mkdir -p "$out/r$r"
  n=${#modes[@]}
  for ((i = 0; i < n; i++)); do
    run_mode "$r" "${modes[$(((r - 1 + i) % n))]}"
  done
done
echo "load average at end: $(uptime | sed 's/.*load average[s]*: //')" | tee -a "$out/header.txt"
python3 -I "$here/perf-real-report.py" "$out" | tee "$out/table.md"
if [ "$sample_failures" -ne 0 ]; then
  echo "$sample_failures incidental samples failed; not an acceptance result" >&2
  exit 1
fi

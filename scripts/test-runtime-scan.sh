#!/usr/bin/env bash
# Regressions for scripts/scan-host-paths.sh, the host-path scan that guards
# runtime archive publication (issue #555): matches anywhere in large files,
# and fail-closed behaviour when the tree cannot be read.
set -euo pipefail
scratch="$(mktemp -d)"
trap 'chmod -R u+rwx "$scratch" 2>/dev/null || true; rm -rf "$scratch"' EXIT
passed=0
# expect <status> <label> <scan args...>
expect() {
  local want="$1" label="$2" got=0
  shift 2
  bash scripts/scan-host-paths.sh "$@" >"$scratch/output" 2>&1 || got=$?
  if [ "$got" != "$want" ]; then
    printf 'runtime scan (%s): status %s, expected %s\n' "$label" "$got" "$want" >&2
    cat "$scratch/output" >&2
    exit 1
  fi
  passed=$((passed + 1))
}
# tree NAME: a fresh tree with one clean binary-ish file.
tree() {
  rm -rf "${scratch:?}/$1" && mkdir -p "$scratch/$1/bin"
  printf 'ordinary\0bytes\n' >"$scratch/$1/bin/tool"
  printf '%s' "$scratch/$1"
}
# big FILE PREFIX SUFFIX: about 3.2 MB of printable lines between them.
big() {
  { printf '%s' "$2"; awk 'BEGIN { for (i = 0; i < 100000; i++) print "an ordinary printable line of text" }'; printf '%s' "$3"; } >"$1"
}

t="$(tree clean)"
expect 0 clean "$t"

t="$(tree early)"
big "$t/bin/large" $'/Users/sail-scan-probe\n' ''
expect 1 "large file, early match" "$t"

t="$(tree late)"
big "$t/bin/large" '' $'\n/home/sail-scan-probe\n'
expect 1 "large file, late match" "$t"

t="$(tree short)"
printf 'x\0/Users/a\0' >"$t/bin/short"
expect 1 "match in a run shorter than strings' minimum" "$t"

t="$(tree token)"
printf 'built on sail-host-17\n' >"$t/bin/info"
expect 0 "token not requested" "$t"
expect 1 "extra token" "$t" sail-host-17

t="$(tree link)"
ln -s /home/someone/lib "$t/bin/lib"
expect 1 "symbolic link target" "$t"

expect 2 "missing directory" "$scratch/does-not-exist"
mkdir -p "$scratch/empty/sub"
expect 2 "no files" "$scratch/empty"
t="$(tree emptytoken)"
expect 2 "empty token" "$t" ''

t="$(tree fifo)"
mkfifo "$t/bin/pipe"
expect 2 "unexpected file type" "$t"

# A read error reported by grep (status 2) is fatal, even as root.
mkdir -p "$scratch/fakebin"
printf '#!/bin/sh\necho "grep: injected read error" >&2\nexit 2\n' >"$scratch/fakebin/grep"
chmod +x "$scratch/fakebin/grep"
t="$(tree readerror)"
PATH="$scratch/fakebin:$PATH" expect 2 "grep read error" "$t"

if [ "$(id -u)" != 0 ]; then
  t="$(tree unreadable)"
  printf 'secret\n' >"$t/bin/locked" && chmod 000 "$t/bin/locked"
  expect 2 "unreadable file" "$t"
  t="$(tree hidden)"
  mkdir -p "$t/locked" && printf 'x\n' >"$t/locked/file" && chmod 000 "$t/locked"
  expect 2 "unenumerable directory" "$t"
else
  echo "runtime scan: running as root, permission cases skipped"
fi



# ADR 0112: explicit platform-scoped public namespace, at exact runtime positions.
runtime=verus/source/target-verus/release
mac=/Users/runner/work/verus/verus/source/builtin
linux=/home/runner/work/verus/verus/source/builtin
for platform in macos linux; do
  namespace="$mac"
  [ "$platform" != linux ] || namespace="$linux"
  for name in libverus_builtin.rlib libvstd.rlib vstd.vir; do
    t="$(tree "scoped-$platform-$name")"
    mkdir -p "$t/$runtime"
    printf 'prefix\0%s/src/lib.rs\0suffix' "$namespace" >"$t/$runtime/$name"
    expect 1 "canonical metadata remains strict by default" "$t"
    expect 0 "scoped canonical $platform $name" --stock-builtin-namespace "$platform" "$t"
    expect 1 "explicit public token stays strict" --stock-builtin-namespace "$platform" "$t" "$namespace"
    printf '\0/home/another-person/private\0' >>"$t/$runtime/$name"
    expect 1 "mixed personal path in allowed metadata" --stock-builtin-namespace "$platform" "$t"
  done
done

for suffix in _lookalike /../private /src/../../private /./src/lib.rs //src/lib.rs '\other'; do
  t="$(tree scoped-lookalike)"
  mkdir -p "$t/$runtime"
  printf '%s%s\0' "$mac" "$suffix" >"$t/$runtime/vstd.vir"
  expect 1 "namespace lookalike or noncanonical path" --stock-builtin-namespace macos "$t"
done

t="$(tree scoped-foreign-platform)"
mkdir -p "$t/$runtime"
printf '%s/src/lib.rs\0' "$linux" >"$t/$runtime/vstd.vir"
expect 1 "other platform namespace" --stock-builtin-namespace macos "$t"
for wrong in bin/libvstd.rlib libvstd.rlib "$runtime/rust_verify" verus/source/target-verus/release-other/vstd.vir; do
  t="$(tree scoped-wrong-position)"
  mkdir -p "$(dirname "$t/$wrong")"
  printf '%s/src/lib.rs\0' "$mac" >"$t/$wrong"
  expect 1 "canonical path in wrong runtime position" --stock-builtin-namespace macos "$t"
done

t="$(tree scoped-link)"
mkdir -p "$t/$runtime"
ln -s "$mac/src/lib.rs" "$t/$runtime/vstd.vir"
expect 1 "allowed filename symlink remains strict" --stock-builtin-namespace macos "$t"

t="$(tree scoped-large)"
mkdir -p "$t/$runtime"
python3 - "$t/$runtime/vstd.vir" "$mac" <<'PYFIXTURE'
from pathlib import Path
import sys
namespace=sys.argv[2].encode()
Path(sys.argv[1]).write_bytes(b'x'*(1024*1024-10)+namespace+b'/src/lib.rs\0'+b'y'*(2*1024*1024))
PYFIXTURE
expect 0 "canonical namespace crosses streaming chunk" --stock-builtin-namespace macos "$t"
printf '\0/Users/not-runner/private\0' >>"$t/$runtime/vstd.vir"
expect 1 "late personal path after canonical match" --stock-builtin-namespace macos "$t"
expect 2 "unknown platform is not an exemption" --stock-builtin-namespace windows "$t"
expect 2 "empty scoped explicit token" --stock-builtin-namespace macos "$t" ''

t="$(tree scoped-reader-error)"
mkdir -p "$t/$runtime"
printf '%s/src/lib.rs\0' "$mac" >"$t/$runtime/vstd.vir"
mkdir -p "$scratch/fakepython"
printf '#!/bin/sh\nexit 2\n' >"$scratch/fakepython/python3"
chmod +x "$scratch/fakepython/python3"
PATH="$scratch/fakepython:$PATH" expect 2 "scoped reader failure is fatal" --stock-builtin-namespace macos "$t"
PATH="$scratch/fakebin:$PATH" expect 2 "scoped grep read error is fatal" --stock-builtin-namespace macos "$t"

printf 'runtime scan regressions: %s passed\n' "$passed"

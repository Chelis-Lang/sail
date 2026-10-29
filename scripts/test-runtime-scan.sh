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

printf 'runtime scan regressions: %s passed\n' "$passed"

#!/usr/bin/env bash
# Temporary L11 capture driver (not committed).
. ~/.cargo/env
cd ~/sail
cargo build --release -p sail-bench -p sail-shim -p sail-cli > ~/l11-build.log 2>&1 || exit 1
mkdir -p ~/l11
B=$PWD/target/release
"$B/sail-bench" warm-capture --out ~/l11/edit --loops corpus/agent-loops.toml \
  --edits corpus/edits.toml --repair corpus/repair-edits.json --rounds 3 \
  --shim "$B/sail-z3" --sail "$B/sail" > ~/l11/edit.log 2>&1
"$B/sail-bench" warm-capture --out ~/l11/fast --tier fast \
  --shim "$B/sail-z3" --sail "$B/sail" > ~/l11/fast.log 2>&1
echo finished >> ~/l11/fast.log

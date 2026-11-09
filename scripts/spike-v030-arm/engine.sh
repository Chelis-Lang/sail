#!/usr/bin/env bash
# Rebuild sail with the aarch64 host arm, stage the source-built stock as the "release" install,
# then build Sail's engine runtime through the real xtask path.
set -euo pipefail
. ~/.cargo/env
unset RUSTC_WRAPPER RUSTFLAGS RUSTUP_TOOLCHAIN || true
cd ~/sail
s=$(date +%s); cargo build --release 2>&1 | tail -2; echo "TIMING sail-rebuild $(( $(date +%s) - s )) s"
inst=$HOME/.cache/sail/verus/0.2026.10.18.7d82994/verus-arm64-linux
mkdir -p "$inst"
rel=$HOME/vsrc/stock/source/target-verus/release
for f in "$rel"/*; do [ -f "$f" ] && case "$f" in *.d) ;; *) cp -p "$f" "$inst/";; esac; done
ls -la "$inst"
s=$(date +%s)
SAIL_BUILD_JOBS=16 cargo xtask build-verus-source --interactive-engine
echo "TIMING xtask-engine $(( $(date +%s) - s )) s"
target/release/sail engine status || true
echo ENGINE-DONE

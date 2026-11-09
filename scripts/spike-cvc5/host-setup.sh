#!/usr/bin/env bash
# SPIKE: prepare a native Linux host for the cvc5 runtime and certificate
# measurements. Run from the synced checkout (~/sail). Idempotent.
set -euo pipefail
tools="$HOME/spike-tools"
mkdir -p "$tools"
# m4: Carcara's gmp-mpfr-sys dependency builds GMP from source.
sudo apt-get install -yq unzip m4 >/dev/null

if ! command -v cargo >/dev/null; then
  curl -fsS https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none >/dev/null
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

# Pinned cvc5 (corpus/toolchain.toml [cvc5]).
if [ ! -x "$tools/cvc5-1.4.2/bin/cvc5" ]; then
  curl -fsSL -o "$tools/cvc5-1.4.2.zip" \
    https://github.com/cvc5/cvc5/releases/download/cvc5-1.4.2/cvc5-Linux-x86_64-static.zip
  echo "7eb18f8c814c36a46c5a7d003983e9cfbc4b0a9828d6dd596239291bd4cc61d8  $tools/cvc5-1.4.2.zip" | sha256sum -c -
  unzip -q -o "$tools/cvc5-1.4.2.zip" -d "$tools"
  mv "$tools/cvc5-Linux-x86_64-static" "$tools/cvc5-1.4.2"
fi
echo "6aecddf466552b83bb5883756c7955e0c9d5ddcc6d7f45b48c2f6e06a741b102  $tools/cvc5-1.4.2/bin/cvc5" | sha256sum -c -

# The cvc5 Verus itself pins (verus external_deps.rs), for Verus's cvc5 mode.
if [ ! -x "$tools/cvc5-1.1.2/bin/cvc5" ]; then
  curl -fsSL -o "$tools/cvc5-1.1.2.zip" \
    https://github.com/cvc5/cvc5/releases/download/cvc5-1.1.2/cvc5-Linux-static.zip
  sha256sum "$tools/cvc5-1.1.2.zip"
  unzip -q -o "$tools/cvc5-1.1.2.zip" -d "$tools"
  mv "$tools/cvc5-Linux-static" "$tools/cvc5-1.1.2"
fi
sha256sum "$tools/cvc5-1.1.2/bin/cvc5"

# Carcara at the commit cvc5 1.4.2's own contrib/get-carcara-checker pins,
# built from that source archive with Sail's pinned Rust toolchain.
rev=394edbb15ba95c47893f1d821fddde7e016af178
if [ ! -x "$tools/carcara/bin/carcara" ]; then
  rm -rf "$tools/carcara-src" && mkdir -p "$tools/carcara-src" "$tools/carcara/bin"
  # cvc5's script names the hanielb/carcara fork, which answers 404 for this
  # commit (2026-11-09); the same commit is served by ufmg-smite/carcara.
  curl -fsSL -o "$tools/carcara.tgz" "https://github.com/ufmg-smite/carcara/archive/$rev.tar.gz"
  sha256sum "$tools/carcara.tgz"
  tar --strip 1 -xzf "$tools/carcara.tgz" -C "$tools/carcara-src"
  toolchain=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)
  (cd "$tools/carcara-src" && RUSTC_WRAPPER= CARGO_TARGET_DIR="$tools/carcara-src/target" \
     cargo "+$toolchain" build --release --quiet)
  cp "$tools/carcara-src/target/release/carcara" "$tools/carcara/bin/carcara"
  cp "$tools/carcara-src/rewrites.eo" "$tools/carcara/rewrites.eo" 2>/dev/null || true
fi
sha256sum "$tools/carcara/bin/carcara"
"$tools/carcara/bin/carcara" --version

# Pinned stock Verus with its Z3.
cargo xtask fetch-verus
echo SETUP-DONE

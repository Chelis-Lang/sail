#!/usr/bin/env bash
# Install the pinned Kani release on a native host and lay out the pinned
# arrayvec copies (pristine, seeded, in-crate contract). Idempotent.
#
#   spikes/kani-pilot/host-setup.sh WORK_DIR
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=${1:?work dir}
kani_version=0.68.0
crate=arrayvec-0.7.8
crate_sha256=d3fb67a6e08acf24fdeccbac2cb6ac4305825bd1f117462e0e6f2f193345ad56

export RUSTC_WRAPPER=
if ! command -v cargo >/dev/null; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
if ! cargo kani --version 2>/dev/null | grep -q "$kani_version"; then
  cargo install --locked kani-verifier --version "$kani_version"
  cargo kani setup
fi

mkdir -p "$work/dl"
if [ ! -f "$work/dl/$crate.crate" ]; then
  curl -fsSL -A sail-spike "https://static.crates.io/crates/arrayvec/$crate.crate" -o "$work/dl/$crate.crate"
fi
echo "$crate_sha256  $work/dl/$crate.crate" | sha256sum -c -

# variant NAME [PATCH]: a pilot copy; with PATCH, arrayvec is replaced by a
# patched path copy through [patch.crates-io].
variant() {
  local name=$1 patch=${2:-} dir="$work/$1"
  rm -rf "$dir"
  mkdir -p "$dir"
  cp -r "$here/pilot" "$dir/pilot"
  rm -rf "$dir/pilot/target"
  if [ -n "$patch" ]; then
    tar -xzf "$work/dl/$crate.crate" -C "$dir"
    (cd "$dir/$crate" && patch -p1 <"$here/patches/$patch")
    printf '\n[patch.crates-io]\narrayvec = { path = "../%s" }\n' "$crate" >>"$dir/pilot/Cargo.toml"
  fi
}
variant pristine
variant seeded seeded-try-push-off-by-one.diff
variant contract contract-in-crate.diff
echo "setup complete in $work"

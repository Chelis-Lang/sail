#!/usr/bin/env bash
# Pin the actual Rustup/proxy bytes for the two hosted cache controls. Runner
# image updates may otherwise change their proof contexts between cold/restore.
# This is fixture setup only; Sail keeps its original runtime-identity checks.
set -euo pipefail
sail_rustup_version=1.29.0
sail_rustup_sha256=4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10
sail_cargo_home="${CARGO_HOME:-$HOME/.cargo}"
sail_rustup_work="$(mktemp -d)"
trap 'rm -rf "$sail_rustup_work"' EXIT
curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location   "https://static.rust-lang.org/rustup/archive/$sail_rustup_version/x86_64-unknown-linux-gnu/rustup-init"   -o "$sail_rustup_work/rustup-init"
printf '%s  %s\n' "$sail_rustup_sha256" "$sail_rustup_work/rustup-init" | sha256sum -c -
chmod 755 "$sail_rustup_work/rustup-init"
RUSTUP_INIT_SKIP_PATH_CHECK=yes "$sail_rustup_work/rustup-init"   -y --no-modify-path --profile minimal --default-toolchain none
"$sail_cargo_home/bin/rustup" set auto-self-update disable
for sail_rustup_proxy in rustup cargo rustc; do
  sail_proxy_path="$(command -v "$sail_rustup_proxy")"
  printf '%s  %s\n' "$sail_rustup_sha256" "$sail_proxy_path" | sha256sum -c -
done
"$sail_cargo_home/bin/rustup" --version

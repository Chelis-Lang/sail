#!/usr/bin/env bash
# Prints the CI image tag: a hash of every input to ci/Dockerfile, so the
# image is rebuilt exactly when one of them changes.
set -euo pipefail
cd "$(dirname "$0")/.."
cat ci/Dockerfile ci/fetch-verus.py rust-toolchain.toml corpus/toolchain.toml \
  | sha256sum 2>/dev/null | cut -c1-16 \
  || cat ci/Dockerfile ci/fetch-verus.py rust-toolchain.toml corpus/toolchain.toml | shasum -a 256 | cut -c1-16

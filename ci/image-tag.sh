#!/usr/bin/env bash
# Prints the CI image tag: a hash of every input to ci/Dockerfile, so the
# image is rebuilt exactly when one of them changes. The optional core target
# omits only the MCP smoke client; mcp (the default) keeps the complete image.
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-mcp}" in
  core|mcp) ;;
  *) echo 'usage: ci/image-tag.sh [core|mcp]' >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { echo 'usage: ci/image-tag.sh [core|mcp]' >&2; exit 2; }
python3 - "${1:-mcp}" <<'PY'
import hashlib
import sys
from pathlib import Path

inputs = (
    ".dockerignore", "ci/Dockerfile", "ci/fetch-verus.py", "ci/partition-verus.py", "ci/engine-artifact.py",
    "engine/verus/artifact-linux.json", "engine/verus/fork.toml",
    "engine/verus/Cargo.lock", "rust-toolchain.toml", "corpus/toolchain.toml",
)
# Read every required input before printing: a missing pin must never produce
# an apparently usable tag from a partial hash.
contents = [Path(name).read_bytes() for name in inputs]
digest = hashlib.sha256()
for content in contents:
    digest.update(content)
suffix = "-core" if sys.argv[1] == "core" else ""
print(digest.hexdigest()[:16] + suffix)
PY

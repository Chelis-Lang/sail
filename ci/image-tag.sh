#!/usr/bin/env bash
# Prints the CI image tag: a hash of every input to ci/Dockerfile, so the
# image is rebuilt exactly when one of them changes.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY'
import hashlib
from pathlib import Path

inputs = (
    "ci/Dockerfile", "ci/fetch-verus.py", "ci/engine-artifact.py",
    "engine/verus/artifact-linux.json", "engine/verus/fork.toml",
    "engine/verus/Cargo.lock", "rust-toolchain.toml", "corpus/toolchain.toml",
)
# Read every required input before printing: a missing pin must never produce
# an apparently usable tag from a partial hash.
contents = [Path(name).read_bytes() for name in inputs]
digest = hashlib.sha256()
for content in contents:
    digest.update(content)
print(digest.hexdigest()[:16])
PY

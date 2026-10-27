#!/usr/bin/env bash
# Cargo fingerprints must not cross distinct checkouts mounted at /work.
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-core}" in
  core) sail_target_prefix=sail-ci-target ;;
  singular) sail_target_prefix=sail-singular-target ;;
  *) echo 'usage: ci/target-volume.sh [core|singular]' >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { echo 'usage: ci/target-volume.sh [core|singular]' >&2; exit 2; }
python3 - "$sail_target_prefix" <<'PY'
import hashlib
import os
import sys
from pathlib import Path

# Physical identity keeps symlink aliases warm while separating prepared
# worktrees whose source mtimes may all predate another checkout's build.
identity = os.fsencode(Path.cwd().resolve())
print(sys.argv[1] + "-" + hashlib.sha256(identity).hexdigest())
PY

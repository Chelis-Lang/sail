#!/usr/bin/env bash
# Builds a release-shaped Linux archive of this commit's Sail for the GitHub
# Action's hosted test (.github/workflows/ci.yml, `action-*` jobs), which
# installs it through the action's `archive` input.
#
#   ci/github-action-archive.sh E2E_BIN_TAR OUT_DIR
#
# CI never builds the engine runtime or a release (ADR 0013), so the archive
# is the newest published release, renamed to this commit's version, with
# bin/sail and bin/sail-z3 replaced by the ones `ci/run.sh e2e-build` built.
# That release must bundle the engine runtime this commit pins: the script
# refuses otherwise (after a re-pin, until the next release). The release index (release-files.json) binds the published
# bin/sail, so Sail declines the archive's shipped vstd proof and verifies
# vstd itself, as for any unqualified installation. This is test packaging
# only; it is never published.
set -euo pipefail
cd "$(dirname "$0")/.."
binaries="${1:?usage: ci/github-action-archive.sh E2E_BIN_TAR OUT_DIR}"
out="${2:?usage: ci/github-action-archive.sh E2E_BIN_TAR OUT_DIR}"
target=x86_64-unknown-linux-gnu
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
base="${SAIL_ACTION_BASE_RELEASE:-$(gh release list --repo Chelis-Lang/sail --exclude-drafts \
  --exclude-pre-releases --limit 1 --json tagName --jq '.[0].tagName')}"
base="${base#v}"
published="sail-$base-$target"
name="sail-$version-$target"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/download" "$out"
tar -xf "$binaries" -C "$work/bin"
(cd "$work/bin" && sha256sum --quiet -c e2e-bin.sha256)
reported="$("$work/bin/sail" --version)"
[ "$reported" = "sail $version" ] || { echo "built $reported, not sail $version" >&2; exit 1; }

gh release download "v$base" --repo Chelis-Lang/sail --dir "$work/download" \
  --pattern "$published.tar.gz" --pattern "$published.tar.gz.sha256"
(cd "$work/download" && sha256sum --quiet -c "$published.tar.gz.sha256")
tar -xzf "$work/download/$published.tar.gz" -C "$work"
[ "$published" = "$name" ] || mv "$work/$published" "$work/$name"

# Same engine runtime as this commit pins, by the pinned manifest digest.
python3 - "$work/$name" engine/verus/artifact-linux.json <<'PY'
import json, sys
from pathlib import Path
index = json.loads((Path(sys.argv[1]) / "release-files.json").read_bytes())
pin = json.loads(Path(sys.argv[2]).read_bytes())
if index["proof"]["runtime_manifest_sha256"] != pin["manifest_sha256"]:
    sys.exit("the newest release bundles another engine runtime than this commit pins")
PY

install -m 755 "$work/bin/sail" "$work/bin/sail-z3" "$work/$name/bin/"
tar -C "$work" --owner=0 --group=0 --numeric-owner -czf "$out/$name.tar.gz" "$name"
(cd "$out" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "$out/$name.tar.gz"

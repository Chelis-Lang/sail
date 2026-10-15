#!/usr/bin/env bash
# Builds Sail's release tarballs and uploads them to a DRAFT GitHub release
# (docs/releasing.md). Local only: CI never builds or publishes releases.
#
#   scripts/release.sh VERSION       build both platforms, package, draft release
#   scripts/release.sh stage OUT ENGINE_BUILD
#                                    copy this checkout's release binaries
#                                    (target/release) and Sail's Verus (the
#                                    validated fork runtime, which is also the
#                                    repair engine) into OUT, then self-check it
#
# VERSION must start with the workspace version in Cargo.toml (for example
# 0.1.0 or 0.1.0-rc.1); the binaries are built with SAIL_VERSION=VERSION so
# `sail --version` reports it exactly. Run on an Apple-silicon Mac with a clean checkout
# of a commit already on origin. The Linux build runs on a native x86-64 host
# (scripts/perf-host.sh, lane $SAIL_RELEASE_LANE, default `release`), inside
# the CI image, whose engine runtime is the pinned artifact
# (engine/verus/artifact-linux.json). The macOS engine is the pinned runtime
# in engine/verus/artifact-macos.json (built locally from the pinned fork
# commit, qualified per docs/validation/macos-engine.md, and published on the
# public fork's releases). Every archive ships the runtime: Sail runs no other
# Verus (ADR 0043).
#
# Never publishes: the release stays a draft until the owner publishes it.
set -euo pipefail
cd "$(dirname "$0")/.."

repo="Chelis-Lang/sail"
die() { echo "release: $*" >&2; exit 1; }

# The runtime variant directory a sail binary looks for beside itself
# (sail_verus::engine::bundled_dir), checked through the binary's own
# discovery with an empty cache and no SAIL_VERUS_BUILD, so nothing else can
# satisfy it.
self_check() {
  local out="$1" scratch status
  local reported
  reported="$("$out/bin/sail" --version)"
  if [ -n "${SAIL_VERSION:-}" ] && [ "$reported" != "sail $SAIL_VERSION" ]; then
    die "sail reports \"$reported\", not \"sail $SAIL_VERSION\""
  fi
  [ -x "$out/bin/sail-z3" ] || die "sail-z3 missing from $out"
  scratch="$(mktemp -d)"
  status="$(env -u SAIL_VERUS_BUILD SAIL_CACHE_DIR="$scratch" "$out/bin/sail" engine status --output json)" \
    || { rm -rf "$scratch"; die "the bundled Verus runtime in $out failed validation"; }
  rm -rf "$scratch"
  case "$status" in
    *"\"build\":\"$(cd "$out/engine" && pwd -P)/"*) ;;
    *) die "sail did not select the bundled engine: $status" ;;
  esac
  echo "$status"
}

# stage OUT ENGINE_BUILD: bin/{sail,sail-z3} and engine/<variant>/ holding
# exactly the runtime files the manifest binds (sail-build.json and
# verus/source/target-verus/release/*). The variant name is the build's own
# (resolved) directory name, which sail requires to match its host and pins.
stage() {
  local out="${1:?usage: scripts/release.sh stage OUT ENGINE_BUILD}"
  local build="${2:?stage needs ENGINE_BUILD: every archive ships the Sail Verus runtime, ADR 0043}"
  local runtime=verus/source/target-verus/release variant
  rm -rf "$out"
  mkdir -p "$out/bin"
  cp target/release/sail target/release/sail-z3 "$out/bin/"
  chmod 755 "$out/bin/sail" "$out/bin/sail-z3"
  # Drop the release profile's line tables from the shipped binaries (symbols
  # stay, so backtraces still name functions). Only Sail's own binaries: the
  # engine runtime files are bound to their manifest digests.
  strip -S "$out/bin/sail" "$out/bin/sail-z3"
  build="$(cd "$build" && pwd -P)"
  variant="$(basename "$build")"
  mkdir -p "$out/engine/$variant/$runtime"
  cp "$build/sail-build.json" "$out/engine/$variant/"
  find "$build/$runtime/" -maxdepth 1 \( -type f -o -type l \) -exec cp -L {} "$out/engine/$variant/$runtime/" \;
  self_check "$out"
}

# package DIR NAME: NAME.tar.gz (top directory NAME, root-owned entries) and
# NAME.tar.gz.sha256 in `shasum -a 256` format, in dist/.
package() {
  local dir="$1" name="$2" parent
  parent="$(mktemp -d)"
  cp -R "$dir" "$parent/$name"
  tar -C "$parent" --uid 0 --gid 0 --uname root --gname root -czf "dist/$name.tar.gz" "$name"
  rm -rf "$parent"
  (cd dist && shasum -a 256 "$name.tar.gz" >"$name.tar.gz.sha256")
}

release() {
  local version="$1" workspace commit lane
  workspace="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
  case "$version" in "$workspace"|"$workspace"-*) ;; *) die "VERSION must start with the workspace version $workspace" ;; esac
  [ "$(uname -s)-$(uname -m)" = Darwin-arm64 ] || die "run on an Apple-silicon Mac (it builds aarch64-apple-darwin locally)"
  [ -z "$(git status --porcelain)" ] || die "the checkout is not clean"
  commit="$(git rev-parse HEAD)"
  git fetch -q origin
  git branch -r --contains "$commit" | grep -q . || die "commit $commit is not on origin"
  if gh release view "v$version" --repo "$repo" >/dev/null 2>&1; then
    die "release v$version already exists"
  fi
  rm -rf dist && mkdir -p dist
  # The binaries report the exact release version (`sail --version`), and
  # self_check requires it.
  export SAIL_VERSION="$version"

  # Linux: build and stage inside the CI image on a native host.
  lane="${SAIL_RELEASE_LANE:-release}"
  case "$lane" in ""|*[!a-z0-9-]*) die "SAIL_RELEASE_LANE must be [a-z0-9-]" ;; esac
  if ! [ -f "$HOME/.sail-perf/$lane.env" ]; then
    # A host this script starts is torn down however it exits. The trap
    # outlives this function, so it carries the lane's value, not the local.
    # shellcheck disable=SC2064
    trap "scripts/perf-host.sh down $lane" EXIT
    scripts/perf-host.sh up "$lane"
  fi
  scripts/perf-host.sh sync "$lane"
  scripts/perf-host.sh run "$lane" env SAIL_RELEASE_OUT=release-out SAIL_VERSION="$version" ci/local.sh release
  mkdir -p dist/stage/linux
  scripts/perf-host.sh ssh "$lane" tar -C sail/release-out -cf - . | tar -C dist/stage/linux -xf -
  package dist/stage/linux "sail-$version-x86_64-unknown-linux-gnu"

  # macOS: build and stage here.
  cargo build --release --locked -p sail-cli -p sail-shim
  # The pinned, qualified runtime (engine/verus/artifact-macos.json) from the
  # fork's public release; install checks its digest and every file.
  local build
  build="$(python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-macos.json --cache dist/engine)"
  stage dist/stage/macos "$build"
  rm -rf dist/engine
  package dist/stage/macos "sail-$version-aarch64-apple-darwin"
  rm -rf dist/stage

  gh release create "v$version" --repo "$repo" --draft --target "$commit" \
    --title "Sail $version" \
    --notes "Draft release built by scripts/release.sh from $commit.

Assets: one tarball per platform with a .sha256 file. Each holds bin/sail, bin/sail-z3 and Sail's Verus (built from Sail's Verus fork; also the repair engine) under engine/. Users need only a Rust toolchain besides. See docs/releasing.md." \
    dist/*.tar.gz dist/*.tar.gz.sha256
  gh release view "v$version" --repo "$repo" --json url,isDraft,assets \
    --jq '"\(.url) draft=\(.isDraft)", (.assets[] | "  \(.name) \(.size)")'
}

case "${1:-}" in
  stage) shift; stage "$@" ;;
  ""|-h|--help) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
  *) release "$1" ;;
esac

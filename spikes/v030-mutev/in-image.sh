#!/usr/bin/env bash
# Spike A1: runs CMD in the CI image as ci/local.sh does (same image, same
# target and cache volumes), with ~/mutev mounted at /mutev for results.
#
#   spikes/v030-mutev/in-image.sh CMD...
set -euo pipefail
cd "$(dirname "$0")/../.."
image="sail-ci:$(ci/image-tag.sh core)"
mkdir -p "$HOME/mutev"
exec docker run --rm --platform linux/amd64 \
  -v "$PWD:/work" -w /work \
  -v "$(ci/target-volume.sh):/work/target" \
  -v sail-ci-cache:/root/.cache/sail \
  -v "$HOME/mutev:/mutev" \
  "$image" "$@"

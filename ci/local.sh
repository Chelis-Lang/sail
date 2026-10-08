#!/usr/bin/env bash
# Runs a CI step (see ci/run.sh) locally, in the same image CI uses.
#
#   ci/local.sh gate
#   ci/local.sh solver
#   ci/local.sh e2e
#   ci/local.sh conformance fast pr
#
# The image is linux/amd64 (the reference platform; Verus publishes no Linux
# arm64 build), so on Apple silicon it runs under emulation: correct, but
# slower than CI. The build directory and sail's cache live in Docker
# volumes, so they never mix with the host's own `target/`.
set -euo pipefail
cd "$(dirname "$0")/.."
image="sail-ci:$(ci/image-tag.sh)"
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker buildx build --platform linux/amd64 -f ci/Dockerfile -t "$image" --load .
fi
tty=()
[ -t 1 ] && tty=(-t)
docker run --rm ${tty[@]+"${tty[@]}"} --platform linux/amd64 \
  -v "$PWD:/work" -w /work \
  -v sail-ci-target:/work/target \
  -v sail-ci-cache:/root/.cache/sail \
  "$image" ci/run.sh "$@"

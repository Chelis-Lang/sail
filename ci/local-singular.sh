#!/usr/bin/env bash
# Optional local Singular environment, separate from every required CI gate.
set -euo pipefail
cd "$(dirname "$0")/.."
sail_target_volume="$(ci/target-volume.sh singular)"
if [ "${1:-}" = target-volume ]; then
  [ "$#" -eq 1 ] || { echo 'usage: ci/local-singular.sh target-volume' >&2; exit 2; }
  echo "$sail_target_volume"
  exit 0
fi
if [ "${CI:-}" = true ]; then
  echo "Singular validation is local-only (ADR 0013)" >&2
  exit 2
fi
base="sail-ci:$(ci/image-tag.sh)"
image="sail-singular:$(ci/image-tag.sh)"
if ! docker image inspect "$base" >/dev/null 2>&1; then
  docker buildx build --platform linux/amd64 -f ci/Dockerfile -t "$base" --load .
fi
docker buildx build --platform linux/amd64 --build-arg "SAIL_CI_IMAGE=$base" -f ci/Dockerfile.singular -t "$image" --load .
docker run --rm --platform linux/amd64 -v "$PWD:/work" -w /work \
  -v "$sail_target_volume:/work/target" -v sail-singular-cache:/root/.cache/sail \
  "$image" ci/run.sh singular "$@"

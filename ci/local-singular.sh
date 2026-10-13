#!/usr/bin/env bash
# Optional local Singular environment, separate from every required CI gate.
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${CI:-}" = true ]; then
  echo "Singular validation is local-only (ADR 0013)" >&2
  exit 2
fi
base="sail-ci:$(ci/image-tag.sh)"
image="sail-singular:$(ci/image-tag.sh)"
if ! docker image inspect "$base" >/dev/null 2>&1; then
  sail_engine_token="${SAIL_ENGINE_ARTIFACT_TOKEN:-}"
  if [ -z "$sail_engine_token" ]; then
    sail_engine_token="$(gh auth token)"
  fi
  SAIL_ENGINE_ARTIFACT_TOKEN="$sail_engine_token" docker buildx build \
    --secret id=sail_engine_token,env=SAIL_ENGINE_ARTIFACT_TOKEN \
    --platform linux/amd64 -f ci/Dockerfile -t "$base" --load .
  unset sail_engine_token
fi
docker buildx build --platform linux/amd64 --build-arg "SAIL_CI_IMAGE=$base" -f ci/Dockerfile.singular -t "$image" --load .
docker run --rm --platform linux/amd64 -v "$PWD:/work" -w /work \
  -v sail-singular-target:/work/target -v sail-singular-cache:/root/.cache/sail \
  "$image" ci/run.sh singular "$@"

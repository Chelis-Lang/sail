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
  sail_engine_token="${SAIL_ENGINE_ARTIFACT_TOKEN:-}"
  if [ -z "$sail_engine_token" ]; then
    sail_engine_token="$(gh auth token)"
  fi
  SAIL_ENGINE_ARTIFACT_TOKEN="$sail_engine_token" docker buildx build \
    --secret id=sail_engine_token,env=SAIL_ENGINE_ARTIFACT_TOKEN \
    --platform linux/amd64 -f ci/Dockerfile -t "$image" --load .
  unset sail_engine_token
fi
sail_runtime_env=()
for sail_env_name in CI SAIL_UNIT_TIMEOUT SAIL_VERUS_RELEASE SAIL_CONFORMANCE_OUT SAIL_BUILD_JOBS SAIL_BENCH_LABEL; do
  if sail_env_value="$(printenv "$sail_env_name")"; then
    sail_runtime_env+=(-e "$sail_env_name=$sail_env_value")
  fi
done
tty=()
[ -t 1 ] && tty=(-t)
docker run --rm ${tty[@]+"${tty[@]}"} ${sail_runtime_env[@]+"${sail_runtime_env[@]}"} --platform linux/amd64 \
  -v "$PWD:/work" -w /work \
  -v sail-ci-target:/work/target \
  -v sail-ci-cache:/root/.cache/sail \
  "$image" ci/run.sh "$@"

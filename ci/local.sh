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
# Only these phases invoke the real external MCP client. The core target keeps
# every pinned Rust, verifier, solver and corpus tool unchanged.
sail_image_target=core
if [ "${1:-}" = e2e ]; then
  case "${2:-full}" in full|managed) sail_image_target=mcp ;; esac
fi
image="sail-ci:$(ci/image-tag.sh "$sail_image_target")"
if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker buildx build --platform linux/amd64 -f ci/Dockerfile \
    --target "$sail_image_target" -t "$image" --load .
fi
sail_runtime_env=()
for sail_env_name in CI SAIL_UNIT_TIMEOUT SAIL_VERUS_RELEASE SAIL_CONFORMANCE_OUT SAIL_RELEASE_OUT SAIL_VERSION SAIL_BUILD_JOBS SAIL_BENCH_LABEL SAIL_E2E_OUTPUT_OUT; do
  if sail_env_value="$(printenv "$sail_env_name")"; then
    sail_runtime_env+=(-e "$sail_env_name=$sail_env_value")
  fi
done
if [ -n "${SAIL_DEPENDENCY_READ_TOKEN:-}" ]; then
  sail_runtime_env+=(-e SAIL_DEPENDENCY_READ_TOKEN)
fi
tty=()
[ -t 1 ] && tty=(-t)
sail_fault_options=()
# Match the one CI job whose real fault control needs a private mount namespace.
if [ "${1:-}" = solver ]; then
  sail_fault_options=(--cap-add SYS_ADMIN --security-opt seccomp=unconfined --security-opt apparmor=unconfined)
fi
docker run --rm ${tty[@]+"${tty[@]}"} ${sail_runtime_env[@]+"${sail_runtime_env[@]}"} ${sail_fault_options[@]+"${sail_fault_options[@]}"} --platform linux/amd64 \
  -v "$PWD:/work" -w /work \
  -v sail-ci-target:/work/target \
  -v sail-ci-cache:/root/.cache/sail \
  "$image" ci/run.sh "$@"

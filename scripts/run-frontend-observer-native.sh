#!/usr/bin/env bash
# Execute only inside the newly allocated native host, with an immutable payload.
# The caller must wrap this entire script in timeout --kill-after=60 5400.
set -euo pipefail
payload="$(realpath "${1:?immutable payload directory}")"
work="${2:?new owned work directory}"
test ! -e "$work"
mkdir -p "$work"
work="$(realpath "$work")"
image=sail-m8-observer-ci
container=sail-m8-observer-capture
cleanup() {
  local status=$?
  docker rm -f "$container" >/dev/null 2>&1 || true
  printf '%s\n' "$status" >"$work/parent.exit"
}
trap cleanup EXIT
trap 'exit 143' TERM
trap 'exit 130' INT
uname -a >"$work/native-environment.txt"
df -h "$work" >>"$work/native-environment.txt"
docker build -f "$payload/ci-context/ci/Dockerfile" -t "$image" "$payload/ci-context" \
  >"$work/image-build.log" 2>&1
docker image inspect "$image" >"$work/image-identity.json"
docker run --name "$container" --rm --cpus 16 --memory 28g \
  -v "$payload:/payload:ro" -v "$work:/m8" -w /m8 \
  -v sail-m8-observer-rustup:/opt/rustup \
  -v sail-m8-observer-cargo:/opt/cargo \
  "$image" bash -euc '
    python3 /payload/build-frontend-observer.py \
      --base-bundle /payload/combined-8a.bundle \
      --observer-bundle /payload/observer-7812863f.bundle \
      --z3 /opt/verus/verus-x86-linux/z3 --work /m8/build --install-toolchain
    for project in vest ironkv; do
      # Each controller retains failed scope and final restoration; continue
      # to the second project so the final index never drops a failed sample.
      status=0
      python3 /payload/capture-frontend-inputs.py --project "$project" \
        --verus-source /m8/build/verus \
        --verus-source-commit 7812863f152ed21f0a1b36711ac33130c8f1183b \
        --runtime /m8/build/build-results/runtime \
        --runtime-manifest /m8/build/build-results/runtime-files.json \
        --patcher /payload/patcher \
        --patcher-sha256 667c9bdecb76d0a09a80e615ad66825d77e323b8dc4be085396e45467b531dfd \
        --work "/m8/capture-$project" || status=$?
      printf "%s\n" "$status" >"/m8/capture-$project.exit"
    done
    test "$(cat /m8/capture-vest.exit)" = 0
    test "$(cat /m8/capture-ironkv.exit)" = 0
  '

#!/usr/bin/env bash
# The CI snippet documented for `sail report` must work in the shells GitHub
# Actions uses for a `run:` step: `bash -e` by default, and `bash --noprofile
# --norc -eo pipefail` under `shell: bash`. Both stop at the first failing
# command, so a snippet that reads `$?` on the line after `sail verify` never
# reaches `sail report` in the failing case the report exists for.
#
# This extracts the snippet from the guide and from the consumer skill and
# runs each one, in both shells, with a `sail` on PATH whose `verify` prints a
# document and exits with a chosen status. `sail report` is a stub that
# records its call, or the real binary when SAIL_BIN names one (then the
# documents are the `sail report` fixtures and the output is checked too).
set -euo pipefail
cd "$(dirname "$0")/.."
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fixtures=crates/sail-cli/tests/fixtures/ci-report
passed=0

fail() {
  printf 'ci report snippet (%s): %s\n' "$label" "$1" >&2
  for file in "$scratch/run/stdout" "$scratch/run/stderr" "$scratch/run/calls"; do
    [ -f "$file" ] && { printf -- '--- %s\n' "$file" >&2; cat "$file" >&2; }
  done
  exit 1
}

# snippet FILE: the first fenced `sh` block of FILE that runs `sail verify
# --output json`.
snippet() {
  awk '
    /^```sh$/ { inside = 1; block = ""; next }
    /^```$/ && inside { inside = 0; if (block ~ /sail verify --output json/) { printf "%s", block; exit } next }
    inside { block = block $0 "\n" }
  ' "$1"
}

mkdir "$scratch/bin"
cat >"$scratch/bin/sail" <<'E'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$SAIL_TEST_RUN/calls"
case "$1" in
  verify)
    [ -n "$SAIL_TEST_DOCUMENT" ] && cat "$SAIL_TEST_DOCUMENT"
    exit "$SAIL_TEST_VERIFY_EXIT"
    ;;
  report)
    if [ -n "${SAIL_BIN:-}" ]; then
      exec "$SAIL_BIN" "$@"
    fi
    # The stub reads its document as the real command does.
    for last; do :; done
    [ -s "$last" ] || { echo 'error[SAIL0908]: sail report: stub: empty document' >&2; exit 2; }
    printf 'report %s\n' "$2 $3"
    ;;
  *) exit 64 ;;
esac
E
chmod +x "$scratch/bin/sail"

# run SCRIPT VERIFY_EXIT DOCUMENT SHELL...: runs SCRIPT as a workflow step.
run() {
  local script="$1" verify_exit="$2" document="$3"
  shift 3
  rm -rf "$scratch/run" && mkdir "$scratch/run"
  status=0
  (
    cd "$scratch/run"
    PATH="$scratch/bin:$PATH" SAIL_TEST_RUN="$scratch/run" \
      SAIL_TEST_VERIFY_EXIT="$verify_exit" SAIL_TEST_DOCUMENT="$document" \
      GITHUB_STEP_SUMMARY="$scratch/run/summary.md" \
      "$@" "$script" >stdout 2>stderr
  ) || status=$?
}

reports() { grep -c '^report ' "$scratch/run/calls" || true; }

for source in docs/ci-reports.md skills/sail-verification/references/verus.md; do
  snippet "$source" >"$scratch/step.sh"
  [ -s "$scratch/step.sh" ] || { label="$source"; fail 'no snippet running sail verify --output json found'; }
  wanted=$(grep -c '^sail report ' "$scratch/step.sh")
  [ "$wanted" -ge 2 ] || { label="$source"; fail 'the snippet does not call sail report'; }
  for shell in 'bash -e' 'bash --noprofile --norc -eo pipefail'; do
    # Verification failed: every report is written and the step fails with
    # `sail verify`'s own exit code.
    label="$source, $shell, failing"
    # shellcheck disable=SC2086 # the shell and its options are separate words
    run "$scratch/step.sh" 101 "$PWD/$fixtures/failing.json" $shell
    [ "$status" = 101 ] || fail "step exit $status, expected 101"
    [ "$(reports)" = "$wanted" ] || fail "$(reports) of $wanted reports ran"
    [ -s "$scratch/run/summary.md" ] || fail 'no job summary was written'
    if [ -n "${SAIL_BIN:-}" ]; then
      grep -q '^## Sail verification: failed$' "$scratch/run/summary.md" || fail 'the summary does not say failed'
      grep -q '^::error file=' "$scratch/run/stdout" || fail 'no error annotation on stdout'
    fi
    passed=$((passed + 1))

    # Verification passed: the step passes.
    label="$source, $shell, passing"
    # shellcheck disable=SC2086
    run "$scratch/step.sh" 0 "$PWD/$fixtures/passing.json" $shell
    [ "$status" = 0 ] || fail "step exit $status, expected 0"
    [ "$(reports)" = "$wanted" ] || fail "$(reports) of $wanted reports ran"
    if [ -n "${SAIL_BIN:-}" ]; then
      grep -q '^## Sail verification: passed$' "$scratch/run/summary.md" || fail 'the summary does not say passed'
    fi
    passed=$((passed + 1))

    # `sail verify` failed without printing a document: `sail report` cannot
    # read one, and the step still fails.
    label="$source, $shell, no document"
    # shellcheck disable=SC2086
    run "$scratch/step.sh" 2 '' $shell
    [ "$status" != 0 ] || fail 'the step passed although sail verify failed'
    passed=$((passed + 1))
  done
done

# Control: the shape this test exists to keep out. Under `bash -e` it stops at
# `sail verify`, so no report runs. If this starts passing, the test above no
# longer distinguishes the two shapes.
cat >"$scratch/old.sh" <<'E'
sail verify --output json > result.json; code=$?
sail report --format github-summary result.json >> "$GITHUB_STEP_SUMMARY"
exit "$code"
E
label='control'
run "$scratch/old.sh" 101 "$PWD/$fixtures/failing.json" bash -e
[ "$status" = 101 ] || fail "step exit $status, expected 101"
[ "$(reports)" = 0 ] || fail 'the control snippet reached sail report under bash -e'
passed=$((passed + 1))

printf 'ci report snippet: %s checks passed\n' "$passed"

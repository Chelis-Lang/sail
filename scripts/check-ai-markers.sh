#!/usr/bin/env bash
# Rejects AI tool authorship markers in commits. The single source of truth for
# both the local commit-msg hook (.githooks/commit-msg) and the CI job
# "No AI authorship markers" (.github/workflows/ci.yml).
#
# Copied from Chelis-Lang/dive. Patterns mirror Chelis-Lang/chelis
# scripts/check_commit_message.py and the org CI guard, plus explicit noreply
# addresses.
#
# Usage:
#   check-ai-markers.sh message <file>        check one commit message file
#   check-ai-markers.sh range <base> <head>   check every commit in base..head
#   check-ai-markers.sh all <head>            check every commit reachable from head
set -euo pipefail

TOKENS='claude|anthropic|kiro|openai|codex|chatgpt|copilot'
# Word boundaries written portably (BSD and GNU grep -E).
WB_L='(^|[^[:alnum:]])'
WB_R='([^[:alnum:]]|$)'
MSG_PATTERNS=(
  "(co-)?authored-by:.*(${TOKENS})"
  "(co-)?authored-by:.*${WB_L}(ai|llm|assistant)${WB_R}"
  "generated (with|by).*((${TOKENS})|gpt-[0-9]+|${WB_L}ai${WB_R}|${WB_L}llm${WB_R})"
  "noreply@(anthropic|openai)\\.com"
)
IDENTITY_PATTERN="(${TOKENS})"

# check_text <label> <text>: prints offending patterns, returns 1 on a match.
check_message() {
  local label="$1" text="$2" found=0 pat
  for pat in "${MSG_PATTERNS[@]}"; do
    if printf '%s\n' "$text" | grep -qiE -- "$pat"; then
      echo "FAIL: ${label}: message matches: ${pat}" >&2
      found=1
    fi
  done
  return "$found"
}

check_commit() {
  local sha="$1" found=0 who
  check_message "$sha" "$(git log -1 --format=%B "$sha")" || found=1
  for who in "$(git log -1 --format='%an <%ae>' "$sha")" "$(git log -1 --format='%cn <%ce>' "$sha")"; do
    if printf '%s\n' "$who" | grep -qiE -- "$IDENTITY_PATTERN"; then
      echo "FAIL: ${sha}: author/committer identity matches: ${who}" >&2
      found=1
    fi
  done
  return "$found"
}

check_revs() {
  local found=0 sha count=0
  while read -r sha; do
    [ -n "$sha" ] || continue
    count=$((count + 1))
    check_commit "$sha" || found=1
  done < <(git rev-list "$@")
  if [ "$found" -ne 0 ]; then
    echo "Remove AI authorship markers from the flagged commits." >&2
    return 1
  fi
  echo "All ${count} commit(s) clean."
}

case "${1:-}" in
  message)
    [ $# -eq 2 ] || { echo "usage: $0 message <file>" >&2; exit 2; }
    # Ignore git's comment lines (the commented diff in verbose mode, etc.).
    if ! check_message "commit message" "$(grep -v '^#' -- "$2" || true)"; then
      echo "Remove AI tool attribution lines from the commit message." >&2
      exit 1
    fi
    ;;
  range)
    [ $# -eq 3 ] || { echo "usage: $0 range <base> <head>" >&2; exit 2; }
    check_revs "$2..$3"
    ;;
  all)
    [ $# -eq 2 ] || { echo "usage: $0 all <head>" >&2; exit 2; }
    check_revs "$2"
    ;;
  *)
    echo "usage: $0 {message <file>|range <base> <head>|all <head>}" >&2
    exit 2
    ;;
esac

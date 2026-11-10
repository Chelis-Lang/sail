#!/usr/bin/env bash
# Lints every workflow with actionlint, including the inputs each workflow
# passes to the local GitHub Action (action/action.yml) and the shell in
# every `run:` step when shellcheck is installed. The pinned release is
# checked against its published SHA-256 before it runs.
set -euo pipefail
cd "$(dirname "$0")/.."
version=1.7.12
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) platform=linux_amd64 sha256=8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8 ;;
  *) echo "check-workflows: no pinned actionlint for $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="actionlint_${version}_${platform}.tar.gz"
curl -fsSL --retry 3 -o "$work/$archive" \
  "https://github.com/rhysd/actionlint/releases/download/v$version/$archive"
echo "$sha256  $work/$archive" | sha256sum --quiet -c -
tar -xzf "$work/$archive" -C "$work" actionlint
"$work/actionlint" -color

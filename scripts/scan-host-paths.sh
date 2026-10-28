#!/usr/bin/env bash
# Scans every file of an unpacked runtime archive for host paths before it is
# published (scripts/build-macos-runtime.sh, docs/releasing.md).
#
#   scripts/scan-host-paths.sh DIR [TOKEN...]
#
# Searches the complete bytes of each regular file, and the target of each
# symbolic link, for `/Users/`, `/home/` and every TOKEN (fixed strings). It
# fails closed: any entry it cannot enumerate or read, any other file type and
# an empty tree are errors, never a clean result.
#
# Exit status: 0 clean, 1 at least one hit (each printed as `scan: TOKEN in
# PATH`), 2 the scan could not be completed.
set -euo pipefail
export LC_ALL=C

fail() { echo "scan: $*; the archive is not checked" >&2; exit 2; }

[ $# -ge 1 ] || { echo "usage: $0 DIR [TOKEN...]" >&2; exit 2; }
dir="$1"
shift
[ -d "$dir" ] || fail "$dir is not a directory"
tokens=(/Users/ /home/ "$@")
for t in "${tokens[@]}"; do [ -n "$t" ] || fail "empty token"; done

list="$(mktemp)"
trap 'rm -f "$list"' EXIT
# Enumerate first, into a file, so a find failure is seen (a process
# substitution's status would be lost).
find "$dir" -mindepth 1 ! -type d -print0 >"$list" || fail "cannot enumerate $dir"

# has TOKEN [FILE]: grep's own status, with anything but match/no match fatal.
# grep reads the file itself (no pipeline), so an early match cannot be lost to
# SIGPIPE; with no FILE it reads stdin.
has() {
  local s=0
  if [ $# -ge 2 ]; then grep -qaF -e "$1" -- "$2" || s=$?; else grep -qaF -e "$1" || s=$?; fi
  [ "$s" -le 1 ] || fail "cannot read ${2:-link target} (grep status $s)"
  return "$s"
}

hits=0 count=0
while IFS= read -r -d '' f; do
  count=$((count + 1))
  rel="${f#"$dir"/}"
  if [ -L "$f" ]; then
    target="$(readlink -- "$f")" || fail "cannot read link $rel"
    for t in "${tokens[@]}"; do
      if has "$t" <<<"$target"; then echo "scan: $t in $rel (link target)"; hits=1; fi
    done
  elif [ -f "$f" ]; then
    [ -r "$f" ] || fail "cannot read $rel"
    for t in "${tokens[@]}"; do
      if has "$t" "$f"; then echo "scan: $t in $rel"; hits=1; fi
    done
  else
    fail "unexpected file type: $rel"
  fi
done <"$list"

[ "$count" -gt 0 ] || fail "$dir has no files"
[ "$hits" = 0 ] || exit 1
echo "scan: clean ($count files)"

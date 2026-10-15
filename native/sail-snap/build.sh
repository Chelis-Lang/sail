#!/usr/bin/env bash
# Builds the sail-snap preload library (prototype, ADR 0048 draft).
#
#   native/sail-snap/build.sh [OUT_DIR]     default: target/sail-snap
#
# Prints the library's path. Linux: libsail_snap.so (LD_PRELOAD); macOS:
# libsail_snap.dylib (DYLD_INSERT_LIBRARIES).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="${1:-$here/../../target/sail-snap}"
mkdir -p "$out"
cc="${CC:-cc}"
case "$(uname -s)" in
  Linux)
    "$cc" -O2 -Wall -Wextra -fPIC -shared -o "$out/libsail_snap.so" "$here/sail_snap.c" -ldl
    lib="$out/libsail_snap.so" ;;
  Darwin)
    "$cc" -O2 -Wall -Wextra -dynamiclib -o "$out/libsail_snap.dylib" "$here/sail_snap.c"
    lib="$out/libsail_snap.dylib" ;;
  *) echo "unsupported platform $(uname -s)" >&2; exit 1 ;;
esac
echo "$(cd "$(dirname "$lib")" && pwd)/$(basename "$lib")"

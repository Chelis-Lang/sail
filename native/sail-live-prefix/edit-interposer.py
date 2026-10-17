#!/usr/bin/env python3
"""Explicit disabled research VERUS_Z3_PATH; never the pinned solver itself."""
import os
from pathlib import Path
import sys

from edit_transport import wrapper


def main():
    try:
        private = Path(os.environ['SAIL_LIVE_PREFIX_EDIT_CONFIG'])
        return wrapper(private, sys.argv[1:], sys.stdin.buffer, sys.stdout.buffer)
    except (OSError, ValueError, RuntimeError, KeyError):
        # A pre-first-record failure remains an explicit unresolved census gap.
        # The outer qualification must test actual Verus's no-reply behavior;
        # never reinterpret an ignored wrapper status as completed broker proof.
        sys.stderr.write('research edit interposer refused startup\n')
        return 1


if __name__ == '__main__':
    raise SystemExit(main())

#!/usr/bin/env python3
"""Diagnose JSON framing only; never substitutes for corpus conformance."""
import argparse
import json
from pathlib import Path
import sys


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON object key: " + key)
        result[key] = value
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stream", type=Path)
    args = parser.parse_args()
    try:
        text = args.stream.read_text()
    except (OSError, UnicodeError) as error:
        print(f"unreadable stream: {error}", file=sys.stderr)
        return 1
    decoder = json.JSONDecoder(object_pairs_hook=unique_object)
    offset = count = 0
    while offset < len(text):
        while offset < len(text) and text[offset].isspace():
            offset += 1
        if offset == len(text):
            break
        try:
            _, offset = decoder.raw_decode(text, offset)
        except ValueError as error:
            print(f"incomplete stream after {count} complete documents: {error}")
            return 1
        count += 1
    print(f"{count} complete documents; syntax/framing only, no verdict qualification")
    return 0 if count else 1


if __name__ == "__main__":
    sys.exit(main())

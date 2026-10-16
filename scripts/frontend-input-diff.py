#!/usr/bin/env python3
"""Describe ordinary AST JSON differences without constructing a reusable key.

Raw equality alone determines the successful exit status. Object pair order,
duplicate keys, array positions and original numeric tokens remain observable.
JSON whitespace/escape spelling differences also fail raw equality, even when
their parsed values match. The complete original files must stay with evidence.
"""

import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import sys

MAX_BYTES = 64 * 1024 * 1024
MAX_NODES = 2_000_000
MAX_DETAILS = 256


@dataclass(frozen=True)
class Number:
    token: str


@dataclass(frozen=True)
class Object:
    pairs: list


def reject_constant(value):
    raise ValueError(f"non-JSON numeric constant: {value}")


def decode(raw):
    if len(raw) > MAX_BYTES:
        raise ValueError("input exceeds 64 MiB")
    return json.loads(
        raw.decode("utf-8"),
        parse_int=Number,
        parse_float=Number,
        parse_constant=reject_constant,
        object_pairs_hook=Object,
    )


def pointer_component(value):
    return str(value).replace("~", "~0").replace("/", "~1")


def compare(left_raw, right_raw):
    left, right = decode(left_raw), decode(right_raw)
    counts = Counter()
    details = []
    visited = 0

    def changed(kind, pointer, **extra):
        counts[kind] += 1
        if len(details) < MAX_DETAILS:
            detail = {"kind": kind, "pointer": pointer[:1024], **extra}
            if len(pointer) > 1024:
                detail.update(pointer_truncated=True,
                              full_pointer_sha256=hashlib.sha256(pointer.encode()).hexdigest())
            details.append(detail)

    def walk(a, b, pointer):
        nonlocal visited
        visited += 1
        if visited > MAX_NODES:
            raise ValueError("comparison exceeds two million node pairs")
        if type(a) is not type(b):
            changed("type", pointer, left_type=type(a).__name__, right_type=type(b).__name__)
        elif isinstance(a, Object):
            a_keys, b_keys = [key for key, _ in a.pairs], [key for key, _ in b.pairs]
            if a_keys != b_keys:
                changed("object_pair_order_or_keys", pointer)
            if len(a.pairs) != len(b.pairs):
                changed("object_length", pointer, left_length=len(a.pairs), right_length=len(b.pairs))
            for index, ((a_key, a_value), (b_key, b_value)) in enumerate(zip(a.pairs, b.pairs)):
                if a_key != b_key:
                    changed("object_pair_key", pointer, pair_index=index,
                            left_key=a_key[:200], right_key=b_key[:200])
                # Index-based pairing deliberately retains ordering and duplicate
                # occurrences; it never aligns/sorts functions or span identifiers.
                child = f"{pointer}/{pointer_component(a_key)}"
                walk(a_value, b_value, child)
        elif isinstance(a, list):
            if len(a) != len(b):
                changed("array_length", pointer, left_length=len(a), right_length=len(b))
            for index, (a_value, b_value) in enumerate(zip(a, b)):
                walk(a_value, b_value, f"{pointer}/{index}")
        elif isinstance(a, Number):
            if a.token != b.token:
                changed("number_token", pointer, left=a.token[:200], right=b.token[:200])
        elif a != b:
            changed("value", pointer)

    walk(left, right, "")
    return {
        "version": 1,
        "analysis_only": True,
        "key_qualified": False,
        "raw_equal": left_raw == right_raw,
        "left": {"bytes": len(left_raw), "sha256": hashlib.sha256(left_raw).hexdigest()},
        "right": {"bytes": len(right_raw), "sha256": hashlib.sha256(right_raw).hexdigest()},
        "node_pairs_visited": visited,
        "difference_counts": dict(counts),
        "difference_details": details,
        "detail_limit": MAX_DETAILS,
        "details_truncated": sum(counts.values()) > len(details),
        "parsed_values_match_but_raw_bytes_differ": not counts and left_raw != right_raw,
        "limits": "Ordinary serde omits opaque RawSpan; this report cannot establish complete inputs.",
    }


def read_bounded(path):
    with path.open("rb") as source:
        raw = source.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError(f"{path}: input exceeds 64 MiB")
    return raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("left", type=Path)
    parser.add_argument("right", type=Path)
    args = parser.parse_args()
    try:
        result = compare(read_bounded(args.left), read_bounded(args.right))
    except (OSError, ValueError, RecursionError) as error:
        print(json.dumps({"analysis_only": True, "key_qualified": False,
                          "complete": False, "error": str(error)}))
        return 2
    print(json.dumps(result, indent=2))
    return 0 if result["raw_equal"] else 1


if __name__ == "__main__":
    sys.exit(main())

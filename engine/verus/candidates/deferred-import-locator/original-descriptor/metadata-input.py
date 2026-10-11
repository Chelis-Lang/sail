#!/usr/bin/env python3
"""Bounded metadata-suffix preflight; actual pinned serde decode follows separately.

This closed legacy-bincode schema is a allocation/work preflight, not a general
VIR decoder or an authority source. The sealed file, exact crate-id, unique
full-consumption suffix and actual pinned CrateMetadata decode are mandatory.
"""
import hashlib
from pathlib import Path
import struct

INPUT_LIMIT = 128 * 1024 * 1024
SUFFIX_LIMIT = 8 * 1024 * 1024
RECORD_LIMIT = 4096
PATH_LIMIT = 4096
CANDIDATE_LIMIT = 64
CRATE_ID = 2208441662476345389
EXPECTED_SHA = "ee8de6d4dedc584f2457938868dea48c839ad6117789d78b6784181dff5d9829"
SELECTED_HASH = bytes.fromhex("966a15d4a7885d0f3391df29d6d31a1d")
SELECTED_RANGE = (406973, 411571)


def parse_suffix(data, offset):
    cursor = offset
    def take(size):
        nonlocal cursor
        if size < 0 or cursor + size > len(data):
            raise ValueError("truncated metadata")
        out = memoryview(data)[cursor:cursor + size]
        cursor += size
        return out
    def number(size):
        return int.from_bytes(take(size), "little")
    if len(data) - offset > SUFFIX_LIMIT or number(8) != CRATE_ID:
        raise ValueError("unsupported metadata suffix")
    count = number(8)
    if count > RECORD_LIMIT:
        raise ValueError("metadata record cap")
    keys = set()
    ranges = set()
    selected = None
    for _ in range(count):
        length = number(8)
        if not 1 <= length <= 32:
            raise ValueError("metadata hash length")
        key = bytes(take(length))
        if key in keys:
            raise ValueError("duplicate metadata hash")
        keys.add(key)
        tag = number(1)
        if tag not in (0, 1):
            raise ValueError("metadata Option tag")
        filename = None
        if tag:
            length = number(8)
            if length > PATH_LIMIT:
                raise ValueError("metadata filename cap")
            filename = bytes(take(length)).decode("utf-8", "strict")
        start, end = number(4), number(4)
        if end < start or (start, end) in ranges:
            raise ValueError("metadata range")
        ranges.add((start, end))
        if key == SELECTED_HASH:
            if (start, end) != SELECTED_RANGE:
                raise ValueError("selected metadata range mismatch")
            selected = {"filename_present": filename is not None,
                        "filename_absolute": filename is not None and Path(filename).is_absolute(),
                        "filename_utf8_bytes": len(filename.encode()) if filename is not None else None,
                        "filename_nul_free": filename is not None and "\0" not in filename,
                        "filename_sha256": hashlib.sha256(filename.encode()).hexdigest() if filename is not None else None,
                        "original_start": start, "original_end": end,
                        "expected_hash_hex": key.hex()}
    if cursor != len(data) or selected is None:
        raise ValueError("metadata must consume complete suffix and contain selected original")
    return {"crate_id": CRATE_ID, "records": count, "selected": selected}


def bounded_input(path, expected_sha=EXPECTED_SHA):
    with path.open("rb") as stream:
        data = stream.read(INPUT_LIMIT + 1)
    if len(data) > INPUT_LIMIT:
        raise ValueError("sealed metadata input cap")
    digest = hashlib.sha256(data).hexdigest()
    if digest != expected_sha:
        raise ValueError("sealed vstd.vir SHA mismatch")
    marker = struct.pack("<Q", CRATE_ID)
    lower = max(0, len(data) - SUFFIX_LIMIT)
    cursor = len(data)
    candidate_count = 0
    raw_marker_count = 0
    accepted = None
    while True:
        offset = data.rfind(marker, lower, cursor)
        if offset < 0:
            break
        cursor = offset
        raw_marker_count += 1
        # The crate id also occurs in ordinary VIR span words. Only a fixed
        # schema header with an admitted record count is a decode candidate;
        # scan remains within 8 MiB, and full parser work stays <=64 candidates.
        if offset + 16 > len(data) or not 1 <= int.from_bytes(memoryview(data)[offset + 8:offset + 16], "little") <= RECORD_LIMIT:
            continue
        candidate_count += 1
        if candidate_count > CANDIDATE_LIMIT:
            raise ValueError("metadata suffix candidate cap")
        try:
            facts = parse_suffix(data, offset)
        except (ValueError, UnicodeError):
            continue
        if accepted is not None:
            raise ValueError("ambiguous full-consumption metadata suffix")
        accepted = (offset, facts)
    if accepted is None:
        raise ValueError("no unique bounded metadata suffix")
    offset, facts = accepted
    suffix = data[offset:]
    facts.update(input_sha256=digest, input_bytes=len(data), suffix_bytes=len(suffix),
                 suffix_sha256=hashlib.sha256(suffix).hexdigest(), candidate_count=candidate_count, raw_marker_count=raw_marker_count,
                 scope="closed-schema allocation/work preflight only; actual pinned serde type/control required")
    return suffix, facts

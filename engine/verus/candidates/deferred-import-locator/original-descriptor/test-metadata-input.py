#!/usr/bin/env python3
"""Malformed-length controls for the metadata preflight, no compiler/runtime use."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location("sail_metadata_input", Path(__file__).with_name("metadata-input.py"))
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


def metadata(path=b"/exact/original.rs", key=subject.SELECTED_HASH, start=406973, end=411571):
    return (struct.pack("<QQQ", subject.CRATE_ID, 1, len(key)) + key + b"\x01" +
            struct.pack("<Q", len(path)) + path + struct.pack("<II", start, end))


class MetadataBounds(unittest.TestCase):
    def test_exact_closed_shape_consumes_full_suffix(self):
        actual = subject.parse_suffix(metadata(), 0)
        self.assertEqual(actual["records"], 1)
        self.assertTrue(actual["selected"]["filename_absolute"])
        self.assertEqual(actual["selected"]["expected_hash_hex"], subject.SELECTED_HASH.hex())
        with self.assertRaises(ValueError):
            subject.parse_suffix(metadata() + b"extra", 0)

    def test_record_and_hash_hints_refuse_before_payload(self):
        for blob in (struct.pack("<QQ", subject.CRATE_ID, 2**64-1),
                     struct.pack("<QQQ", subject.CRATE_ID, 1, 2**64-1)):
            with self.assertRaises(ValueError):
                subject.parse_suffix(blob, 0)

    def test_path_hint_and_bad_option_refuse_before_payload(self):
        prefix = struct.pack("<QQQ", subject.CRATE_ID, 1, len(subject.SELECTED_HASH)) + subject.SELECTED_HASH
        for blob in (prefix + b"\x01" + struct.pack("<Q", 2**64-1), prefix + b"\x02"):
            with self.assertRaises(ValueError):
                subject.parse_suffix(blob, 0)

    def test_selected_original_hash_range_are_not_guessed(self):
        for blob in (metadata(key=b"different hash!!"), metadata(start=0, end=1), metadata(start=411571, end=406973)):
            with self.assertRaises(ValueError):
                subject.parse_suffix(blob, 0)


if __name__ == "__main__":
    unittest.main()

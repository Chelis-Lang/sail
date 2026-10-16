#!/usr/bin/env python3
"""Exact-field and byte-policy controls for the offline AST observer analysis."""

import importlib.util
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "frontend_input_diff", Path(__file__).with_name("frontend-input-diff.py")
)
DIFF = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DIFF
SPEC.loader.exec_module(DIFF)


class ExactDiffTests(unittest.TestCase):
    def test_span_fields_are_all_observed(self):
        original = b'{"span":{"id":7,"data":[1,2],"as_string":"old.rs:1"}}'
        edited = b'{"span":{"id":8,"data":[1,3],"as_string":"new.rs:2"}}'
        result = DIFF.compare(original, edited)
        self.assertFalse(result["raw_equal"])
        self.assertEqual(
            {entry["pointer"] for entry in result["difference_details"]},
            {"/span/id", "/span/data/1", "/span/as_string"},
        )

    def test_object_order_and_duplicate_occurrences_remain_visible(self):
        result = DIFF.compare(b'{"a":1,"b":2}', b'{"b":2,"a":1}')
        self.assertIn("object_pair_order_or_keys", result["difference_counts"])
        duplicate = DIFF.compare(b'{"a":1,"a":2}', b'{"a":1,"a":3}')
        self.assertEqual(duplicate["difference_counts"], {"number_token": 1})

    def test_number_tokens_and_boolean_types_are_not_coerced(self):
        for left, right in [(b"1", b"1.0"), (b"0", b"-0"), (b"1e0", b"1.0")]:
            self.assertEqual(DIFF.compare(left, right)["difference_counts"], {"number_token": 1})
        self.assertEqual(DIFF.compare(b"1", b"true")["difference_counts"], {"type": 1})

    def test_array_order_is_not_aligned(self):
        result = DIFF.compare(b'["f","g"]', b'["g","f"]')
        self.assertEqual(result["difference_counts"], {"value": 2})

    def test_formatting_and_string_escaping_cannot_pass_raw_gate(self):
        for left, right in [(b'{"x":1}', b'{ "x": 1 }'), (b'"a"', b'"\\u0061"')]:
            result = DIFF.compare(left, right)
            self.assertFalse(result["raw_equal"])
            self.assertTrue(result["parsed_values_match_but_raw_bytes_differ"])
        self.assertTrue(DIFF.compare(b'{"x":1}', b'{"x":1}')["raw_equal"])

    def test_refusals_and_detail_bound(self):
        for raw in [b"NaN", b"Infinity", b"{", b'"\xff"']:
            with self.assertRaises(ValueError):
                DIFF.compare(raw, raw)
        original_limit = DIFF.MAX_BYTES
        try:
            DIFF.MAX_BYTES = 2
            with self.assertRaises(ValueError):
                DIFF.compare(b"123", b"123")
        finally:
            DIFF.MAX_BYTES = original_limit
        result = DIFF.compare(b"[" + b",".join([b"1"] * 300) + b"]",
                              b"[" + b",".join([b"2"] * 300) + b"]")
        self.assertEqual(result["difference_counts"], {"number_token": 300})
        self.assertEqual(len(result["difference_details"]), DIFF.MAX_DETAILS)
        self.assertTrue(result["details_truncated"])

    def test_cli_raw_equal_different_and_refused_exit_statuses(self):
        with tempfile.TemporaryDirectory(prefix="sail-frontend-diff-") as directory:
            left, right = Path(directory) / "left.json", Path(directory) / "right.json"
            left.write_bytes(b'{"x":1}')
            for raw, expected in [(b'{"x":1}', 0), (b'{ "x": 1 }', 1), (b"NaN", 2)]:
                right.write_bytes(raw)
                child = subprocess.run([sys.executable, str(SPEC.origin), str(left), str(right)],
                                       capture_output=True, check=False, timeout=5)
                self.assertEqual(child.returncode, expected, child.stderr)
                self.assertIn(b'"key_qualified": false', child.stdout)


if __name__ == "__main__":
    unittest.main()

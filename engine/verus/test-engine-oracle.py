#!/usr/bin/env python3
"""Negative controls for the exact owner-approved termination inventory."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("engine_oracle", Path(__file__).with_name("test-engine.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class Inventory(unittest.TestCase):
    def test_exact_qualified_stock_and_fork_inventories(self):
        oracle.check_inventory("recursive", "stock", ["root.smt_transcript"], oracle.ISOLATED_RECURSIVE, True)
        oracle.check_inventory("recursive", "fork", oracle.ISOLATED_RECURSIVE, oracle.ISOLATED_RECURSIVE, True)

    def test_missing_extra_duplicate_and_unqualified_inventories_fail(self):
        cases = [
            ("recursive", "stock", [], oracle.ISOLATED_RECURSIVE, True),
            ("recursive", "stock", ["root.smt_transcript"], ["root.smt_transcript"], True),
            ("recursive", "stock", ["root.smt_transcript"], oracle.ISOLATED_RECURSIVE + ["extra.smt_transcript"], True),
            ("recursive", "stock", ["root.smt_transcript"], oracle.ISOLATED_RECURSIVE * 2, True),
            ("recursive", "stock", ["root.smt_transcript"], oracle.ISOLATED_RECURSIVE, False),
            ("success", "stock", ["root.smt_transcript"], oracle.ISOLATED_RECURSIVE, True),
            ("recursive", "fork", ["root.smt_transcript"], oracle.ISOLATED_RECURSIVE, True),
        ]
        for args in cases:
            with self.subTest(args=args), self.assertRaises(AssertionError):
                oracle.check_inventory(*args)


if __name__ == "__main__":
    unittest.main()

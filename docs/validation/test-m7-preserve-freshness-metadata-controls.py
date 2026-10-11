"""Pure controller predicate checks; no compiler or verification actor."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("controls", Path(__file__).with_name("m7-preserve-freshness-metadata-controls.py"))
CONTROLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTROLS)


class Controls(unittest.TestCase):
    def test_only_exact_unchanged_owner_assertion_failure_is_sensitivity(self):
        text = CONTROLS.CASE + " ... FAILED\nassertion failed: " + CONTROLS.ASSERTION + "\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n"
        self.assertTrue(CONTROLS.expected_negative(101, text))
        for code, candidate in [(0, text), (101, text.replace(CONTROLS.ASSERTION, "other")),
                                (101, text.replace(CONTROLS.CASE, "other")),
                                (101, text.replace("1 failed", "2 failed")),
                                (101, text.replace("1 failed", "11 failed")),
                                (101, "error: compilation failed")]:
            self.assertFalse(CONTROLS.expected_negative(code, candidate))


if __name__ == "__main__":
    unittest.main()

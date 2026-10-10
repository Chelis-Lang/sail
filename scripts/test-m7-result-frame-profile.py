#!/usr/bin/env python3
"""Fast offline-profiler checks, without daemon, proof or acceptance claims."""

import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "profile", Path(__file__).with_name("m7-result-frame-profile.py"))
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


class WholeFrameProfile(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "frame"
        self.expected = {
            "version": 1, "body": {
                "response": "result", "value": {
                    "job": {"currentness": "unknown"},
                    "result": {"nested": [{"diagnostics": "raw\\n\"text\"", "extra": 4}]},
                    "repair": {"full": {"extra": 4}},
                },
            },
        }
        payload = json.dumps(self.expected).encode()
        self.frame = struct.pack(">I", len(payload)) + payload
        self.path.write_bytes(self.frame)

    def test_exact_whole_graph_current_and_experimental_transports(self):
        frame, expected = profile.load_frame(self.path)
        self.assertEqual(frame, self.frame)
        self.assertEqual(expected, self.expected)
        self.assertEqual(profile.transfer(frame, False), expected)
        self.assertEqual(profile.transfer(frame, True), expected)
        result = profile.profile(frame, expected, 4)
        self.assertFalse(result["live_acceptance"])
        self.assertTrue(result["complete_graph_equality_checked"])
        self.assertEqual(len(result["results"]), 6)

    def test_truncated_oversized_extra_or_unsupported_frame_refused(self):
        for frame in (
                b"", self.frame[:-1], self.frame + b"x",
                struct.pack(">I", profile.MAX_FRAME + 1),
                struct.pack(">I", 13) + b'{"version":2}'):
            with self.subTest(frame_length=len(frame)):
                self.path.write_bytes(frame)
                with self.assertRaises(ValueError):
                    profile.load_frame(self.path)

    def test_graph_loss_or_invalid_samples_refused(self):
        with self.assertRaisesRegex(ValueError, "graph changed"):
            profile.profile(self.frame, {"changed": True}, 4)
        with self.assertRaises(ValueError):
            profile.profile(self.frame, self.expected, 3)


if __name__ == "__main__":
    unittest.main()

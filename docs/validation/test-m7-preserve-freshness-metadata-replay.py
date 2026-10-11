"""Pure immutable replay-lineage checks; no executable or compiler is run."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("replay", Path(__file__).with_name("m7-preserve-freshness-metadata-replay.py"))
REPLAY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPLAY)
CONTROLS = REPLAY.controls()


class Lineage(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        root = Path(self.directory.name)
        self.prior = root / "prior"
        self.historical = root / "historical"
        self.prior.mkdir()
        self.historical.mkdir()
        self.prior = self.prior.resolve(strict=True)
        self.historical = self.historical.resolve(strict=True)
        (self.prior / "tested-binaries").mkdir()
        self.binding = {"source": {}, "lock_sha256": "unit-only"}
        for name, key in [("m7-preserve-freshness-metadata-controls.py", "prior_controller_sha256"),
                          ("m7-preserve-freshness-metadata-source.json", "prior_source_record_sha256"),
                          ("m7-preserve-freshness-metadata-counterfactual.patch", "counterfactual_sha256")]:
            path = self.historical / name
            path.write_text("unit fixture " + name)
            self.binding[key] = CONTROLS.sha(path)
        for key in ("positive_image", "negative_image"):
            path = self.prior / "tested-binaries" / key
            path.write_bytes(key.encode())  # Deliberately not an executable.
            path.chmod(0o755)
            self.binding[key] = {"name": key, "bytes": path.stat().st_size, "sha256": CONTROLS.sha(path)}
        names = ["positive-build", "metadata", "freshness", "strict", "removed-noop-build", "removed-noop"]
        self.result = {"phases": [], "status": "FAIL", "source_equal": True, "lock_equal": True,
                       "active_leader_pid": None, "closure_uncertainty_reasons": [], "source": {}, "lock_sha256": "unit-only"}
        self.binding["prior_logs"] = {}
        for name, code in zip(names, [0, 0, 0, 0, 0, 101]):
            phase = {"name": name, "exit_code": code}
            if name in ("positive-build", "removed-noop-build"):
                key = "positive_image" if name == "positive-build" else "negative_image"
                phase["tested_images"] = [self.binding[key]]
            path = self.prior / (name + ".log")
            data = Path(__file__).with_name("m7-preserve-freshness-metadata-libtest-failure.txt").read_bytes() if name == "removed-noop" else b"unit log\n"
            path.write_bytes(data)
            self.binding["prior_logs"][path.name] = CONTROLS.sha(path)
            self.result["phases"].append(phase)
        self.save_result()

    def save_result(self):
        path = self.prior / "result.json"
        path.write_text(json.dumps(self.result))
        self.binding["prior_result_sha256"] = CONTROLS.sha(path)

    def verify(self):
        return REPLAY.verify_prior(CONTROLS, self.prior, self.binding, self.historical)

    def test_exact_lineage_and_both_images(self):
        self.assertEqual(len(self.verify()), 2)

    def test_changed_retained_image_is_refused(self):
        self.verify()
        (self.prior / "tested-binaries" / "positive_image").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            self.verify()

    def test_changed_compiler_log_is_refused(self):
        (self.prior / "positive-build.log").write_text("changed")
        with self.assertRaises(ValueError):
            self.verify()

    def test_changed_historical_controller_is_refused(self):
        (self.historical / "m7-preserve-freshness-metadata-controls.py").write_text("changed")
        with self.assertRaises(ValueError):
            self.verify()

    def test_incomplete_compilation_lineage_is_refused(self):
        self.result["phases"].pop()
        self.save_result()
        with self.assertRaises(ValueError):
            self.verify()
        changed = copy.deepcopy(self.binding)
        changed["prior_result_sha256"] = "wrong"
        with self.assertRaises(ValueError):
            REPLAY.verify_prior(CONTROLS, self.prior, changed, self.historical)


if __name__ == "__main__":
    unittest.main()

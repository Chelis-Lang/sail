#!/usr/bin/env python3
"""Refusal controls for ordinary frontend input capture evidence."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "capture_frontend_inputs", Path(__file__).with_name("capture-frontend-inputs.py")
)
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


class CaptureTests(unittest.TestCase):
    def test_verification_requires_actual_positive_error_free_json(self):
        good = {"verification-results": {"verified": 7, "errors": 0,
                "encountered-error": False, "encountered-vir-error": False}, "func-details": {}}
        raw = json.dumps(good).encode()
        self.assertEqual(len(CAPTURE.verification(b"cargo prefix\n" + raw)), 1)
        for bad in [b"", b"{}", raw.replace(b'"errors": 0', b'"errors": 1'),
                    raw.replace(b'"verified": 7', b'"verified": 0')]:
            with self.assertRaises(ValueError):
                CAPTURE.verification(bad)

    def test_capture_requires_exact_complete_payloads_and_bucket_set(self):
        with tempfile.TemporaryDirectory(prefix="sail-m8-capture-test-") as temporary:
            root = Path(temporary)
            artifacts = []
            for suffix in ["pruned-ast.json", "context.debug", "fresh-sst.debug"]:
                path = root / ("sail-m8-123-0-" + suffix)
                path.write_bytes(b'{"span":{"id":81,"data":[1,2]}}')
                artifacts.append({"name": path.name, "bytes": path.stat().st_size,
                                  "sha256": CAPTURE.sha(path), "representation_finished": True})
            manifest = {"version": 1, "crate": "test", "bucket": "module one",
                        "key_qualified": False, "output_artifact_qualified": False,
                        "record_byte_limit": CAPTURE.RECORD_BYTES, "process_record_limit": 2,
                        "raw_pruned_bincode_byte_limit": CAPTURE.RAW_HASH_BYTES,
                        "raw_pruned_bincode_bytes": 40,
                        "raw_pruned_bincode_finished": True, "fresh_lowering_returned": True,
                        "fresh_bucket_returned": True, "artifacts": artifacts}
            path = root / "sail-m8-123-0-manifest.json"

            def validate():
                path.write_text(json.dumps(manifest))
                return CAPTURE.validate_captures(root, "test", ["module one"])

            self.assertEqual(len(validate()), 1)
            for key, wrong in [("raw_pruned_bincode_byte_limit", CAPTURE.RECORD_BYTES),
                               ("raw_pruned_bincode_bytes", CAPTURE.RAW_HASH_BYTES + 1),
                               ("record_byte_limit", 64 * 1024 * 1024),
                               ("process_record_limit", 3)]:
                previous = manifest[key]
                manifest[key] = wrong
                with self.assertRaises(ValueError):
                    validate()
                manifest[key] = previous
            previous = CAPTURE.RECORD_BYTES
            try:
                CAPTURE.RECORD_BYTES = CAPTURE.MANIFEST_BYTES + 1
                manifest["record_byte_limit"] = CAPTURE.RECORD_BYTES
                with self.assertRaises(ValueError):
                    validate()
            finally:
                CAPTURE.RECORD_BYTES = previous
                manifest["record_byte_limit"] = previous
            for key in ["raw_pruned_bincode_finished", "fresh_lowering_returned", "fresh_bucket_returned"]:
                manifest[key] = False
                with self.assertRaises(ValueError):
                    validate()
                manifest[key] = True
            artifacts[0]["representation_finished"] = False
            with self.assertRaises(ValueError):
                validate()
            artifacts[0]["representation_finished"] = True
            payload = root / artifacts[0]["name"]
            payload.write_bytes(b"different bytes")
            with self.assertRaises(ValueError):
                validate()

    def test_missing_observer_cannot_count_as_success(self):
        with tempfile.TemporaryDirectory(prefix="sail-m8-empty-test-") as temporary:
            with self.assertRaises(ValueError):
                CAPTURE.validate_captures(Path(temporary), "test", ["module one"])

    def test_disabled_observer_requires_actually_empty_directory(self):
        with tempfile.TemporaryDirectory(prefix="sail-m8-disabled-test-") as temporary:
            root = Path(temporary)
            self.assertEqual(CAPTURE.validate_disabled(root), [])
            (root / "unexpected-pruned-ast.json").write_bytes(b"{}")
            with self.assertRaises(ValueError):
                CAPTURE.validate_disabled(root)

    def test_packaged_runtime_is_not_a_dependency_source_checkout(self):
        with tempfile.TemporaryDirectory(prefix="sail-m8-layout-test-") as temporary:
            root = Path(temporary)
            for name in ["builtin", "builtin_macros", "vstd"]:
                (root / name).mkdir()
                (root / name / "Cargo.toml").write_text("[package]\n")
            with self.assertRaises(ValueError):
                CAPTURE.dependency_source_identity(root)

    def test_patch_receipt_rejects_actual_archived_noop(self):
        with tempfile.TemporaryDirectory(prefix="sail-m8-patch-test-") as temporary:
            root = Path(temporary)
            manifest = root / "Cargo.toml"
            manifest.write_text("[package]\n")
            receipt = f"Some({json.dumps(str(manifest))}) [\"vstd\"]\n".encode()
            self.assertEqual(CAPTURE.validate_patch_receipt(receipt, root)["crates"], ["vstd"])
            for raw in [b"None []\n", b"", b"Some(\"unrelated\") []\n",
                        receipt.replace(b'["vstd"]', b'[]')]:
                with self.assertRaises((ValueError, OSError)):
                    CAPTURE.validate_patch_receipt(raw, root)

    def test_resolved_graph_must_use_frozen_source_for_all_verus_dependencies(self):
        with tempfile.TemporaryDirectory(prefix="sail-m8-deps-test-") as temporary:
            root = Path(temporary)
            packages = []
            for name, directory in [("vstd", "vstd"), ("verus_builtin", "builtin"),
                                    ("verus_builtin_macros", "builtin_macros")]:
                path = root / "source" / directory / "Cargo.toml"
                path.parent.mkdir(parents=True)
                path.write_text("[package]\n")
                packages.append({"name": name, "manifest_path": str(path), "source": None,
                                 "id": "owned:" + name, "version": "0.0.0"})
            metadata = {"packages": packages}
            self.assertEqual(len(CAPTURE.validate_dependency_resolution(metadata, root)), 3)
            packages[-1]["source"] = "registry+https://github.com/rust-lang/crates.io-index"
            with self.assertRaises(ValueError):
                CAPTURE.validate_dependency_resolution(metadata, root)
            packages[-1]["source"] = None
            packages[-1]["manifest_path"] = str(root / "source/vstd/Cargo.toml")
            with self.assertRaises(ValueError):
                CAPTURE.validate_dependency_resolution(metadata, root)
            packages[-1]["manifest_path"] = str(root / "source/builtin_macros/Cargo.toml")
            with self.assertRaises(ValueError):
                CAPTURE.validate_dependency_resolution({"packages": packages + packages[-1:]}, root)
            with self.assertRaises(ValueError):
                CAPTURE.validate_dependency_resolution({"packages": packages[:-1]}, root)

    def test_body_edit_is_explicit_and_rejects_wrong_location(self):
        raw = b"// prefix\nproof fn lemma(x: int)\n    requires x > 0,\n{\n}\n"
        self.assertEqual(CAPTURE.body_edit(raw, 2), raw.replace(b"{", b"{ assert(true);", 1))
        with self.assertRaises(ValueError):
            CAPTURE.body_edit(raw, 1)


if __name__ == "__main__":
    unittest.main()

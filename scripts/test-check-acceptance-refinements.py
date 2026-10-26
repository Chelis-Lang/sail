#!/usr/bin/env python3
"""Failure controls for production source capture and declaration refinement."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / filename)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


CHECKER = load("production_refinement_checker_tests", "check-acceptance-refinements.py")
SOURCE = CHECKER.SOURCE


class SourceCaptureTests(unittest.TestCase):
    """Mistaken source/representation selection cannot become proof evidence."""

    def snapshots(self) -> dict[str, bytes]:
        return {"crates/sail-api/src/" + name + ".rs":
            (ROOT / "crates/sail-api/src" / (name + ".rs")).read_bytes()
            for name in SOURCE.API_MODULES}

    def test_real_stencil_preserves_actual_authority_fields(self):
        files, inventory = SOURCE.api_stencils(self.snapshots())
        self.assertIn(b"pub command: Vec < String >,", files["output.rs"])
        report = next(item for item in inventory["output"]["declarations"] if item["name"] == "Report")
        self.assertIn("diagnostics", report["fields"])
        self.assertNotIn(b"pub diagnostics:", files["output.rs"])
        self.assertIn("source_sha256", inventory["output"])
        self.assertIn(b"pub run_id: String,", files["project.rs"])
        self.assertIn(b"pub args: Vec < String >,", files["project.rs"])
        self.assertIn(b"pub targets: Vec < RequestTarget >,", files["project.rs"])
        self.assertIn(b"pub engine: Option < EngineIdentity >,", files["project.rs"])
        self.assertIn(b"Repair(crate :: repair :: Selection),", files["jobs.rs"])
        self.assertIn(b"pub function: String,", files["repair.rs"])
        self.assertIn(b"pub solver: Option < SolverInfo >,", files["output.rs"])
        self.assertIn(b"pub mode: String,", files["output.rs"])
        self.assertIn(b"pub sha256: Option < String >,", files["output.rs"])
        self.assertIn(b"pub establishing_run: String,", files["project.rs"])
        self.assertIn(b"pub reused: bool,", files["project.rs"])


    def test_submission_payload_and_selection_representation_cannot_change(self):
        snapshots = self.snapshots()
        _, expected = SOURCE.api_stencils(snapshots)
        for path, before, after in (
            ("jobs", b"Repair(crate::repair::Selection)", b"Repair(String)"),
            ("jobs", b"Repair(crate::repair::Selection)", b"Repair(crate::repair::Selection, bool)"),
            ("jobs", b"Repair(crate::repair::Selection)", b"Repair"),
            ("repair", b"pub function: String,", b"pub function: Option<String>,"),
            ("repair", b"pub target: Option<String>,", b"pub target: String,"),
        ):
            candidate = dict(snapshots)
            key = "crates/sail-api/src/" + path + ".rs"
            self.assertIn(before, candidate[key])
            candidate[key] = candidate[key].replace(before, after, 1)
            with self.subTest(path=path, after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.api_stencils(candidate, expected)
        for before, after in (
            (b"pub enum Mode {", b"#[cfg(unix)] pub enum Mode {"),
            (b"pub enum Mode {", b"#[cfg(not(sail_verus))] pub enum Mode {"),
            (b"use serde_json::Value;", b"use serde_json::Value; use crate::repair::Selection as String;"),
            (b'cfg!(target_os = "linux")', b'cfg!(feature = "proof_override")'),
        ):
            candidate = dict(snapshots)
            key = "crates/sail-api/src/jobs.rs"
            self.assertIn(before, candidate[key])
            candidate[key] = candidate[key].replace(before, after, 1)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.api_stencils(candidate, expected)
        for source in ("pub enum Mode { Repair(String) }",
                "pub enum Mode { Repair { selection: String } }"):
            with self.subTest(source=source), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.record(source, "Mode", "enum")

    def test_request_identity_fields_cannot_disappear_from_stencil(self):
        snapshots = self.snapshots()
        _, expected = SOURCE.api_stencils(snapshots)
        path = "crates/sail-api/src/project.rs"
        for before, after in (
            (b"pub run_id: String,", b"pub run_id: Option<String>,"),
            (b"pub targets: Vec<RequestTarget>,", b"pub targets: Vec<TargetResult>,"),
            (b"pub engine: Option<EngineIdentity>,", b"pub engine: Option<String>,"),
        ):
            candidate = dict(snapshots)
            self.assertIn(before, candidate[path])
            candidate[path] = candidate[path].replace(before, after, 1)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.api_stencils(candidate, expected)

    def test_nested_or_literal_declaration_cannot_replace_top_level_type(self):
        for source in (
            'const TEXT: &str = "pub struct Report { pub field: bool }";',
            "mod hidden { pub struct Report { pub field: bool } }",
            "macro_rules! hidden { () => { pub struct Report { pub field: bool } } }",
        ):
            with self.subTest(source=source), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.record(source, "Report", "struct")

    def test_conditional_declaration_and_field_refuse(self):
        for source in (
            "#[cfg(not(sail_verus))] pub struct Report { pub field: bool }",
            "pub struct Report { #[cfg(not(sail_verus))] pub field: bool }",
            'pub struct Report { #[cfg_attr(sail_verus, some_attr)] pub field: bool }',
        ):
            with self.subTest(source=source), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.record(source, "Report", "struct")

    def test_custom_derive_cannot_inject_hidden_type_bindings(self):
        for derive in ("arbitrary::InjectBindings", "InjectBindings", "Debug, arbitrary::Copy"):
            source = "#[derive(" + derive + ")] pub struct Report { pub field: bool }"
            with self.subTest(derive=derive), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.record(source, "Report", "struct")

    def test_unknown_derive_on_unselected_type_also_refuses(self):
        snapshots = self.snapshots()
        key = "crates/sail-api/src/output.rs"
        snapshots[key] += b"\n#[derive(custom::Inject)] pub struct Unused {}\n"
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.api_stencils(snapshots)

    def test_variant_shape_and_conditional_variant_refuse(self):
        for source in (
            "pub enum Verdict { Good, Other(u8), }",
            "pub enum Verdict { Good, #[cfg(sail_verus)] Other, }",
            "pub enum Verdict { Good = 4, }",
        ):
            with self.subTest(source=source), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.record(source, "Verdict", "enum")

    def test_import_alias_and_primitive_shadowing_refuse(self):
        for addition in (b"use hidden::Vec;\n", b"type Vec<T> = hidden::Vec<T>;\n",
                b"pub struct String {}\n", b"inject_types!();\n"):
            snapshots = self.snapshots()
            key = "crates/sail-api/src/output.rs"
            snapshots[key] = addition + snapshots[key]
            with self.subTest(addition=addition), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.api_stencils(snapshots)

    def test_unread_field_change_still_requires_representation_review(self):
        snapshots = self.snapshots()
        _, expected = SOURCE.api_stencils(snapshots)
        key = "crates/sail-api/src/output.rs"
        snapshots[key] = snapshots[key].replace(b"pub sail_version: String,", b"pub sail_version: Option<String>,")
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.api_stencils(snapshots, expected)

    def test_syntax_runtime_branch_cannot_differ_from_reviewed_erasure(self):
        original = (ROOT / CHECKER.SYNTAX).read_bytes()
        CHECKER.compiled_subset(CHECKER.SYNTAX, original)
        changed = original.replace(
            b"-> $return_type $(where $($bounds)*)? $body",
            b"-> $return_type $(where $($bounds)*)? { true }",
        )
        self.assertNotEqual(changed, original)
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.compiled_subset(CHECKER.SYNTAX, changed)

    def test_cargo_paths_and_private_token_fields_cannot_change_without_review(self):
        paths = ["Cargo.toml", "crates/sail-acceptance/Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/src/query.rs", "crates/sail-acceptance/src/inputs.rs",
            "crates/sail-acceptance/src/authority.rs",
            "crates/sail-api/src/acceptance.rs", "crates/sail-api/src/lib.rs"]
        paths += ["crates/" + name + "/Cargo.toml" for name in
            ("sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon")]
        snapshots = {path: (ROOT / path).read_bytes() for path in paths}
        CHECKER.validate_assembly(snapshots)
        for path, before, after in (
            ("crates/sail-acceptance/Cargo.toml", b"[lints]", b'[lib]\npath="../../elsewhere.rs"\n\n[lints]'),
            ("crates/sail-api/Cargo.toml", b"[lints]", b'[lib]\npath="alternate.rs"\n\n[lints]'),
            ("Cargo.toml", b'sail-api = { path = "crates/sail-api" }', b'sail-api = { path = "alternate/sail-api" }'),
            ("Cargo.toml", b'[workspace]', b'[replace]\n"sail-acceptance:0.1.0" = { path = "alternate" }\n[workspace]'),
            ("crates/sail-acceptance/src/lib.rs", b"pub mod authority;", b'#[path="hidden.rs"] pub mod authority;'),
            ("crates/sail-api/src/acceptance.rs", b"report: &'a Report,", b"pub report: &'a Report,"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct CompleteReport", b"#[derive(Default)] pub struct CompleteReport"),
            ("crates/sail-api/src/acceptance.rs", b"request: &'a ResolvedRequest,", b"pub request: &'a ResolvedRequest,"),
            ("crates/sail-acceptance/src/query.rs", b"bytes: Vec<u8>,", b"pub bytes: Vec<u8>,"),
            ("crates/sail-acceptance/src/query.rs", b"bytes: &'a [u8],", b"pub bytes: &'a [u8],"),
            ("crates/sail-acceptance/src/query.rs", b"pub struct RawTranscript", b"#[derive(Default)] pub struct RawTranscript"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct ProviderSeal", b"#[derive(Default)] pub struct ProviderSeal"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct OpenedRead", b"#[derive(Default)] pub struct OpenedRead"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct ReadChunk", b"#[derive(Deserialize)] pub struct ReadChunk"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct ReadSeal", b"#[derive(Default)] pub struct ReadSeal"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct SubmissionBinding", b"#[derive(Deserialize)] pub struct SubmissionBinding"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct SubmittedAuthority", b"#[derive(Default)] pub struct SubmittedAuthority"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct AttemptLedger", b"#[derive(Default)] pub struct AttemptLedger"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct ReservedAttempt", b"#[derive(Deserialize)] pub struct ReservedAttempt"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct RunningAttempt", b"#[derive(Default)] pub struct RunningAttempt"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct ObservedAttempt", b"#[derive(Deserialize)] pub struct ObservedAttempt"),

        ):
            candidate = dict(snapshots)
            self.assertIn(before, candidate[path])
            candidate[path] = candidate[path].replace(before, after, 1)
            with self.subTest(path=path, after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(candidate)

    def test_executable_cfg_loading_and_unchecked_spec_refuse(self):
        for source in (b"#[cfg(sail_verus)] fn valid() -> bool { true }",
                b'include!("hidden.rs");', b"assume(false);", b"assume_specification[foo]() ensures false;", b"#[verifier::external_body] fn f() {}"):
            with self.subTest(source=source), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset("crates/sail-acceptance/src/query.rs", source)

    def test_unchecked_private_token_constructor_refuses_even_when_stock_ignores_it(self):
        for source in (
            "pub fn unchecked_constructor() -> bool { true }",
            "sail_code! { pub fn unchecked_constructor() -> bool { true } }",
            "sail_code! { impl Token { pub fn unchecked_constructor() -> bool { true } } }",
        ):
            with self.subTest(source=source), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.checked_item_envelopes(source)
        SOURCE.checked_item_envelopes("sail_verified! { pub fn checked() -> (r:bool) requires(true) ensures(r) { true } }")

    def test_untrusted_ordering_cannot_substitute_the_report_being_authorized(self):
        source = (ROOT / "crates/sail-api/src/acceptance_sort.rs").read_text()
        SOURCE.candidate_forwarders(source)
        for before, after in (
            ("complete_request_ordered(request, project, &orders)", "complete_request_ordered(other_request(), project, &orders)"),
            ("complete_request_ordered(request, project, &orders)", "complete_request_ordered(request, other_project(), &orders)"),
            ("complete_report_ordered(report, policy,", "complete_report_ordered(other_report(), policy,"),
            ("report: &Report, policy:", "report: &OtherReport, policy:"),
            ("use crate::output::Report;", "use hidden::Report;"),
        ):
            changed = source.replace(before, after, 1)
            self.assertNotEqual(changed, source)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.candidate_forwarders(changed)

    def test_erased_import_or_function_cannot_change_executable_name_resolution(self):
        path = "crates/sail-acceptance/src/authority.rs"
        original = (ROOT / path).read_bytes()
        CHECKER.compiled_subset(path, original)
        for addition in (b"\nuse hidden::*;", b"\n#[cfg(sail_verus)] use super::inputs::byte_lt as exact_bytes;"):
            with self.subTest(addition=addition), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original + addition)
        for erased in ("pub fn exact_bytes() -> bool { true }", "type Byte = bool;", "use hidden::*;"):
            with self.subTest(erased=erased), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.checked_item_envelopes("sail_spec! { " + erased + " }")

    def test_reviewed_pointer_equality_shadow_mutant_refuses(self):
        path = "crates/sail-acceptance/src/inputs.rs"
        original = (ROOT / path).read_bytes()
        mutation = original.replace(b"byte_lt(a.name, b.name)", b"eq(a.name, b.name)")
        self.assertNotEqual(mutation, original)
        mutation += b"\nuse std::ptr::*;\n#[cfg(sail_verus)] use super::inputs::byte_lt as eq;\n"
        with self.assertRaisesRegex(CHECKER.Refusal, "import bindings changed"):
            CHECKER.compiled_subset(path, mutation)

    def test_working_assembly_detects_new_build_source_and_symlink_paths(self):
        with tempfile.TemporaryDirectory(prefix="sail-source-layout-") as temporary:
            root = Path(temporary)
            path = "crates/sail-api/src/lib.rs"
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_bytes(b"original")
            snapshots = {path: b"original"}
            CHECKER.working_source_matches(root, snapshots)
            build = root / "crates/sail-api/build.rs"
            build.write_text("fn main() {}")
            with self.assertRaisesRegex(CHECKER.Refusal, "build script"):
                CHECKER.working_source_matches(root, snapshots)
            build.unlink()
            extra = source.parent / "extra.rs"
            extra.write_bytes(b"extra")
            with self.assertRaisesRegex(CHECKER.Refusal, "discovery changed"):
                CHECKER.working_source_matches(root, snapshots)
            extra.unlink()
            outside = root / "outside"
            outside.write_bytes(b"original")
            source.unlink()
            source.symlink_to(outside)
            with self.assertRaisesRegex(CHECKER.Refusal, "symlink"):
                CHECKER.working_source_matches(root, snapshots)

    def test_complete_runner_rejects_build_script_created_during_stock_calls(self):
        # This is an orchestration failure control, not simulated proof evidence.
        # Keep real capture/layout/driver/result validation; replace only Git and
        # external runtime I/O, then add a Cargo auto-discovered file mid-run.
        import json
        import tomllib
        all_paths = CHECKER.COMMON.git(ROOT, "ls-tree", "-r", "--name-only", "HEAD").splitlines()
        fixed = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "corpus/toolchain.toml",
            "xtask/src/layering.rs", "scripts/check-acceptance-proofs.py",
            "scripts/check-acceptance-refinements.py", "scripts/acceptance-source.py"}
        paths = [path for path in all_paths if path in fixed
            or path.startswith("crates/") and ("/src/" in path and path.endswith(".rs") or path.endswith("/Cargo.toml"))
            or path.startswith("proofs/") and path.endswith((".rs", ".json"))]
        snapshots = {path: (ROOT / path).read_bytes() for path in paths}
        manifest = json.loads(snapshots[CHECKER.MANIFEST])
        entries = {entry["id"]: entry for entry in manifest["components"]}
        pins = tomllib.loads(snapshots["corpus/toolchain.toml"].decode())
        with tempfile.TemporaryDirectory(prefix="sail-final-assembly-") as temporary:
            base = Path(temporary)
            root = base / "checkout"
            for path, content in snapshots.items():
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(content)
            calls = {component: 0 for component in entries}
            def git(_root, *arguments):
                if arguments[0] == "rev-parse": return "f" * 40
                if arguments[0] == "ls-tree": return "\n".join(paths)
                if arguments[0] == "status": return ""
                self.fail("unexpected Git boundary")
            def stock(command, **_kwargs):
                component = Path(command[1]).stem.removeprefix("check_")
                entry = entries[component]
                broken = calls[component] > 0
                calls[component] += 1
                (root / "crates/sail-acceptance/build.rs").write_text("fn main() {}")
                document = {"verus": {"commit": pins["verus"]["commit"], "version": pins["verus"]["release"]},
                    "func-details": {name: {} for name in entry["functions"]},
                    "verification-results": {"is-verifying-entire-crate": True,
                        "encountered-vir-error": False, "success": not broken,
                        "encountered-error": broken, "errors": int(broken),
                        "verified": entry["verified"] - int(broken)}}
                return subprocess.CompletedProcess(command, int(broken), json.dumps(document), "")
            with mock.patch.object(CHECKER.COMMON, "git", side_effect=git), \
                    mock.patch.object(CHECKER, "captured_tree", return_value=snapshots), \
                    mock.patch.object(CHECKER.COMMON, "validated_runtime", return_value={}), \
                    mock.patch.object(CHECKER.COMMON, "proof_environment", return_value={}), \
                    mock.patch.object(CHECKER.subprocess, "check_output", return_value="commit-hash: " + pins["verus"]["rust_commit"]), \
                    mock.patch.object(CHECKER.subprocess, "run", side_effect=stock), \
                    mock.patch("builtins.print"):
                with self.assertRaisesRegex(CHECKER.Refusal, "build script"):
                    CHECKER.check(root, base / "archive", base / "runtime", base / "evidence", 120)
            self.assertEqual(sum(calls.values()), sum(1 + len(e["negative_controls"]) for e in entries.values()))
            self.assertFalse((base / "evidence/report.json").exists())

    def test_component_partition_preserves_all_controls_and_requires_full_inventory(self):
        import json
        entries = json.loads((ROOT / CHECKER.MANIFEST).read_text())["components"]
        self.assertEqual(CHECKER.selected_entries(entries, None), entries)
        combined = []
        for component in CHECKER.COMPONENTS:
            selected = CHECKER.selected_entries(entries, component)
            self.assertEqual(selected, [entry for entry in entries if entry["id"] == component])
            combined.extend((entry["id"], name) for entry in selected
                for name in ["positive", *[c["name"] for c in entry["negative_controls"]]])
        expected = [(entry["id"], name) for entry in entries
            for name in ["positive", *[c["name"] for c in entry["negative_controls"]]]]
        self.assertCountEqual(combined, expected)
        self.assertEqual(len(combined), len(set(combined)))
        for bad in (entries[:-1], [entries[0], entries[0], entries[2]]):
            with self.assertRaisesRegex(CHECKER.Refusal, "omitted or duplicated"):
                CHECKER.selected_entries(bad, "query")
        for unknown in ("", "all", "query,inputs", "positive"):
            with self.assertRaisesRegex(CHECKER.Refusal, "unknown refinement"):
                CHECKER.selected_entries(entries, unknown)

    def test_selected_guard_control_cannot_be_omitted(self):
        import json
        manifest = json.loads((ROOT / CHECKER.MANIFEST).read_text())
        snapshots = {path: (ROOT / path).read_bytes() for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        CHECKER.inventory(ROOT, manifest, snapshots)
        manifest["components"][0]["negative_controls"].pop()
        with self.assertRaisesRegex(CHECKER.Refusal, "selected mutation inventory"):
            CHECKER.inventory(ROOT, manifest, snapshots)

    def test_captured_driver_uses_snapshots_not_current_paths(self):
        component = "inputs"
        entry = {"id": component, "source": "proofs/refinement/inputs.rs",
            "dependencies": CHECKER.dependency_paths(component)}
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertEqual(files["core_inputs.rs"], snapshots["crates/sail-acceptance/src/inputs.rs"])
        entry["dependencies"] = entry["dependencies"][:-1]
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.captured_driver(entry, snapshots)

    def test_git_batch_ignores_replace_refs_export_attributes_and_inherited_redirection(self):
        with tempfile.TemporaryDirectory(prefix="sail-refinement-git-") as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            commits = []
            for content in ("original $Format:%H$", "replacement"):
                (root / "proof.rs").write_text(content)
                (root / ".gitattributes").write_text("proof.rs export-subst\n")
                subprocess.run(["git", "-C", str(root), "add", "."], check=True)
                subprocess.run(["git", "-C", str(root), "-c", "user.name=Proof Fixture",
                    "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
                    "commit", "-qm", "test: capture fixture"], check=True)
                commits.append(CHECKER.COMMON.git(root, "rev-parse", "HEAD"))
            subprocess.run(["git", "-C", str(root), "replace", commits[0], commits[1]], check=True)
            with mock.patch.dict(os.environ, {
                "GIT_DIR": str(root / "missing-git"), "GIT_WORK_TREE": str(root / "wrong-tree"),
                "GIT_OBJECT_DIRECTORY": str(root / "wrong-objects"), "GIT_CONFIG_COUNT": "1",
                "GIT_CONFIG_KEY_0": "core.fsmonitor", "GIT_CONFIG_VALUE_0": "malformed-helper",
            }):
                captured = CHECKER.captured_tree(root, commits[0], ["proof.rs"])
            self.assertEqual(captured["proof.rs"], b"original $Format:%H$")


if __name__ == "__main__":
    unittest.main()

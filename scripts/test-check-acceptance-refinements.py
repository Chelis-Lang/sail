#!/usr/bin/env python3
"""Failure controls for production source capture and declaration refinement."""

from __future__ import annotations

import hashlib
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


class SourceLexerTests(unittest.TestCase):
    """Indexed matching preserves exact literal boundaries and source spans."""

    def assert_values_and_spans(self, source: str, values: list[str]):
        expected = []
        cursor = 0
        for value in values:
            begin = source.index(value, cursor)
            cursor = begin + len(value)
            expected.append((value, begin, cursor))
        self.assertEqual([tuple(token) for token in SOURCE.tokens(source)], expected)

    def test_each_regex_uses_absolute_end_at_nonzero_positions(self):
        source = 'pre r#"raw"# \'a\' \'life 12u8 post'
        self.assertEqual([tuple(token) for token in SOURCE.tokens(source)], [
            ("pre", 0, 3), ('r#"raw"#', 4, 12), ("'a'", 13, 16),
            ("'life", 17, 22), ("12u8", 23, 27), ("post", 28, 32)])

    def test_unicode_spans_are_original_character_offsets(self):
        source = "λ \u2003'a' r\"é\" z"
        self.assertEqual([tuple(token) for token in SOURCE.tokens(source)], [
            ("λ", 0, 1), ("'a'", 3, 6), ('r"é"', 7, 11), ("z", 12, 13)])

    def test_comments_and_escaped_literals_do_not_expose_delimiters(self):
        literal = r'"// text /* \" ]"'
        raw = r'br##"/* // \" }"##'
        source = "before /* first /* nested */ { } */ " + literal + " // ignored\r\n" + raw + " after"
        self.assert_values_and_spans(source, ["before", literal, raw, "after"])

    def test_raw_prefixes_and_exact_hash_terminators(self):
        for prefix in ("r", "br", "cr"):
            for hashes in ("", "###"):
                body = "/* [ ] // é" if not hashes else 'short "## does not end'
                literal = prefix + hashes + '"' + body + '"' + hashes
                source = "before " + literal + "; after"
                with self.subTest(prefix=prefix, hashes=hashes):
                    self.assert_values_and_spans(source, ["before", literal, ";", "after"])

    def test_character_priority_lifetimes_and_existing_prefix_subset(self):
        source = r'''before 'a' 'static '\n' b'7' c"text" 0xff_u8 r#name'''
        self.assert_values_and_spans(source, ["before", "'a'", "'static", r"'\n'", "b", "'7'",
            "c", '"text"', "0xff_u8", "r", "#", "name"])

    def test_malformed_literals_keep_exact_refusal_messages(self):
        for source, message in (
            ("before /* open", "unterminated block comment"),
            ('before r##"open"#', "unterminated raw string"),
            ('before br#"open', "unterminated raw string"),
            ('before cr"open', "unterminated raw string"),
            ('before "open\\', "unterminated string"),
            ("before '", "unsupported character literal"),
            (r"before '\u{41}'", "unsupported character literal"),
            ("before '\\\n'", "unsupported character literal"),
            ("before 'λ", "unsupported character literal"),
        ):
            with self.subTest(source=source):
                with self.assertRaises(SOURCE.SourceRefusal) as failure:
                    SOURCE.tokens(source)
                self.assertEqual(str(failure.exception), message)


class SourceCaptureTests(unittest.TestCase):
    """Mistaken source/representation selection cannot become proof evidence."""

    def snapshots(self) -> dict[str, bytes]:
        return {"crates/sail-api/src/" + name + ".rs":
            (ROOT / "crates/sail-api/src" / (name + ".rs")).read_bytes()
            for name in ("output", "project")}

    def test_real_stencil_preserves_actual_authority_fields(self):
        files, inventory = SOURCE.api_stencils(self.snapshots())
        self.assertIn(b"pub command: Vec < String >,", files["output.rs"])
        report = next(item for item in inventory["output"]["declarations"] if item["name"] == "Report")
        self.assertIn("diagnostics", report["fields"])
        self.assertNotIn(b"pub diagnostics:", files["output.rs"])
        self.assertIn("source_sha256", inventory["output"])

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
        changed = original.replace(b"-> $return_type $body", b"-> $return_type { true }")
        self.assertNotEqual(changed, original)
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.compiled_subset(CHECKER.SYNTAX, changed)

    def test_cargo_paths_and_private_token_fields_cannot_change_without_review(self):
        paths = ["Cargo.toml", "crates/sail-acceptance/Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/src/query.rs", "crates/sail-acceptance/src/inputs.rs",
            "crates/sail-api/src/acceptance.rs", "crates/sail-api/src/lib.rs"]
        paths += ["crates/" + name + "/Cargo.toml" for name in
            ("sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon")]
        snapshots = {path: (ROOT / path).read_bytes() for path in paths}
        CHECKER.validate_assembly(snapshots)
        for path, before, after in (
            ("crates/sail-acceptance/Cargo.toml", b"[lints]", b'[lib]\npath="../../elsewhere.rs"\n\n[lints]'),
            ("crates/sail-api/Cargo.toml", b"[lints]", b'[lib]\npath="alternate.rs"\n\n[lints]'),
            ("Cargo.toml", b'sail-api = { path = "crates/sail-api" }', b'sail-api = { path = "alternate/sail-api" }'),
            ("Cargo.toml", b'sail-api = { path = "crates/sail-api" }', b'sail-api = { path = "crates/sail-api", default-features = false }'),
            ("Cargo.toml", b'sail-daemon = { path = "crates/sail-daemon" }',
                b'sail-daemon = { path = "alternate/sail-daemon" }'),
            ("Cargo.toml", b'sail-daemon = { path = "crates/sail-daemon" }',
                b'sail-daemon = { path = "crates/sail-daemon", default-features = false }'),
            ("Cargo.toml", b'sail-daemon = { path = "crates/sail-daemon" }',
                b'sail-daemon = { path = "crates/sail-daemon", features = ["x"] }'),
            ("Cargo.toml", b'sail-daemon = { path = "crates/sail-daemon" }',
                b'sail-daemon = { git = "https://example.invalid/sail" }'),
            ("Cargo.toml", b'[workspace]', b'[replace]\n"sail-acceptance:0.1.0" = { path = "alternate" }\n[workspace]'),
            ("crates/sail-acceptance/src/lib.rs", b"pub mod authority;", b'#[path="hidden.rs"] pub mod authority;'),
            ("crates/sail-api/src/acceptance.rs", b"report: &'a Report,", b"pub report: &'a Report,"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct CompleteReport", b"#[derive(Default)] pub struct CompleteReport"),
        ):
            candidate = dict(snapshots)
            self.assertIn(before, candidate[path])
            candidate[path] = candidate[path].replace(before, after, 1)
            with self.subTest(path=path, after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(candidate)

    def test_reviewed_proof_import_identifier_digits_preserve_exact_binding(self):
        path = "crates/sail-api/src/proof_import_fixture.rs"
        original = (b"#[cfg(sail_verus)]\n"
            b"use vstd::arithmetic::power2::{pow2, lemma2_to64};\n"
            b"sail_spec! { proof fn ordinary2() {} }\n")
        stream = SOURCE.tokens(original.decode())
        start = next(i for i, token in enumerate(stream) if token.value == "use")
        end = next(i for i in range(start, len(stream)) if stream[i].value == ";")
        imports = [[token.value for token in stream[start:end + 1]]]
        digest = hashlib.sha256(CHECKER.json.dumps(imports, separators=(",", ":")).encode()).hexdigest()
        with mock.patch.dict(CHECKER.IMPORT_SHA256, {path: digest}):
            CHECKER.compiled_subset(path, original)
            for changed in (
                original.replace(b"power2", b"power3"),
                original.replace(b"vstd::", b"hidden::"),
                original.replace(b"cfg(sail_verus)", b"cfg(not(sail_verus))"),
                original + b"#[cfg(sail_verus)] fn unchecked2() -> bool { true }",
            ):
                with self.subTest(source=changed), self.assertRaises((CHECKER.Refusal, SOURCE.SourceRefusal)):
                    CHECKER.compiled_subset(path, changed)

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
            ("complete_report_ordered(report, policy,", "complete_report_ordered(other_report(), policy,"),
            ("report: &Report, policy:", "report: &OtherReport, policy:"),
            ("use crate::output::Report;", "use hidden::Report;"),
        ):
            changed = source.replace(before, after, 1)
            self.assertNotEqual(changed, source)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.candidate_forwarders(changed)

    def test_ghost_set_union_member_call_is_not_an_erased_declaration(self):
        SOURCE.checked_item_envelopes("sail_spec! { pub open spec fn combine(a: Set<int>, b: Set<int>) -> Set<int> { a.union(b) } }")
        for declaration in ("pub union Hidden { value: u64 }", "pub type Hidden = u64;",
                "pub struct Hidden { value: u64 }", "pub enum Hidden { Value }",
                "pub trait Hidden {}", "impl Hidden {}", "pub const Hidden: u64 = 0;",
                "pub static Hidden: u64 = 0;", "x.union! { pub struct Hidden; }",
                "x.union { value: 0 }"):
            with self.subTest(declaration=declaration), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.checked_item_envelopes("sail_spec! { " + declaration + " }")

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

    def test_reviewed_api_assembly_is_accepted_and_any_other_module_refused(self):
        library = "crates/sail-acceptance/src/lib.rs"
        api = "crates/sail-api/src/lib.rs"
        reviewed = (CHECKER.ROOT / api).read_bytes()
        self.assertIn(b"pub mod stamp;", reviewed)
        self.assertEqual(hashlib.sha256(reviewed).hexdigest(), CHECKER.API_LIB_SHA256)
        snapshots = {library: (CHECKER.ROOT / library).read_bytes(),
            api: reviewed.replace(b"pub mod stamp;", b"pub mod stamp;\npub mod unreviewed;")}
        with self.assertRaisesRegex(CHECKER.Refusal, "API factory module assembly changed"):
            CHECKER.validate_assembly(snapshots)

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


class RuntimeControlTests(unittest.TestCase):
    """Negative credit requires an executable fault with intact obligations."""

    def test_runtime_control_skips_nested_contract_braces(self):
        source = """sail_method! {
            fn compare(x: bool) -> (accepted: bool)
                requires ({ let nested = true; nested })
                ensures (accepted == x, if accepted {1nat} else {0nat} == 0nat)
            { if !x { return false; } true }
        }"""
        SOURCE.runtime_mutation(source, "{ if !x { return false; } true }",
            "{ if !x { return false; } false }")
        # The historical #468 selector must refuse before invoking Stock.
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            SOURCE.runtime_mutation(source, "1nat", "false")
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            SOURCE.runtime_mutation(source, "let nested = true", "let nested = false")

    def test_reused_projection_does_not_reuse_another_sources_runtime_location(self):
        warm = "sail_method! { fn f() -> (ok: bool) requires (false) ensures (ok) { true } }"
        SOURCE.runtime_mutation(warm, "true", "false")
        # Same method and selector, but the only selected token is now a contract.
        different = "sail_method! { fn f() -> (ok: bool) requires (true) ensures (ok) { false } }"
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            SOURCE.runtime_mutation(different, "true", "false")
        SOURCE.runtime_mutation(warm, "true", "false")

    def test_runtime_control_preserves_loop_contracts_and_selects_actual_loop_code(self):
        source = """sail_method! {
            fn checked() -> (ok: bool) requires (true) ensures (ok)
            { let mut n = 0; sail_while! { (n < 1)
                invariant(n <= 1, if n == 0 {true} else {n == 1})
                decreases(1 - n) { n += 1; } } n == 1 }
        }"""
        for before, after in (
            ("n <= 1", "n < 1"),
            ("1 - n", "2 - n"),
            ("{true} else", "{false} else"),
            ("invariant(n <= 1", "invariant(n <= 2"),
        ):
            with self.subTest(before=before), self.assertRaisesRegex(
                    SOURCE.SourceRefusal, "loop contract"):
                SOURCE.runtime_mutation(source, before, after)
        SOURCE.runtime_mutation(source, "(n < 1)", "(n < 0)")
        SOURCE.runtime_mutation(source, "n += 1;", "n += 2;")
        SOURCE.runtime_mutation(source, "} n == 1 }", "} n == 0 }")

    def test_runtime_control_refuses_loop_contract_changes_inside_whole_method(self):
        method = "fn f() -> (ok: bool) requires (true) ensures (ok) { sail_while! { (false) invariant(true) decreases(0) { } } true }"
        source = "sail_method! { " + method + " }"
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "loop contract"):
            SOURCE.runtime_mutation(source, method, method.replace("invariant(true)", "invariant(false)").replace("} true }", "} false }"))
        SOURCE.runtime_mutation(source, method, method.replace("} true }", "} false }"))

    def test_runtime_control_refuses_erased_and_proof_only_selectors(self):
        source = """sail_spec! { spec fn ghost() -> bool { true } }
        sail_method! {
            fn checked() -> (answer: bool) requires (true) ensures (answer)
            { sail_proof! { assert(true); } true }
        }"""
        for before, after in (
            ("spec fn ghost() -> bool { true }", "spec fn ghost() -> bool { false }"),
            ("assert(true)", "assert(false)"),
            ("sail_proof! { assert(true); }", "sail_proof! { assert(false); }"),
        ):
            with self.subTest(before=before), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.runtime_mutation(source, before, after)
        SOURCE.runtime_mutation(source,
            "{ sail_proof! { assert(true); } true }",
            "{ sail_proof! { assert(true); } false }")

    def test_erased_proof_semicolon_cannot_masquerade_as_a_runtime_fault(self):
        for wrapper in ("sail_proof!", "proof"):
            for count in range(4):
                before = "{ " + wrapper + " { assert(true); }" + ";" * count + " true }"
                source = "sail_method! { fn f() -> (out: bool) requires (true) ensures (out) " + before + " }"
                for remaining in range(4):
                    after = "{ " + wrapper + " { assert(false); }" + ";" * remaining + " true }"
                    with self.subTest(wrapper=wrapper, count=count, remaining=remaining), self.assertRaises(SOURCE.SourceRefusal):
                        SOURCE.runtime_mutation(source, before, after)
                with self.subTest(wrapper=wrapper, count=count), self.assertRaises(SOURCE.SourceRefusal):
                    SOURCE.runtime_mutation(source, before, "{ true }")
                # Real runtime mutations stay accepted with every empty tail.
                SOURCE.runtime_mutation(source, before, before[:-6] + "false }")

    def test_runtime_control_cannot_hide_an_envelope_in_ghost_code(self):
        method = "sail_method! { fn hidden() -> (x: bool) requires (true) ensures (x) { true } }"
        for wrapper in ("sail_spec! { %s }", "sail_proof! { %s }", "proof { %s }"):
            source = wrapper % method
            self.assertEqual(SOURCE.executable_envelopes(source), [])
            with self.subTest(wrapper=wrapper), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.runtime_mutation(source, "{ true }", "{ false }")

    def test_runtime_control_refuses_nested_runtime_declarations(self):
        for nested in (
            "sail_method! { fn hidden() -> (x: bool) requires (true) ensures (if x {1nat} else {0nat} == 0nat) { true } }",
            "fn hidden() -> bool requires true ensures if result {1nat} else {0nat} == 0nat { true }",
        ):
            source = "sail_method! { fn outer() -> (ok: bool) requires (true) ensures (ok) { " + nested + " true } }"
            with self.subTest(nested=nested), self.assertRaisesRegex(
                    SOURCE.SourceRefusal, "nested runtime function"):
                SOURCE.runtime_mutation(source, "1nat", "false")

    def test_runtime_control_keeps_contracts_even_with_whole_method_selector(self):
        source = "sail_verified! { fn f() -> (ok: bool) requires (true) ensures (ok) { true } }"
        method = "fn f() -> (ok: bool) requires (true) ensures (ok) { true }"
        SOURCE.runtime_mutation(source, method, method[:-6] + "false }")
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            SOURCE.runtime_mutation(source, method, method.replace("ensures (ok)", "ensures (true)"))

    def test_runtime_control_preserves_erased_proofs_with_executable_edits(self):
        before = "{ sail_proof! { assert(true); } true }"
        source = "sail_method! { fn f() -> (out: bool) requires (true) ensures (out) " + before + " }"
        for after in (
            "{ sail_proof! { assert(false); } true && true }",
            "{ true && true }",
            "{ sail_proof! { assert(true); assert(false); } true && true }",
        ):
            with self.subTest(after=after), self.assertRaisesRegex(SOURCE.SourceRefusal, "erased proof"):
                SOURCE.runtime_mutation(source, before, after)
        SOURCE.runtime_mutation(source, before, "{ sail_proof! { assert(true); } false }")

    def test_runtime_control_cannot_move_proof_guidance_between_bodies(self):
        first = "sail_method! { fn f() -> (out: bool) requires (true) ensures (out) { sail_proof! { assert(true); } true } }"
        second = "sail_method! { fn g() -> (out: bool) requires (true) ensures (out) { true } }"
        source = first + second
        changed = first.replace("{ sail_proof! { assert(true); } true }", "{ true && true }")
        changed += second.replace("{ true }", "{ sail_proof! { assert(true); } true }")
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "erased proof"):
            SOURCE.runtime_mutation(source, source, changed)

    def test_runtime_control_cannot_move_proof_across_runtime_assignment(self):
        for proof in ("sail_proof! { assert(x); }", "proof { assert(x); }"):
            before = "{ let mut x = true; " + proof + " x = false; true }"
            source = "sail_method! { fn f() -> (out: bool) requires (true) ensures (out) " + before + " }"
            after = "{ let mut x = true; x = false; " + proof + " true && true }"
            with self.subTest(proof=proof), self.assertRaisesRegex(
                    SOURCE.SourceRefusal, "both sides of erased proof"):
                SOURCE.runtime_mutation(source, before, after)
            # Moving in the other direction also changes proof context.
            with self.subTest(reverse=proof), self.assertRaisesRegex(
                    SOURCE.SourceRefusal, "both sides of erased proof"):
                SOURCE.runtime_mutation(source.replace(before, after), after, before)

    def test_runtime_control_refuses_ambiguous_edits_on_both_sides_of_proof(self):
        before = "{ let x = true; sail_proof! { assert(x); } if x { true } else { false } }"
        source = "sail_method! { fn f() -> (out: bool) requires (true) ensures (out) " + before + " }"
        after = "{ let x = false; sail_proof! { assert(x); } if !x { false } else { true } }"
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "both sides of erased proof"):
            SOURCE.runtime_mutation(source, before, after)
        SOURCE.runtime_mutation(source, before, before.replace("let x = true;", "let x = false;"))
        SOURCE.runtime_mutation(source, before, before.replace("if x", "if !x"))

    def test_runtime_control_refuses_relocation_with_changed_repeated_expressions(self):
        before = "{ let mut x = true == true; sail_proof! { assert(x); } x = false; true }"
        source = "sail_method! { fn f() -> (out: bool) requires (true) ensures (out) " + before + " }"
        after = "{ let mut x = true; x = false == true; sail_proof! { assert(x); } true && true }"
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "both sides of erased proof"):
            SOURCE.runtime_mutation(source, before, after)

    def test_runtime_control_refuses_bare_verification_syntax(self):
        before = "{ true }"
        source = "sail_verified! { fn f() -> (out: bool) requires (true) ensures (out) " + before + " }"
        for statement in ("assert(false);", "assert forall |x: int| x == 0 by {};",
                "hide(f);", "reveal(f);", "reveal_with_fuel(f, 0);", "assume(false);"):
            with self.subTest(statement=statement), self.assertRaisesRegex(
                    SOURCE.SourceRefusal, "verification-only syntax"):
                SOURCE.runtime_mutation(source, before, "{ " + statement + " true }")
        SOURCE.runtime_mutation(source, before, "{ false }")

    def test_inventory_and_capture_refuse_contract_only_control(self):
        import copy
        import json
        manifest = copy.deepcopy(json.loads((ROOT / CHECKER.MANIFEST).read_text()))
        entry = manifest["components"][0]
        control = entry["negative_controls"][0]
        control["before"] = "ensures (result.contents() == Seq::<T>::empty())"
        control["after"] = "ensures (true)"
        snapshots = {path: (ROOT / path).read_bytes() for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        self.assertEqual(snapshots[control["source"]].decode().count(control["before"]), 1)
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            CHECKER.inventory(ROOT, manifest, snapshots)
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            CHECKER.captured_driver(entry, snapshots, control)


if __name__ == "__main__":
    unittest.main()

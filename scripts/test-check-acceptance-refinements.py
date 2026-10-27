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

    def test_stream_list_body_is_checked_and_native_delegation_is_bound(self):
        entry = {"id": "query", "dependencies": CHECKER.dependency_paths("query"),
            "source": "proofs/refinement/query.rs"}
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertEqual(files["core_query_split.rs"], snapshots[CHECKER.SPLIT_CORE])
        self.assertIn(CHECKER.SPLIT_CORE, CHECKER.source_roles(entry)["checked_source"])
        for path, before, after in (
            ("crates/sail-smtlib/src/split.rs",
                b"let current = std::mem::replace(&mut self.0, StreamSplitter::start(true));\n        let (next, items) = current.push_items(data, std::mem::take(out));",
                b"let current = std::mem::replace(&mut self.0, StreamSplitter::start(true));\n        let (next, items) = current.push_items(&[], std::mem::take(out));"),
            ("crates/sail-smtlib/src/split.rs", b"self.0.finish()\n    }\n}\n\n/// Splits solver output", b"None\n    }\n}\n\n/// Splits solver output"),
            ("crates/sail-smtlib/src/lex.rs", b"self.0\n            .next_token()\n            .map(|result| result.map_err(native_failure))", b"None"),
            ("crates/sail-smtlib/Cargo.toml", b"sail-acceptance.workspace = true", b"sail-acceptance = { path = \"../alternate\" }"),
        ):
            original = snapshots[path]
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after))

    def test_actual_owned_output_splitter_is_checked_and_native_joint_call_is_bound(self):
        entry = {"id": "query", "dependencies": CHECKER.dependency_paths("query"),
            "source": "proofs/refinement/query.rs"}
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertEqual(files["core_query_output_split.rs"], snapshots[CHECKER.OUTPUT_SPLIT_CORE])
        self.assertIn(CHECKER.OUTPUT_SPLIT_CORE, CHECKER.source_roles(entry)["checked_source"])
        path = "crates/sail-shim/src/cached.rs"
        for before, after in (
            (b"query_output_order::OutputOrder::new(", b"query_output_split::ParsedOutput::new_replayed("),
            (b"let mut batch = order.append_batch(chunk);", b"let mut batch = order.append_batch(&[]);"),
        ):
            original = snapshots[path]
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(before=before), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after))

    def test_actual_output_event_order_and_independent_native_check_are_closed(self):
        entry = {"id": "query", "dependencies": CHECKER.dependency_paths("query"),
            "source": "proofs/refinement/query.rs"}
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertEqual(files["core_query_output_order.rs"], snapshots[CHECKER.OUTPUT_ORDER_CORE])
        self.assertIn(CHECKER.OUTPUT_ORDER_CORE, CHECKER.source_roles(entry)["checked_source"])
        for path, before, after in (
            ("crates/sail-shim/src/cached.rs", b"order.append_batch(chunk)", b"order.append_batch(&[])"),
            ("crates/sail-shim/src/cached.rs", b"batch.settle_current(&mut pairer.pending, payload, &SolverOwnerIdentity)", b"true"),
            ("crates/sail-smtlib/src/split.rs", b"sail_acceptance::query_split::single_response(reply)", b"true"),
        ):
            original = snapshots[path]
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(before=before), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after))
        self.assertEqual(snapshots[CHECKER.OUTPUT_ORDER_CORE].count(
            b"self.order.comparison.next_statistics() && !crate::query_split::single_response(actual)"), 1)

    def test_actual_lexer_state_and_head_are_checked_with_bound_native_calls(self):
        entry = {"id": "query", "dependencies": CHECKER.dependency_paths("query"),
            "source": "proofs/refinement/query.rs"}
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertEqual(files["core_query_lex.rs"], snapshots[CHECKER.LEX_CORE])
        self.assertIn(CHECKER.LEX_CORE, CHECKER.source_roles(entry)["checked_source"])
        for path, before, after in (
            ("crates/sail-smtlib/src/command.rs", b"let (open_start, name) = lexer.command_head()?;", b"let (open_start, name) = (0, &text[..0]);"),
            ("crates/sail-smtlib/src/lex.rs", b"Error::new(code, failure.offset)", b"Error::new(code, 0)"),
        ):
            original = snapshots[path]
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after))

    def test_completed_presentation_has_no_public_or_derived_mint(self):
        paths = {path for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        paths.update(["Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/Cargo.toml", "crates/sail-api/src/lib.rs"])
        paths.update("crates/" + name + "/Cargo.toml" for name in
            ("sail-smtlib", "sail-api", "sail-key", "sail-cache", "sail-shim", "sail-verus", "sail-daemon"))
        original = {path: (ROOT / path).read_bytes() for path in paths}
        path = "crates/sail-api/src/acceptance.rs"
        for before, after in (
            (b"        whole: Option<CompleteRequest<'a>>,", b"        pub whole: Option<CompleteRequest<'a>>,"),
            (b"    pub struct CompletedPresentation<'a> {", b"    #[derive(Default)]\n    pub struct CompletedPresentation<'a> {"),
        ):
            self.assertEqual(original[path].count(before), 1)
            snapshots = dict(original)
            snapshots[path] = snapshots[path].replace(before, after)
            with self.subTest(after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(snapshots)

    def test_completed_view_native_pair_and_candidate_forwarding_are_bound(self):
        path = "crates/sail-daemon/src/jobs/product.rs"
        original = (ROOT / path).read_bytes()
        CHECKER.compiled_subset(path, original)
        self.assertIn(path, CHECKER.source_roles({"dependencies": CHECKER.dependency_paths("authority")})["reviewed_presentation_callers"])
        for before, after in (
            (b"completed_presentation(&self.result, self.request.as_ref())", b"completed_presentation(&self.result, None)"),
            (b"request: presentation.request(),", b"request: None,"),
        ):
            self.assertEqual(original.count(before), 1)
            with self.subTest(before=before), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after))
        source = (ROOT / "crates/sail-api/src/acceptance_sort.rs").read_text()
        SOURCE.candidate_forwarders(source)
        before = "completed_presentation_ordered(project, request, &orders)"
        self.assertEqual(source.count(before), 1)
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.candidate_forwarders(source.replace(before, "completed_presentation_ordered(project, None, &orders)"))

    def test_presentation_callers_are_native_bindings_and_enum_projection_does_not_supply_copy(self):
        entry = {"id": "authority", "source": "proofs/refinement/authority.rs",
            "dependencies": CHECKER.dependency_paths("authority")}
        roles = CHECKER.source_roles(entry)
        self.assertEqual(set(roles["reviewed_presentation_callers"]), set(CHECKER.PRESENTATION_NATIVE_BINDINGS))
        self.assertFalse(set(CHECKER.PRESENTATION_NATIVE_BINDINGS) & set(roles["checked_source"]))
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertNotIn(b"derive", files["output.rs"])
        self.assertIn(b"presentation_verdict", files["api_acceptance.rs"])
        for path, before in (
            ("crates/sail-api/src/evidence.rs", b"crate::acceptance::present_function_verdict(report)"),
            ("crates/sail-mcp/src/summary.rs", b"sail_api::acceptance::present_function_verdict(f)"),
            ("crates/sail-mcp/src/lib.rs", b"mod summary;"),
        ):
            self.assertEqual(snapshots[path].count(before), 1)
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, snapshots[path].replace(before, b"unreviewed_presentation"))

    def test_replay_receipts_and_controller_have_only_checked_private_construction(self):
        paths = {path for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        paths.update(["Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/Cargo.toml", "crates/sail-api/src/lib.rs"])
        paths.update("crates/" + name + "/Cargo.toml" for name in
            ("sail-smtlib", "sail-api", "sail-key", "sail-cache", "sail-shim", "sail-verus", "sail-daemon"))
        original = {path: (ROOT / path).read_bytes() for path in paths}
        for before, after in (
            (b"        statistics: bool,", b"        pub statistics: bool,"),
            (b"        checked: usize,", b"        pub checked: usize,"),
            (b"    pub struct ServedReply {", b"    #[derive(Default)]\n    pub struct ServedReply {"),
            (b"    pub struct Replay<'a, Owner> {", b"    #[derive(Default)]\n    pub struct Replay<'a, Owner> {"),
        ):
            snapshots = dict(original)
            self.assertEqual(snapshots[CHECKER.REPLAY_CORE].count(before), 1)
            snapshots[CHECKER.REPLAY_CORE] = snapshots[CHECKER.REPLAY_CORE].replace(before, after)
            with self.subTest(after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(snapshots)

    def test_replay_native_framing_owner_join_and_budget_forwarders_remain_bound(self):
        path = "crates/sail-shim/src/cached.rs"
        original = (ROOT / path).read_bytes()
        CHECKER.compiled_subset(path, original)
        for before, after in (
            (b"!is_single_response(bytes)", b"false"),
            (b"output.with_replay(&check.comparison, &SolverOwnerIdentity)", b"Some(output)"),
            (b"PUBLICATION_FIXED_BYTES.saturating_add(served_storage_bytes(&served))", b"PUBLICATION_FIXED_BYTES"),
        ):
            self.assertEqual(original.count(before), 1)
            with self.subTest(before=before), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after))


    def test_option_state_is_checked_but_native_parser_and_session_are_only_bound(self):
        helpers = {
            "crates/sail-session/src/lib.rs", "crates/sail-session/Cargo.toml",
            "crates/sail-smtlib/Cargo.toml",
            *{"crates/sail-smtlib/src/" + stem + ".rs" for stem in
                ("lib", "command", "lex", "split", "error")},
        }
        entry = {"id": "query", "dependencies": CHECKER.dependency_paths("query"),
            "source": "proofs/refinement/query.rs"}
        roles = CHECKER.source_roles(entry)
        self.assertIn(CHECKER.OPTIONS_CORE, roles["checked_source"])
        self.assertTrue(helpers <= set(roles["reviewed_query_io"]))
        self.assertFalse(helpers & set(roles["checked_source"]))
        snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
        _, _, files = CHECKER.captured_driver(entry, snapshots)
        self.assertIn("core_query_options.rs", files)
        for stem in ("lib", "command", "lex", "split", "error"):
            self.assertNotIn(stem + ".rs", files)
        for path in helpers:
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, snapshots[path] + b"\n// drift\n")

    def test_option_state_has_no_unchecked_public_or_derived_constructor(self):
        paths = {path for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        paths.update(["Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/Cargo.toml", "crates/sail-api/src/lib.rs"])
        paths.update("crates/" + name + "/Cargo.toml" for name in
            ("sail-smtlib", "sail-api", "sail-key", "sail-cache", "sail-shim", "sail-verus", "sail-daemon"))
        original = {path: (ROOT / path).read_bytes() for path in paths}
        for before, after in (
            (b"        active: bool,", b"        pub active: bool,"),
            (b"    pub struct OptionState {", b"    #[derive(Default)]\n    pub struct OptionState {"),
        ):
            snapshots = dict(original)
            self.assertEqual(snapshots[CHECKER.OPTIONS_CORE].count(before), 1)
            snapshots[CHECKER.OPTIONS_CORE] = snapshots[CHECKER.OPTIONS_CORE].replace(before, after)
            with self.subTest(after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(snapshots)

    def test_candidate_clock_and_endpoint_native_surface_cannot_restart_or_escape(self):
        source = (ROOT / CHECKER.CANDIDATE_NATIVE).read_text()
        SOURCE.candidate_memory_policy(source)
        for before, after in (
            ("        self.clock.exhaust();", "        self.clock = Clock::start();"),
            ("        self.slots.clear();", "        *self = Self::default();"),
            ("        self.accounting.clear();", "        self.accounting = Accounting::empty();"),
            ("            &mut self.accounting,", "            &mut unrelated_accounting,"),
            ("            &mut self.order,", "            &mut wrong_order,"),
            ("const SLOT_GROWTH: usize =", "type EscapedOldSlot = SlotBeforeGeneration;\nconst SLOT_GROWTH: usize ="),
            ("                inner: Arc::clone(endpoint),", "                inner: Arc::new(Mutex::new(Inner::default())),"),
            ("struct MemoryEndpoint {", "pub struct MemoryEndpoint {"),
            ("    queued: Prepared<MemoryEndpoint, Effort>,", "    pub queued: Prepared<MemoryEndpoint, Effort>,"),
            ("#[derive(Debug)]\npub struct MemoryTier", "#[derive(Debug, Default)]\npub struct MemoryTier"),
        ):
            self.assertEqual(source.count(before), 1)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.candidate_memory_policy(source.replace(before, after))

    def test_candidate_records_are_private_and_ticket_copy_cannot_mint_a_constructor(self):
        paths = {path for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        paths.update(["Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/Cargo.toml", "crates/sail-api/src/lib.rs"])
        paths.update("crates/" + name + "/Cargo.toml" for name in
            ("sail-api", "sail-key", "sail-cache", "sail-shim", "sail-verus", "sail-daemon"))
        original = {path: (ROOT / path).read_bytes() for path in paths}
        for before, after in (
            (b"        last: u64,", b"        pub last: u64,"),
            (b"        bytes: u64,", b"        pub bytes: u64,"),
            (b"        generation: u64,\n    }", b"        pub generation: u64,\n    }"),
            (b"    pub struct Clock {", b"    #[derive(Default)]\n    pub struct Clock {"),
            (b"    pub struct Ticket {", b"    #[derive(Default)]\n    pub struct Ticket {"),
            (b"        endpoint: Endpoint,", b"        pub endpoint: Endpoint,"),
            (b"        candidate: CandidateWrite<Effort>,", b"        pub candidate: CandidateWrite<Effort>,"),
        ):
            snapshots = dict(original)
            self.assertIn(before, snapshots[CHECKER.CANDIDATE_CORE])
            snapshots[CHECKER.CANDIDATE_CORE] = snapshots[CHECKER.CANDIDATE_CORE].replace(before, after)
            with self.subTest(after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(snapshots)

    def test_old_layout_alias_cannot_hide_a_runtime_constructor(self):
        source = (ROOT / CHECKER.CANDIDATE_NATIVE).read_text()
        for before in ("    state: StateBeforeGeneration,", "const SLOT_GROWTH: usize =",
                "        let (key, slot) = entry;"):
            self.assertEqual(source.count(before), 1)
        changed = source.replace("    state: StateBeforeGeneration,", "    state: PriorState,")
        changed = changed.replace("std::mem::size_of::<SlotBeforeGeneration>()", "80", 1)
        changed = changed.replace("const SLOT_GROWTH: usize =",
            "type PriorState = StateBeforeGeneration;\nconst SLOT_GROWTH: usize =")
        changed = changed.replace("        let (key, slot) = entry;",
            "        let (key, slot) = entry;\n        let _old = SlotBeforeGeneration {\n"
            "            value: Arc::clone(slot.value()), state: PriorState::Pending, tick: slot.tick(),\n        };")
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.candidate_memory_policy(changed)

    def test_candidate_actual_native_callers_are_bound_but_not_all_stock_compiled(self):
        entry = {"id": "query", "dependencies": CHECKER.dependency_paths("query"),
            "source": "proofs/refinement/query.rs"}
        roles = CHECKER.source_roles(entry)
        self.assertIn(CHECKER.CANDIDATE_CORE, roles["checked_source"])
        paths = {"crates/sail-cache/src/memory.rs", "crates/sail-cache/src/lib.rs",
            "crates/sail-daemon/src/writer.rs", "crates/sail-daemon/src/server.rs"}
        self.assertTrue(paths <= set(roles["reviewed_query_io"]))
        self.assertFalse(paths & set(roles["checked_source"]))
        for path in paths:
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, (ROOT / path).read_bytes() + b"\n// native drift\n")

    def test_full_body_control_cannot_remove_checked_loop_syntax(self):
        original = b"{ sail_while! { (i < n) invariant(i <= n) { i += 1; } } true }"
        CHECKER.check_control_loops(original, original[:-6] + b"false }")
        with self.assertRaisesRegex(CHECKER.Refusal, "checked loop syntax"):
            CHECKER.check_control_loops(original, b"{ false }")

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

    def test_inventory_refuses_historical_contract_selector_before_stock(self):
        import copy
        import json
        manifest = copy.deepcopy(json.loads((ROOT / CHECKER.MANIFEST).read_text()))
        controls = manifest["components"][0]["negative_controls"]
        control = next(item for item in controls
            if item["name"] == "replay-full-comparison-always-refuses")
        control["before"], control["after"] = "1nat", "false"
        snapshots = {path: (ROOT / path).read_bytes() for component in CHECKER.COMPONENTS
            for path in CHECKER.dependency_paths(component)}
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            CHECKER.inventory(ROOT, manifest, snapshots)

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

    def test_report_does_not_promote_bound_input_adapters_to_proved_bodies(self):
        for component, definition in CHECKER.COMPONENTS.items():
            entry = {"dependencies": CHECKER.dependency_paths(component)}
            roles = CHECKER.source_roles(entry)
            checked = set(roles["checked_source"])
            bound = set(roles["bound_native_input_source"])
            native = set(entry["dependencies"]) & set(CHECKER.INPUT_NATIVE_SHA256)
            self.assertEqual(bound, native)
            self.assertFalse(checked & native)
            self.assertFalse(checked & set(CHECKER.QUERY_NATIVE_BINDINGS))
            self.assertNotIn(CHECKER.KEY_NATIVE, checked)
            self.assertTrue({"crates/sail-acceptance/src/" + stem + ".rs"
                for stem in definition["core"]} <= checked)
            if CHECKER.PIPE_CORE in entry["dependencies"]:
                self.assertEqual(roles["abstract_primitive_contracts"][CHECKER.PIPE_CORE],
                    ["CompilerPipeReadHost"])

    def test_current_main_input_helpers_are_native_bindings_only(self):
        helpers = {
            "crates/sail-api/src/lib.rs", "crates/sail-api/src/stamp.rs",
            "crates/sail-verus/src/lib.rs", "crates/sail-verus/Cargo.toml",
            "crates/sail-verus/src/adapter_verus_channel.rs",
            *{"crates/sail-verus/src/managed/" + name + ".rs" for name in (
                "transport", "ghost_environment", "tool_digests", "receipts",
                "source_inputs", "build_inputs_sources", "retained/history",
                "retained/phase_fields", "noop_origins", "files")},
        }
        self.assertTrue(helpers <= set(CHECKER.INPUT_NATIVE_SHA256))
        for component in ("inputs", "authority"):
            entry = {"id": component, "dependencies": CHECKER.dependency_paths(component),
                "source": "proofs/refinement/" + component + ".rs"}
            roles = CHECKER.source_roles(entry)
            self.assertTrue(helpers <= set(roles["bound_native_input_source"]))
            self.assertFalse(helpers & set(roles["checked_source"]))
            snapshots = {path: (ROOT / path).read_bytes() for path in entry["dependencies"]}
            _, _, files = CHECKER.captured_driver(entry, snapshots)
            self.assertNotIn("stamp.rs", files)
            self.assertNotIn("transport.rs", files)
            self.assertNotIn("ghost_environment.rs", files)

    def test_current_main_native_helper_drift_refuses(self):
        for path, before, after in (
            ("crates/sail-verus/src/lib.rs",
                b"pub mod adapter;", b'#[path="unreviewed.rs"] pub mod adapter;'),
            ("crates/sail-verus/Cargo.toml",
                b"sail-acceptance.workspace = true", b'sail-acceptance = { path = "hidden" }'),
            ("crates/sail-verus/src/managed/tool_digests.rs",
                b"*recorded == stamp", b"true"),
            ("crates/sail-verus/src/managed/command_environment.rs",
                b"#[cfg(test)]\nimpl ConfigureCommand for std::process::Command",
                b"impl ConfigureCommand for std::process::Command"),
            ("crates/sail-verus/src/managed/command_environment.rs",
                b"        self.env_remove(name);", b"        self.inner.env_remove(name);"),
            ("crates/sail-verus/src/managed/transport.rs",
                b"command.remove_env(name);", b"drop(name);"),
            ("crates/sail-verus/src/adapter_verus_channel.rs",
                b"command.env(COMMON, value);", b"command.arg(value);"),
            ("crates/sail-verus/src/managed/ghost_environment.rs",
                b"document.pid != pid", b"false"),
            ("crates/sail-verus/src/managed/ghost_environment.rs",
                b"old != source", b"false"),
            ("crates/sail-verus/src/managed/ghost_environment.rs",
                b"let source = source.as_ref()?;", b"let Some(source) = source.as_ref() else { continue };"),
            ("crates/sail-verus/src/adapter.rs",
                b"crate::managed::ghost_environment::merge(&path, &observed)?;",
                b"let _ = crate::managed::ghost_environment::merge(&path, &observed);"),
            ("crates/sail-api/src/stamp.rs",
                b"metadata.ctime_nsec(),", b"0,"),
            ("crates/sail-verus/src/managed/receipts.rs",
                b"&& self.producer_history_intact()", b"&& true"),
            ("crates/sail-verus/src/managed/receipts.rs",
                b"&& self.invocation.environment_observed()", b"&& true"),
            ("crates/sail-verus/src/managed/build_inputs_sources.rs",
                b"previous.version == SOURCE_VERSION && previous.covered",
                b"previous.version == SOURCE_VERSION || previous.covered"),
        ):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            self.assertIn(before, original)
            with self.subTest(path=path, before=before), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after, 1))

    def snapshots(self) -> dict[str, bytes]:
        return {"crates/sail-api/src/" + name + ".rs":
            (ROOT / "crates/sail-api/src" / (name + ".rs")).read_bytes()
            for name in SOURCE.API_MODULES}

    def test_worker_capture_record_and_actual_version_are_in_the_checked_projection(self):
        snapshots = {"crates/sail-api/src/" + module + ".rs":
            (ROOT / "crates/sail-api/src" / (module + ".rs")).read_bytes()
            for module in SOURCE.API_MODULES}
        stencils, inventory = SOURCE.api_stencils(snapshots)
        project = stencils["project.rs"].decode()
        self.assertIn("pub struct RequestCapture {", project)
        for field in ("pub version: u32,", "pub worker_id: String,", "pub request: ResolvedRequest,"):
            self.assertIn(field, project)
        self.assertIn("pub const REQUEST_CAPTURE_VERSION : u32 = 1 ;", project)
        path = "crates/sail-api/src/project.rs"
        for before, after in (
            (b"pub const REQUEST_CAPTURE_VERSION: u32 = 1;", b"pub const REQUEST_CAPTURE_VERSION: u32 = 2;"),
            (b"pub const REQUEST_CAPTURE_VERSION: u32 = 1;", b"const REQUEST_CAPTURE_VERSION: u32 = 1;"),
            (b"pub const REQUEST_CAPTURE_VERSION: u32 = 1;", b"#[cfg(not(sail_verus))] pub const REQUEST_CAPTURE_VERSION: u32 = 1;"),
            (b"pub worker_id: String,", b"pub worker_id: Vec<u8>,"),
        ):
            changed = dict(snapshots)
            self.assertEqual(changed[path].count(before), 1)
            changed[path] = changed[path].replace(before, after)
            with self.subTest(before=before), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.api_stencils(changed, inventory)

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

    def test_job_currentness_variants_require_representation_review(self):
        snapshots = self.snapshots()
        _, expected = SOURCE.api_stencils(snapshots)
        path = "crates/sail-api/src/jobs.rs"
        before = b"pub enum Currentness {"
        self.assertIn(before, snapshots[path])
        snapshots[path] = snapshots[path].replace(before, before + b" PotentiallyCurrent,", 1)
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.api_stencils(snapshots, expected)

    def test_mutable_retained_forwarder_cannot_skip_or_substitute_request_check(self):
        path = ROOT / "crates/sail-api/src/acceptance_sort.rs"
        original = path.read_text()
        SOURCE.candidate_forwarders(original)
        before = "withhold_unbound_request_authority_ordered(project, request, &orders)"
        self.assertEqual(original.count(before), 1)
        for after in ("true", "withhold_unbound_request_authority_ordered(project, None, &orders)"):
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.candidate_forwarders(original.replace(before, after, 1))

    def test_reviewed_native_retained_bridge_cannot_skip_the_shared_guard(self):
        original = (ROOT / CHECKER.RETAINED_BRIDGE).read_bytes()
        CHECKER.compiled_subset(CHECKER.RETAINED_BRIDGE, original)
        before = b"sail_api::acceptance::withhold_unbound_request_authority(project, request)"
        self.assertEqual(original.count(before), 1)
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.compiled_subset(CHECKER.RETAINED_BRIDGE, original.replace(before, b"true", 1))

    def test_replay_and_exact_scope_forwarders_keep_actual_arguments(self):
        original = (ROOT / "crates/sail-api/src/acceptance_sort.rs").read_text()
        SOURCE.candidate_forwarders(original)
        for before, after in (
            ("bind_replayed_project_ordered(project, original, current, &orders, readback)", "true"),
            ("bind_replayed_project_ordered(project, original, current, &orders, readback)",
                "bind_replayed_project_ordered(project, original, original, &orders, readback)"),
            ("complete_exact_scope_report_ordered(report, arguments, &order.forward, &order.inverse)",
                "complete_exact_scope_report_ordered(report, &report.command, &order.forward, &order.inverse)"),
        ):
            self.assertEqual(original.count(before), 1)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.candidate_forwarders(original.replace(before, after, 1))

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

    def test_cached_tokens_do_not_share_a_callers_mutable_list(self):
        source = 'sail_verified! { pub fn guard() -> (out: bool) requires(true) ensures(out) { true } }'
        expected = CHECKER.SOURCE.tokens(source)
        first = CHECKER.SOURCE.tokens(source)
        first.clear()
        first.append(CHECKER.SOURCE.Token("false", 0, 5))
        self.assertEqual(CHECKER.SOURCE.tokens(source), expected)
        self.assertIsNot(CHECKER.SOURCE.tokens(source), CHECKER.SOURCE.tokens(source))

    def test_cached_tokens_keep_each_exact_sources_original_spans(self):
        first = 'fn guard() { "body" }'
        second = '/* earlier source */ ' + first
        CHECKER.SOURCE.tokens(first)
        changed = CHECKER.SOURCE.tokens(second)
        self.assertEqual([t.value for t in changed], [t.value for t in CHECKER.SOURCE.tokens(first)])
        self.assertEqual(changed[0].begin, second.index("fn"))
        for token in changed:
            self.assertEqual(second[token.begin:token.end], token.value)
        self.assertEqual(CHECKER.SOURCE.tokens(first)[0].begin, 0)

    def test_human_noop_counters_use_actual_project_declarations(self):
        snapshots = {"crates/sail-api/src/" + name + ".rs":
            (ROOT / "crates/sail-api/src" / (name + ".rs")).read_bytes()
            for name in CHECKER.SOURCE.API_MODULES}
        files, declarations = CHECKER.SOURCE.api_stencils(snapshots)
        selected = [t.value for t in CHECKER.SOURCE.tokens(files["project.rs"].decode())]
        for field in ("execution", "no_reuse", "verifier_invocations", "reused_targets"):
            self.assertIn(field, selected)
        self.assertNotIn("wall_ms", selected)
        original = next(d for d in declarations["project"]["declarations"] if d["name"] == "Execution")
        self.assertEqual(original["fields"]["verifier_invocations"], ["u64"])
        self.assertIn("wall_ms", original["fields"])

    def test_cargo_paths_and_private_token_fields_cannot_change_without_review(self):
        paths = ["Cargo.toml", "crates/sail-acceptance/Cargo.toml", "crates/sail-acceptance/src/lib.rs",
            "crates/sail-acceptance/src/query.rs", "crates/sail-acceptance/src/inputs.rs",
            "crates/sail-acceptance/src/authority.rs",
            CHECKER.CANDIDATE_CORE, CHECKER.CANDIDATE_NATIVE,
            CHECKER.KEY_CORE, CHECKER.WRITE_CORE, CHECKER.DELIVERY_CORE, CHECKER.OUTPUT_CORE, CHECKER.PUBLICATION_CORE, CHECKER.OUTPUT_OWNER_CORE, CHECKER.HOLDING_CORE, CHECKER.CHECKPOINT_CORE, CHECKER.OPTIONS_CORE, CHECKER.REPLAY_CORE, CHECKER.SPLIT_CORE, CHECKER.LEX_CORE, CHECKER.OUTPUT_SPLIT_CORE, CHECKER.OUTPUT_ORDER_CORE, CHECKER.PIPE_CORE,
            CHECKER.KEY_NATIVE, "crates/sail-key/Cargo.toml",
            "crates/sail-acceptance/src/query_prefix.rs",
            "crates/sail-acceptance/src/phase_inputs.rs",
            "crates/sail-api/src/acceptance.rs", "crates/sail-api/src/lib.rs", CHECKER.REQUEST_ORDER]
        paths += ["crates/" + name + "/Cargo.toml" for name in
            ("sail-smtlib", "sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon")]
        snapshots = {path: (ROOT / path).read_bytes() for path in paths}
        CHECKER.validate_assembly(snapshots)
        for path, before, after in (
            (CHECKER.REQUEST_ORDER, b"        original: (String, String, String),",
                b"        pub original: (String, String, String),"),
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
            ("crates/sail-api/src/acceptance.rs", b"pub struct ExactScopeReport", b"#[derive(Default)] pub struct ExactScopeReport"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct ReplayReadback", b"#[derive(Deserialize)] pub struct ReplayReadback"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct EmptyHumanOutput", b"#[derive(Deserialize)] pub struct EmptyHumanOutput"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct EmptyHumanContext", b"#[derive(Deserialize)] pub struct EmptyHumanContext"),
            ("crates/sail-api/src/acceptance.rs", b"stderr: &'a [u8],", b"pub stderr: &'a [u8],"),
            (CHECKER.OUTPUT_CORE, b"pub struct Transcript", b"#[derive(Default)] pub struct Transcript"),
            (CHECKER.OUTPUT_CORE, b"pub struct Frame", b"#[derive(Deserialize)] pub struct Frame"),
            (CHECKER.OUTPUT_CORE, b"pub struct CompleteOutput", b"#[derive(Default)] pub struct CompleteOutput"),
            (CHECKER.PUBLICATION_CORE, b"pub struct PublicationObservation", b"#[derive(Default)] pub struct PublicationObservation"),
            (CHECKER.OUTPUT_CORE, b"pub struct FramePayload", b"#[derive(Deserialize)] pub struct FramePayload"),
            (CHECKER.OUTPUT_OWNER_CORE, b"pub struct StreamOutput", b"#[derive(Default)] pub struct StreamOutput"),
            (CHECKER.OUTPUT_SPLIT_CORE, b"pub struct ParsedOutput", b"#[derive(Default)] pub struct ParsedOutput"),
            (CHECKER.OUTPUT_ORDER_CORE, b"pub struct OutputOrder", b"#[derive(Deserialize)] pub struct OutputOrder"),
            (CHECKER.OUTPUT_ORDER_CORE, b"pub struct OutputBatch", b"#[derive(Default)] pub struct OutputBatch"),
            (CHECKER.OUTPUT_ORDER_CORE, b"items:Vec<crate::query_split::Item>", b"pub items:Vec<crate::query_split::Item>"),
            (CHECKER.OUTPUT_SPLIT_CORE, b"controller:crate::query_output_owner::StreamOutput<Owner>", b"pub controller:crate::query_output_owner::StreamOutput<Owner>"),
            (CHECKER.OUTPUT_OWNER_CORE, b"pub struct StreamFrame", b"#[derive(Deserialize)] pub struct StreamFrame"),
            (CHECKER.OUTPUT_OWNER_CORE, b"pub struct OwnedFrame", b"#[derive(Default)] pub struct OwnedFrame"),
            (CHECKER.OUTPUT_OWNER_CORE, b"pub struct CompletedStream", b"#[derive(Deserialize)] pub struct CompletedStream"),
            (CHECKER.OUTPUT_OWNER_CORE, b"pub struct OwnedPublication", b"#[derive(Default)] pub struct OwnedPublication"),
            (CHECKER.HOLDING_CORE, b"pub struct Holding", b"#[derive(Default)] pub struct Holding"),
            (CHECKER.SPLIT_CORE, b"pub struct ListFrame", b"#[derive(Default)] pub struct ListFrame"),
            (CHECKER.SPLIT_CORE, b"        depth: usize,", b"        pub depth: usize,"),
            (CHECKER.LEX_CORE, b"pub struct Lexer<'a>", b"#[derive(Default)] pub struct Lexer<'a>"),
            (CHECKER.LEX_CORE, b"        position:usize,", b"        pub position:usize,"),
            ("crates/sail-acceptance/src/query.rs", b"pub struct ReplayedPrefix", b"#[derive(Default)] pub struct ReplayedPrefix"),
            ("crates/sail-acceptance/src/query_prefix.rs", b"pub struct FrozenCommand", b"#[derive(Deserialize)] pub struct FrozenCommand"),
            ("crates/sail-acceptance/src/query_prefix.rs", b"command: Vec<u8>,", b"pub command: Vec<u8>,"),
            (CHECKER.OUTPUT_OWNER_CORE, b"replayed: Option<ReplayedPrefix>,", b"pub replayed: Option<ReplayedPrefix>,"),
            (CHECKER.KEY_NATIVE, b"pub struct OwnedCommand", b"#[derive(Default)] pub struct OwnedCommand"),
            (CHECKER.KEY_NATIVE, b"owner: Arc<()>,", b"pub owner: Arc<()>,"),
            (CHECKER.PUBLICATION_CORE, b"input: Delivered<Host>,", b"pub input: Delivered<Host>,"),
            (CHECKER.PIPE_CORE, b"pub struct PipeCapture", b"#[derive(Default)] pub struct PipeCapture"),
            (CHECKER.PIPE_CORE, b"pub struct SealedPipe", b"#[derive(Deserialize)] pub struct SealedPipe"),
            (CHECKER.PIPE_CORE, b"owner: Owner,", b"pub owner: Owner,"),
            ("crates/sail-api/src/acceptance.rs", b"request: &'a ResolvedRequest,", b"pub request: &'a ResolvedRequest,"),
            ("crates/sail-acceptance/src/query.rs", b"bytes: Vec<u8>,", b"pub bytes: Vec<u8>,"),
            ("crates/sail-acceptance/src/query.rs", b"bytes: &'a [u8],", b"pub bytes: &'a [u8],"),
            ("crates/sail-acceptance/src/query.rs", b"pub struct RawTranscript", b"#[derive(Default)] pub struct RawTranscript"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct ProviderSeal", b"#[derive(Default)] pub struct ProviderSeal"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct OpenedRead", b"#[derive(Default)] pub struct OpenedRead"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct ReadChunk", b"#[derive(Deserialize)] pub struct ReadChunk"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct ReadSeal", b"#[derive(Default)] pub struct ReadSeal"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct GuardedReadback", b"#[derive(Default)] pub struct GuardedReadback"),
            ("crates/sail-acceptance/src/inputs.rs", b"pub struct CompletedRead", b"#[derive(Default)] pub struct CompletedRead"),
            (CHECKER.KEY_CORE, b"pub struct Prefix", b"#[derive(Default)] pub struct Prefix"),
            (CHECKER.WRITE_CORE, b"pub struct WriteCommit", b"#[derive(Default)] pub struct WriteCommit"),
            (CHECKER.DELIVERY_CORE, b"pub struct Delivery", b"#[derive(Default)] pub struct Delivery"),
            (CHECKER.DELIVERY_CORE, b"pub struct Delivered", b"#[derive(Deserialize)] pub struct Delivered"),
            ("crates/sail-acceptance/src/phase_inputs.rs", b"pub struct PhaseRequest", b"#[derive(Default)] pub struct PhaseRequest"),
            ("crates/sail-acceptance/src/phase_inputs.rs", b"pub struct PhaseInputs", b"#[derive(Deserialize)] pub struct PhaseInputs"),
            ("crates/sail-acceptance/src/phase_inputs.rs", b"pub struct EnvironmentProjection", b"#[derive(Default)] pub struct EnvironmentProjection"),
            ("crates/sail-acceptance/src/phase_inputs.rs", b"pub struct CommandEnvironment", b"#[derive(Deserialize)] pub struct CommandEnvironment"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct AttemptIdentity", b"#[derive(Default)] pub struct AttemptIdentity"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct DeclinedEngine", b"#[derive(Deserialize)] pub struct DeclinedEngine"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct MissingEngineFeature", b"#[derive(Default)] pub struct MissingEngineFeature"),
            ("crates/sail-acceptance/src/query_prefix.rs", b"pub struct QueryPrefix", b"#[derive(Default)] pub struct QueryPrefix"),
            ("crates/sail-acceptance/src/query_prefix.rs", b"pub struct FramedCommand", b"#[derive(Deserialize)] pub struct FramedCommand"),
            ("crates/sail-acceptance/src/query_prefix.rs", b"raw: RawTranscript,", b"pub raw: RawTranscript,"),
            ("crates/sail-acceptance/src/query_prefix.rs", b"head: &'a [u8],", b"pub head: &'a [u8],"),
            ("crates/sail-key/Cargo.toml", b"[lints]", b'[lib]\npath="unreviewed.rs"\n\n[lints]'),
            ("crates/sail-api/src/acceptance.rs", b"pub struct SubmissionBinding", b"#[derive(Deserialize)] pub struct SubmissionBinding"),
            ("crates/sail-api/src/acceptance.rs", b"pub struct SubmittedAuthority", b"#[derive(Default)] pub struct SubmittedAuthority"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct AttemptLedger", b"#[derive(Default)] pub struct AttemptLedger"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct UnconfiguredAttempt", b"#[derive(Deserialize)] pub struct UnconfiguredAttempt"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct ReservedAttempt", b"#[derive(Deserialize)] pub struct ReservedAttempt"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct RunningAttempt", b"#[derive(Default)] pub struct RunningAttempt"),
            ("crates/sail-acceptance/src/authority.rs", b"pub struct ObservedAttempt", b"#[derive(Deserialize)] pub struct ObservedAttempt"),

        ):
            candidate = dict(snapshots)
            self.assertIn(before, candidate[path])
            candidate[path] = candidate[path].replace(before, after, 1)
            with self.subTest(path=path, after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.validate_assembly(candidate)

    def test_abstract_file_host_cannot_change_or_erase_native_bindings(self):
        path = "crates/sail-acceptance/src/inputs.rs"
        original = (ROOT / path).read_bytes()
        CHECKER.compiled_subset(path, original)
        for before, after in (
            (b"pub struct ScriptedReadFile", b"pub struct ReadSeal"),
            (b"pub enum FileObservation", b"pub enum InputValue"),
            (b"count <= final(buffer).len()", b"true"),
            (b"fn hash_finish(&mut self, hasher: Hasher)", b"fn hash_finish(&mut self, hasher: Digest)"),
        ):
            self.assertIn(before, original)
            with self.subTest(after=after), self.assertRaises((CHECKER.Refusal, SOURCE.SourceRefusal)):
                CHECKER.compiled_subset(path, original.replace(before, after, 1))
        for source in (
            "sail_spec! { pub struct ScriptedReadFile { value: bool } }",
            "sail_trait! { pub trait FileReadHost { fn trusted() -> bool; } }",
        ):
            with self.subTest(source=source), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.checked_item_envelopes(source)

    def test_key_projection_preserves_exact_native_fields_and_mode_payload(self):
        original = (ROOT / CHECKER.KEY_NATIVE).read_bytes()
        files = {CHECKER.KEY_NATIVE: original}
        stencil, inventory = SOURCE.key_stencil(files)
        self.assertIn(b"binary_sha256: [ u8 ; 32 ],", stencil)
        self.assertIn(b"OptIn(String),", stencil)
        for before, after in (
            (b"pub platform: String,", b"pub platform: Option<String>,"),
            (b"pub binary_sha256: [u8; 32],", b"pub binary_sha256: [u8; 16],"),
            (b"OptIn(String)", b"OptIn"),
            (b"OptIn(String)", b"OptIn(Vec<u8>)"),
            (b"pub enum Mode {", b"pub enum Mode { Unrecorded,"),
        ):
            self.assertIn(before, original)
            with self.subTest(after=after), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.key_stencil({CHECKER.KEY_NATIVE: original.replace(before, after, 1)}, inventory)

    def test_key_native_hash_and_recording_host_cannot_be_substituted(self):
        native = (ROOT / CHECKER.KEY_NATIVE).read_bytes()
        CHECKER.compiled_subset(CHECKER.KEY_NATIVE, native)
        for before, after in (
            (b"Self(self.0.clone())", b"Self(Sha256::new())"),
            (b"Sha256Host(Sha256::new())", b"Sha256Host(Sha256::digest(b\"foreign\"))"),
            (b"pub name: String,", b"pub name: Vec<u8>,"),
        ):
            self.assertIn(before, native)
            with self.subTest(after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(CHECKER.KEY_NATIVE, native.replace(before, after, 1))
        core = (ROOT / CHECKER.KEY_CORE).read_bytes()
        CHECKER.compiled_subset(CHECKER.KEY_CORE, core)
        before = b"fn append(&mut self, bytes: &[u8]) { self.bytes.extend_from_slice(bytes); }"
        self.assertEqual(core.count(before), 1)
        changed = core.replace(before, b"fn append(&mut self, bytes: &[u8]) { self.bytes.clear(); }", 1)
        with self.assertRaises((CHECKER.Refusal, SOURCE.SourceRefusal)):
            CHECKER.compiled_subset(CHECKER.KEY_CORE, changed)
        source = core.decode()
        begin = source.index("    impl HashHost for RecordingHash {")
        stream = SOURCE.tokens(source[begin:])
        opening = next(i for i, token in enumerate(stream) if token.value == "{")
        end = SOURCE.closing(stream, opening)
        ordinary = source[begin:begin + stream[end].end]
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.checked_item_envelopes(ordinary, reviewed_trait_impls=CHECKER.KEY_RECORDING_IMPLS)

    def test_owned_query_modules_keep_actual_imports_and_checked_constructors(self):
        for path in ("crates/sail-acceptance/src/query_prefix.rs",
                "crates/sail-acceptance/src/query_policy.rs",
                "crates/sail-acceptance/src/phase_inputs.rs", CHECKER.OUTPUT_OWNER_CORE, CHECKER.HOLDING_CORE, CHECKER.CHECKPOINT_CORE, CHECKER.OPTIONS_CORE, CHECKER.REPLAY_CORE, CHECKER.SPLIT_CORE, CHECKER.LEX_CORE, CHECKER.CANDIDATE_CORE, CHECKER.KEY_FACTORY):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            for addition in (b"\nuse hidden::*;", b"\n#[cfg(sail_verus)] use hidden::*;"):
                with self.subTest(path=path, addition=addition), self.assertRaises(CHECKER.Refusal):
                    CHECKER.compiled_subset(path, original + addition)
        for path in ("crates/sail-acceptance/src/query_prefix.rs", CHECKER.OUTPUT_OWNER_CORE, CHECKER.HOLDING_CORE, CHECKER.CHECKPOINT_CORE, CHECKER.OPTIONS_CORE, CHECKER.REPLAY_CORE, CHECKER.SPLIT_CORE, CHECKER.LEX_CORE, CHECKER.CANDIDATE_CORE, CHECKER.KEY_FACTORY):
            original = (ROOT / path).read_bytes()
            with self.subTest(path=path), self.assertRaises(SOURCE.SourceRefusal):
                CHECKER.compiled_subset(path, original + b"\npub fn unchecked_factory() -> bool { true }")

    def test_query_io_primitives_and_script_implementations_remain_source_bound(self):
        for path, before, after in (
            ("crates/sail-cache/src/persist.rs", b".create_new(true)", b".create(true)"),
            ("crates/sail-cache/src/lib.rs", b"persist::write_record(&tmp, &path, &meta, reply)",
                b"persist::write_record(&tmp, &path, &meta, b\"unsat\\n\")"),
            ("crates/sail-shim/src/cached.rs", b"stdin.write_all(bytes)", b"stdin.write_all(b\"\")"),
        ):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after, 1))

        for path, before, after in (
            (CHECKER.WRITE_CORE, b"spec fn steps(&self) -> Seq<u8>;", b"spec fn steps(&self) -> bool;"),
            (CHECKER.DELIVERY_CORE, b"spec fn sent(&self) -> Seq<u8>;", b"spec fn sent(&self) -> bool;"),
            (CHECKER.WRITE_CORE, b"fn remove(&mut self) { self.steps.push(11); }", b"fn remove(&mut self) {}"),
            (CHECKER.DELIVERY_CORE, b"fn close(&mut self) { self.closed = true; }", b"fn close(&mut self) {}"),
        ):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(path=path), self.assertRaises(SOURCE.SourceRefusal):
                CHECKER.compiled_subset(path, original.replace(before, after, 1))

    def test_original_checkpoint_and_owned_publication_native_forwarders_remain_bound(self):
        for path, before, after in (
            (CHECKER.KEY_NATIVE, b"command: self.frame.freeze(),", b"command: hidden::checkpoint(),"),
            (CHECKER.KEY_NATIVE, b"Arc::ptr_eq(&self.owner, &command.owner)", b"true"),
            (CHECKER.KEY_NATIVE, b".validate_bindings(&checkpoints, &frames, &order, &inverse)", b".validate_bindings(&checkpoints, &frames, &inverse, &order)"),
            ("crates/sail-shim/src/cached.rs", b"    publication.validate_pairs(&pairs)\n}", b"    true\n}"),
            ("crates/sail-shim/src/cached.rs", b"let fresh = all.advance_replay();", b"let fresh = true;"),
            ("crates/sail-shim/src/cached.rs", b"self.input.charge(bytes.len())", b"true"),
        ):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(path=path, before=before), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original.replace(before, after, 1))

    def test_checkpoint_ghost_import_does_not_permit_alternate_bindings(self):
        original = (ROOT / CHECKER.CHECKPOINT_CORE).read_bytes()
        CHECKER.compiled_subset(CHECKER.CHECKPOINT_CORE, original)
        before = b"#[cfg(sail_verus)]\nuse crate::key_prefix::end;"
        self.assertEqual(original.count(before), 1)
        for after in (b"#[cfg(sail_verus)]\nuse hidden::end;",
                b"#[cfg(not(sail_verus))]\nuse crate::key_prefix::end;"):
            with self.subTest(after=after), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(CHECKER.CHECKPOINT_CORE, original.replace(before, after, 1))

    def test_compiler_pipe_read_primitive_and_native_endpoint_remain_source_bound(self):
        for path, before, after in (
            (CHECKER.PIPE_CORE, b"fn owner_view(&self) -> Owner;", b"fn owner_view(&self) -> bool;"),
            (CHECKER.PIPE_CORE, b"Bytes(Seq<u8>), Eof, EmptyRead, Failure(Failure)",
                b"Bytes(Seq<u8>), Eof, EmptyRead, Failure(bool)"),
            ("crates/sail-verus/src/managed/command_pipe.rs", b"self.reader.read(buffer)", b"Ok(0)"),
        ):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            self.assertEqual(original.count(before), 1)
            with self.subTest(path=path), self.assertRaises((CHECKER.Refusal, SOURCE.SourceRefusal)):
                CHECKER.compiled_subset(path, original.replace(before, after, 1))

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

    def test_inherited_trait_method_is_checked_and_raw_methods_still_refuse(self):
        method = "fn eq(&self, other: &Self) -> (same: bool) ensures(same) { true }"
        SOURCE.checked_item_envelopes(
            "sail_code! { impl PartialEq for Key { sail_inherited_method! { " + method + " } } }")
        for body in (
            "fn eq(&self, other: &Self) -> bool { true }",
            "unchecked_inherited_method! { " + method + " }",
            "sail_inherited_method! { " + method + " } fn unchecked() -> bool { true }",
        ):
            with self.subTest(body=body), self.assertRaises(SOURCE.SourceRefusal):
                SOURCE.checked_item_envelopes("sail_code! { impl PartialEq for Key { " + body + " } }")

    def test_actual_original_dictionary_modules_and_native_callers_remain_closed(self):
        for path in (CHECKER.REQUEST_ORDER, CHECKER.REQUEST_TARGETS):
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            for addition in (b"\nuse hidden::*;", b"\npub fn unchecked() -> bool { true }"):
                with self.subTest(path=path, addition=addition), self.assertRaises(
                        (CHECKER.Refusal, SOURCE.SourceRefusal)):
                    CHECKER.compiled_subset(path, original + addition)
        for path in CHECKER.REQUEST_NATIVE_BINDINGS:
            self.assertIn(path, CHECKER.INPUT_NATIVE_SHA256)
            original = (ROOT / path).read_bytes()
            CHECKER.compiled_subset(path, original)
            with self.subTest(path=path), self.assertRaises(CHECKER.Refusal):
                CHECKER.compiled_subset(path, original + b"\nfn unchecked() {}\n")

    def test_inherited_trait_method_mutation_preserves_its_postcondition(self):
        source = "sail_inherited_method! { fn eq(&self, other: &Self) -> (same: bool) ensures(if same {1nat} else {0nat} == 1nat) { true } }"
        SOURCE.runtime_mutation(source, "{ true }", "{ false }")
        with self.assertRaisesRegex(SOURCE.SourceRefusal, "contract or non-runtime"):
            SOURCE.runtime_mutation(source, "same {1nat}", "same {2nat}")
        with self.assertRaises(SOURCE.SourceRefusal):
            SOURCE.runtime_mutation(source,
                "ensures(if same {1nat} else {0nat} == 1nat)", "requires(true) ensures(same)")

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

    def test_ci_partitions_cover_every_control_once_and_repeat_the_whole_positive(self):
        import json
        entries = json.loads((ROOT / CHECKER.MANIFEST).read_text())["components"]
        for entry in entries:
            expected = CHECKER.partition_cases(entry, None)
            for count in (2, 4):
                combined = []
                for index in range(count):
                    part = CHECKER.partition_cases(entry, (index, count))
                    self.assertEqual(part[0], (0, None))
                    self.assertTrue(part[1:], "no positive-only CI partition")
                    self.assertEqual([ordinal for ordinal, _ in part], sorted(ordinal for ordinal, _ in part))
                    combined.extend(part[1:])
                self.assertEqual(sorted(combined), expected[1:])
                self.assertEqual(len(combined), len({ordinal for ordinal, _ in combined}))
        for bad in ("", "0", "0/1", "1/1", "4/4", "-1/4", "0/0", "00/4", "0/04", "0/4/5", " 0/4", "zero/4"):
            with self.subTest(bad=bad), self.assertRaises(CHECKER.Refusal):
                CHECKER.parse_partition(bad)
        for bad in ((0, 0), (0, 1), (-1, 4), (4, 4), (False, 4), (0, True), (0, 10000)):
            with self.subTest(bad=bad), self.assertRaises(CHECKER.Refusal):
                CHECKER.partition_cases(entries[0], bad)

    def test_ci_matrix_requires_all_fixed_partitions_for_each_component(self):
        import json
        import re
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        for component in CHECKER.COMPONENTS:
            job = workflow.split("\n  refinement-" + component + ":\n", 1)[1]
            job = re.split(r"\n  [a-z][a-z0-9-]*:\n", job, maxsplit=1)[0]
            match = re.search(r"^        partition: (\[[^\n]+\])$", job, re.M)
            self.assertIsNotNone(match)
            self.assertEqual(json.loads(match[1]), ["0/4", "1/4", "2/4", "3/4"])
            self.assertIn("fail-fast: false", job)
            self.assertIn('ci/run.sh refinements ' + component + ' "$PARTITION"', job)
            self.assertIn("PARTITION: ${{ matrix.partition }}", job)
        aggregate = workflow.split("\n  acceptance-refinements:\n", 1)[1]
        aggregate = re.split(r"\n  [a-z][a-z0-9-]*:\n", aggregate, maxsplit=1)[0]
        self.assertIn("needs: [refinement-query, refinement-inputs, refinement-authority]", aggregate)
        for name in ("QUERY", "INPUTS", "AUTHORITY"):
            self.assertIn(name + ": ${{ needs.refinement-" + name.lower() + ".result }}", aggregate)

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

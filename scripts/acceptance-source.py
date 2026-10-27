#!/usr/bin/env python3
"""Closed source capture for executable acceptance refinements.

This is reviewed proof-runner code, not a proved Rust parser. It deliberately
accepts only the named public, nongeneric records and unit enums used by the
authority adapters. Unknown syntax refuses the gate.
"""

from __future__ import annotations

import hashlib
from functools import lru_cache
import re
from typing import NamedTuple


class SourceRefusal(ValueError):
    """Source lies outside the reviewed capture subset."""


class Token(NamedTuple):
    """One Rust token with its original byte-character span."""

    value: str
    begin: int
    end: int


DERIVES = {"Debug", "Clone", "Copy", "PartialEq", "Eq", "PartialOrd", "Ord",
    "Default", "Hash", "Serialize", "Deserialize"}

# These patterns have no anchors or lookbehind: matching at an absolute source
# position preserves their former match-on-suffix behavior without tail copies.
_RAW_STRING_PREFIX = re.compile(r'(?:br|cr|r)(#*)"')
_CHAR_LITERAL = re.compile(r"'(?:\\.|[^'\\\n])'")
_LIFETIME = re.compile(r"'[A-Za-z_][A-Za-z_0-9]*")
_WORD = re.compile(r"[A-Za-z_][A-Za-z_0-9]*|[0-9][A-Za-z_0-9]*")


def checked_attribute(stream: list[Token], start: int, allowed: set[str]) -> int:
    """Accept only reviewed attributes, with no custom derive expansion."""
    require(stream[start].value == "#" and start + 2 < len(stream)
        and stream[start + 1].value == "[", "invalid source attribute")
    end = closing(stream, start + 1)
    values = [token.value for token in stream[start + 2:end]]
    require(bool(values) and values[0] in allowed, "conditional/unknown source attribute")
    if values[0] == "derive":
        require(len(values) >= 3 and values[1] == "(" and values[-1] == ")",
            "unsupported derive syntax")
        parts = values[2:-1]
        require(bool(parts), "empty derive list")
        for i, value in enumerate(parts):
            require(value in DERIVES if i % 2 == 0 else value == ",",
                "custom/qualified derive outside capture subset")
    return end


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SourceRefusal(message)


def tokens(source: str) -> list[Token]:
    """Tokenize delimiters without treating comments or literals as code."""
    # Every caller owns its list. Only immutable token tuples are shared, and
    # their cache key is the complete exact source, including original spans.
    return list(_token_tuple(source))


@lru_cache(maxsize=32)
def _token_tuple(source: str) -> tuple[Token, ...]:
    """Retain exact-source lexical work without sharing mutable parser state."""
    result: list[Token] = []
    at = 0
    while at < len(source):
        if source[at].isspace():
            at += 1
            continue
        if source.startswith("//", at):
            newline = source.find("\n", at)
            at = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", at):
            depth = 1
            at += 2
            while depth:
                require(at < len(source), "unterminated block comment")
                if source.startswith("/*", at):
                    depth += 1
                    at += 2
                elif source.startswith("*/", at):
                    depth -= 1
                    at += 2
                else:
                    at += 1
            continue
        start = at
        raw = _RAW_STRING_PREFIX.match(source, at)
        if raw:
            ending = '"' + raw[1]
            at = raw.end()
            end = source.find(ending, at)
            require(end >= 0, "unterminated raw string")
            at = end + len(ending)
        elif source[at] == '"':
            at += 1
            while at < len(source) and source[at] != '"':
                at += 2 if source[at] == "\\" else 1
            require(at < len(source), "unterminated string")
            at += 1
        elif source[at] == "'":
            char = _CHAR_LITERAL.match(source, at)
            lifetime = _LIFETIME.match(source, at)
            require(char is not None or lifetime is not None, "unsupported character literal")
            at = (char or lifetime).end()
        else:
            word = _WORD.match(source, at)
            at = word.end() if word else at + 1
        result.append(Token(source[start:at], start, at))
    return tuple(result)


def closing(stream: list[Token], start: int) -> int:
    """Find a balanced ordinary Rust group, rejecting crossed delimiters."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    require(stream[start].value in pairs, "expected delimited group")
    stack = [pairs[stream[start].value]]
    for at in range(start + 1, len(stream)):
        token = stream[at].value
        if token in pairs:
            stack.append(pairs[token])
        elif token in pairs.values():
            require(token == stack[-1], "crossed source delimiters")
            stack.pop()
            if not stack:
                return at
    raise SourceRefusal("unterminated source group")



def executable_envelopes(source: str) -> list[tuple[int, int]]:
    """Locate checked runtime bodies after balanced macro contract groups.

    This recognizes only the three reviewed checked-function envelope grammars. A
    brace inside requires/ensures (including an if expression) is never a body.
    Erased macro/proof groups cannot introduce an executable envelope.
    Spans index tokens, including the executable body's outer braces.
    """
    return _executable_envelopes(tokens(source))


def _executable_envelopes(stream: list[Token]) -> list[tuple[int, int]]:
    """Walk each balanced envelope once using the caller's exact token list."""
    bodies: list[tuple[int, int]] = []
    i = 0
    while i < len(stream):
        token = stream[i]
        macro = i + 2 < len(stream) and stream[i + 1].value == "!"
        if macro and token.value in {"sail_spec", "sail_impl_spec", "sail_proof"}:
            i = closing(stream, i + 2) + 1
            continue
        if token.value in {"proof", "spec", "by"} and i + 1 < len(stream) and stream[i + 1].value == "{":
            i = closing(stream, i + 1) + 1
            continue
        if not macro or token.value not in {"sail_verified", "sail_method", "sail_inherited_method"}:
            i += 1
            continue
        end = closing(stream, i + 2)
        at = i + 3
        seen_fn = False
        first_clause = "ensures" if token.value == "sail_inherited_method" else "requires"
        while at < end and stream[at].value != first_clause:
            value = stream[at].value
            require(value != "{", "runtime body before macro contracts")
            if value == "fn":
                require(not seen_fn, "multiple functions in checked envelope")
                seen_fn = True
            if value in {"(", "["}:
                at = closing(stream, at)
            at += 1
        require(seen_fn, "missing checked runtime function")
        clauses = ("ensures",) if first_clause == "ensures" else ("requires", "ensures")
        for clause in clauses:
            require(at + 1 < end and stream[at].value == clause
                and stream[at + 1].value == "(", "unsupported checked macro contract")
            at = closing(stream, at + 1) + 1
        if at < end and stream[at].value == "decreases":
            require(stream[at + 1].value == "(", "unsupported decreases clause")
            at = closing(stream, at + 1) + 1
        require(at < end and stream[at].value == "{" and closing(stream, at) == end - 1,
            "missing or trailing checked runtime body")
        bodies.append((at, end - 1))
        i = end + 1
    return bodies


@lru_cache(maxsize=16)
def _runtime_projection(text: str) -> tuple[tuple[str, ...], tuple[tuple[str, ...], ...], tuple[tuple[int, int], ...], tuple[tuple[str, ...], ...]]:
    """Cache immutable projections by complete source text, never path or identity.

    Every changed source still receives its own full parse. Repeated original
    files reuse the same checked token projection within this process only.
    """
    stream = tokens(text)
    bodies = _executable_envelopes(stream)
    skeleton: list[str] = []
    executable: list[list[str]] = []
    spans: list[tuple[int, int]] = []
    loop_contracts: list[list[str]] = []
    cursor = 0
    for begin, end in bodies:
        skeleton.extend(token.value for token in stream[cursor:begin])
        skeleton.append("<checked-runtime-body>")
        values: list[str] = []
        annotations: dict[int, int] = {}
        at = begin
        while at <= end:
            token = stream[at]
            if at in annotations:
                at = annotations[at]
                continue
            macro = at + 2 <= end and stream[at + 1].value == "!"
            ghost = (macro and token.value in {"sail_proof", "sail_spec", "sail_impl_spec"})
            ghost_group = (token.value in {"proof", "spec", "by"}
                and at + 1 <= end and stream[at + 1].value == "{")
            if ghost or ghost_group:
                at = closing(stream, at + (2 if ghost else 1)) + 1
                continue
            require(token.value != "fn" and not (macro
                and token.value in {"sail_verified", "sail_method", "sail_inherited_method"}),
                "nested runtime function outside mutation capture subset")
            if macro and token.value == "sail_while":
                require(stream[at + 2].value == "{" and stream[at + 3].value == "(",
                    "unsupported checked loop contract")
                loop_end = closing(stream, at + 2)
                contract_begin = closing(stream, at + 3) + 1
                body_begin = contract_begin
                for clause in ("invariant", "decreases"):
                    require(body_begin + 1 < loop_end and stream[body_begin].value == clause
                        and stream[body_begin + 1].value == "(",
                        "unsupported checked loop contract")
                    body_begin = closing(stream, body_begin + 1) + 1
                require(body_begin < loop_end and stream[body_begin].value == "{"
                    and closing(stream, body_begin) == loop_end - 1,
                    "missing or trailing checked loop body")
                loop_contracts.append([item.value for item in stream[contract_begin:body_begin]])
                annotations[contract_begin] = body_begin
            values.append(token.value)
            spans.append((token.begin, token.end))
            at += 1
        executable.append(values)
        cursor = end + 1
    skeleton.extend(token.value for token in stream[cursor:])
    return (tuple(skeleton), tuple(tuple(body) for body in executable),
        tuple(spans), tuple(tuple(contract) for contract in loop_contracts))


def runtime_mutation(source: str, before: str, after: str) -> None:
    """Require a mutation of runtime code with unchanged signatures/contracts.

    Erasing whole bodies yields an exact token skeleton: contracts, erased
    items and every other surrounding declaration must remain identical. The
    selected envelope must include runtime tokens; selecting only a proof
    block is refused even if a broader executable body contains it. Checked
    loop invariants and decreases clauses must also remain identical and do
    not supply runtime selector spans. This is an
    early source check; Stock's actual full function/obligation inventory still
    determines ordinary negative credit.
    """
    require(bool(before) and source.count(before) == 1 and before != after,
        "empty or ambiguous executable mutation")
    changed = source.replace(before, after)

    original_skeleton, original_runtime, spans, original_loops = _runtime_projection(source)
    changed_skeleton, changed_runtime, _, changed_loops = _runtime_projection(changed)
    require(original_skeleton == changed_skeleton,
        "mutation changes signature, contract or non-runtime source")
    require(original_loops == changed_loops, "mutation changes checked loop contract")
    begin = source.index(before)
    end = begin + len(before)
    require(any(begin <= lo and hi <= end for lo, hi in spans),
        "mutation selector contains no executable runtime tokens")
    require(len(original_runtime) == len(changed_runtime)
        and sum(left != right for left, right in zip(original_runtime, changed_runtime)) == 1,
        "mutation must change exactly one executable body, not only proof syntax")



def candidate_memory_policy(source: str) -> None:
    """Close the reviewed native clock/endpoint construction surface.

    This conservative source policy supplements full native binding hashes and
    actual tests. It is not a proof of Mutex, map or physical endpoint semantics.
    Unknown reset/alias syntax requires a fresh explicit review.
    """
    stream = tokens(source)
    marker = ["#", "[", "cfg", "(", "test", ")", "]", "mod", "tests", "{"]
    starts = [i for i in range(len(stream) - len(marker) + 1)
        if [token.value for token in stream[i:i + len(marker)]] == marker]
    require(len(starts) == 1, "candidate memory test boundary changed")
    require(closing(stream, starts[0] + len(marker) - 1) == len(stream) - 1,
        "candidate memory source after test boundary")
    stream = stream[:starts[0]]
    values = [token.value for token in stream]

    def occurrences(pattern: list[str]) -> int:
        return sum(values[i:i + len(pattern)] == pattern
            for i in range(len(values) - len(pattern) + 1))

    require(occurrences(["Clock", ":", ":", "start", "("]) == 1,
        "candidate clock must initialize only once in fresh Inner")
    require(occurrences(["Inner", ":", ":", "default", "("]) == 1,
        "candidate Inner must initialize only once in a fresh endpoint")
    require(occurrences(["Arc", ":", ":", "new", "("]) == 1,
        "candidate memory cannot replace or fabricate its endpoint")
    require(occurrences(["Accounting", ":", ":", "empty", "("]) == 1,
        "candidate accounting must initialize only once in a fresh Inner")
    for kind, name in (("enum", "StateBeforeGeneration"), ("struct", "SlotBeforeGeneration")):
        declarations = [i for i in range(len(stream) - 1)
            if values[i:i + 2] == [kind, name]]
        require(len(declarations) == 1 and declarations[0] > 0
            and stream[declarations[0] - 1].value == "]",
            "candidate old layout types must remain private attributed declarations")
        require(values.count(name) == 2,
            "candidate old layout types may only describe the original carrier for size_of")
    require(not ({"unsafe", "replace", "swap", "take", "try_unwrap", "into_inner", "cfg", "mod"} & set(values)),
        "candidate memory reset/alias syntax outside reviewed subset")
    for prefix in ("Self", "Default"):
        require(occurrences([prefix, ":", ":", "default", "("]) == 0,
            "candidate memory cannot reconstruct an existing Inner")
    for name in ("self", "inner"):
        require(occurrences(["*", name, "="]) == 0,
            "candidate memory cannot replace the held Inner")
    for i, token in enumerate(stream):
        if i and stream[i - 1].value == "." and token.value in {"clock", "inner", "accounting"}:
            allowed = {"clock": {"advance", "exhaust", "entry", "prepare"},
                "inner": {"lock", "as_ref", "is_some"},
                "accounting": {"apply", "reserve", "clear", "bytes", "pending"}}[token.value]
            require(i + 2 < len(stream) and stream[i + 1].value == "."
                and stream[i + 2].value in allowed,
                "candidate clock/endpoint/accounting assignment or mutable escape")
    names = [stream[i + 2].value for i in range(len(stream) - 2)
        if values[i:i + 2] == ["pub", "fn"]]
    require(names == ["tier_overhead", "candidate_bytes", "key", "charged_bytes", "write",
        "into_value", "remove_if_current", "new", "lookup", "insert_on_disk", "prepare", "remove", "stats"],
        "candidate memory public constructor/escape surface changed")
    for name in ("Inner", "MemoryEndpoint", "PreparedWrite", "ObservedValue", "MemoryTier"):
        declarations = [i for i in range(len(stream) - 1)
            if values[i:i + 2] == ["struct", name]]
        require(len(declarations) == 1, "missing/ambiguous native candidate record")
        declaration = declarations[0]
        public = declaration > 0 and stream[declaration - 1].value == "pub"
        require(public == (name in {"PreparedWrite", "ObservedValue", "MemoryTier"}),
            "native candidate visibility changed")
        previous = declaration - (2 if public else 1)
        require(previous >= 0 and stream[previous].value == "]",
            "native candidate attribute surface changed")
        start = previous - 1
        while start >= 0 and stream[start].value != "[":
            start -= 1
        require(start > 0 and stream[start - 1].value == "#"
            and [item.value for item in stream[start + 1:previous]] == ["derive", "(", "Debug", ")"],
            "native candidate derives an unchecked constructor")
        at = declarations[0] + 2
        while at < len(stream) and stream[at].value != "{":
            at += 1
        require(at < len(stream), "missing native candidate fields")
        end = closing(stream, at)
        fields = stream[at + 1:end]
        require(not any(token.value == "pub" and level
            for token, level in zip(fields, top_level(fields), strict=True)),
            "native candidate endpoint/state field is public")


def top_level(stream: list[Token]) -> list[bool]:
    """Identify declaration positions outside all Rust item/expression groups."""
    result = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    for token in stream:
        result.append(not stack)
        if token.value in pairs:
            stack.append(pairs[token.value])
        elif token.value in pairs.values():
            require(bool(stack) and stack.pop() == token.value, "crossed source groups")
            if not stack:
                result[-1] = True  # The end of a top-level item/group.
    require(not stack, "unclosed source groups")
    return result


def checked_item_envelopes(source: str, *, proof: bool = False,
        reviewed_ghost_items: frozenset[str] = frozenset(),
        reviewed_host_traits: frozenset[str] = frozenset(),
        reviewed_trait_impls: frozenset[str] = frozenset()) -> None:
    """Privileged modules cannot hide ordinary unchecked Rust constructors.

    Stock Verus intentionally ignores ordinary Rust outside its envelopes.
    Full-crate scope and function counters alone cannot detect such additions.
    Every runtime function here must belong to a checked function envelope;
    proof-only items are erased, and ordinary candidate adapters are separate.
    """
    stream = tokens(source)
    levels = top_level(stream)
    items = {"sail_code", "sail_spec", "sail_impl_spec", "sail_verified", "sail_method",
        "sail_inherited_method"}
    functions = {"sail_spec", "sail_impl_spec", "sail_verified", "sail_method",
        "sail_inherited_method"}
    if reviewed_host_traits:
        items.add("sail_trait")
        functions.add("sail_trait")
    if proof:
        items.add("verus")
        functions.add("verus")
    regions: list[tuple[int, int]] = []
    # The reviewed recording host is ordinary executable trait syntax inside
    # sail_code!, whose stock expansion verifies its inherited trait contract.
    # The same implementation outside that envelope would be ignored by Verus.
    code_regions = [(i, closing(stream, i + 2)) for i in range(len(stream) - 2)
        if stream[i].value == "sail_code" and stream[i + 1].value == "!"]
    for i, token in enumerate(stream):
        if token.value != "impl" or not any(begin < i < end for begin, end in code_regions):
            continue
        body = i + 1
        while body < len(stream) and stream[body].value != "{":
            body += 1
        require(body < len(stream), "unterminated checked trait implementation")
        end = closing(stream, body)
        values = [item.value for item in stream[i:end + 1]]
        if hashlib.sha256(repr(values).encode()).hexdigest() in reviewed_trait_impls:
            regions.append((i, end))
    for i in range(len(stream) - 2):
        if stream[i].value in functions and stream[i + 1].value == "!":
            end = closing(stream, i + 2)
            regions.append((i, end))
            values = [token.value for token in stream[i + 3:end]]
            identity = hashlib.sha256(repr(values).encode()).hexdigest()
            if stream[i].value == "sail_trait":
                require(identity in reviewed_host_traits,
                    "unreviewed host trait contract or implementation")
                continue
            if stream[i].value in {"sail_spec", "sail_impl_spec"}:
                erased = values
                if erased == [token.value for token in tokens("use vstd::prelude::*;")]:
                    continue
                if identity in reviewed_ghost_items:
                    continue  # Exact reviewed ghost types; no arbitrary shadowing.
                require(not ({"type", "struct", "enum", "union", "trait", "impl", "const", "static"} & set(erased))
                    and all(value != "use" or position > 0 and erased[position - 1] == "broadcast"
                        for position, value in enumerate(erased)),
                    "erased items cannot introduce executable/type bindings")
                for position, value in enumerate(erased):
                    if value == "fn":
                        require(position > 0 and erased[position - 1] in {"spec", "proof"},
                            "erased executable function can change name resolution")
    for i, token in enumerate(stream):
        if token.value == "fn":
            require(any(begin < i < end for begin, end in regions),
                "ordinary unchecked function in privileged source")
    at = 0
    while at < len(stream):
        value = stream[at].value
        if value == "#":
            start = at + 1
            if stream[start].value == "!":
                start += 1
            end = closing(stream, start)
            attribute = [t.value for t in stream[start + 1:end]]
            safe_lint = (len(attribute) == 11
                and attribute[:5] == ["allow", "(", "clippy", ":", ":"]
                and attribute[5] in {"redundant_as_str", "elidable_lifetime_names"}
                and attribute[6:9] == [",", "reason", "="]
                and attribute[9].startswith('"') and attribute[10] == ")")
            require(attribute == ["forbid", "(", "unsafe_code", ")"] or safe_lint,
                "unexpected privileged item attribute")
            at = end + 1
            continue
        begin = at
        if value == "pub":
            at += 1
            require(at < len(stream) and stream[at].value == "use", "ordinary public item outside checked envelope")
        if stream[at].value == "use":
            while at < len(stream) and not (levels[at] and stream[at].value == ";"):
                at += 1
            require(at < len(stream), "unterminated privileged import")
            at += 1
            continue
        if stream[at].value == "impl":
            while at < len(stream) and stream[at].value != "{":
                at += 1
            require(at < len(stream), "unterminated implementation")
            end = closing(stream, at)
            body = source[stream[at].end:stream[end].begin]
            checked_item_envelopes(body, proof=proof,
                reviewed_ghost_items=reviewed_ghost_items,
                reviewed_host_traits=reviewed_host_traits,
                reviewed_trait_impls=reviewed_trait_impls)
            at = end + 1
            continue
        at = begin
        while at + 2 < len(stream) and stream[at + 1].value == ":" and stream[at + 2].value == ":":
            at += 3
        require(at + 2 < len(stream) and stream[at].value in items
            and stream[at + 1].value == "!", "ordinary item outside checked envelope")
        at = closing(stream, at + 2) + 1
        if at < len(stream) and stream[at].value == ";":
            at += 1


def binding_environment(source: str, module: str) -> list[list[str]]:
    """Close the names used by stencils against alias/import substitution."""
    stream = tokens(source)
    levels = top_level(stream)
    imports = []
    item_begin = 0
    for i, token in enumerate(stream):
        if not levels[i]:
            continue
        value = token.value
        if value == "#":
            end = closing(stream, i + 1)
            kind = stream[i + 2].value
            if kind == "cfg":
                condition = [t.value for t in stream[i + 2:end]]
                after = end + 1
                while after < len(stream) and stream[after].value == "#":
                    after = checked_attribute(stream, after, {"derive", "serde", "doc"}) + 1
                following = [t.value for t in stream[after:after + 3]]
                native_socket = (module == "jobs"
                    and condition in (["cfg", "(", "unix", ")"], ["cfg", "(", "not", "(", "unix", ")", ")"])
                    and (following[:2] == ["fn", "short_socket"]
                        or condition == ["cfg", "(", "unix", ")"]
                            and (following == ["pub", "struct", "Client"] or following[:2] == ["impl", "Client"])))
                require(condition == ["cfg", "(", "test", ")"]
                    and [t.value for t in stream[end + 1:end + 3]] == ["mod", "tests"] or native_socket,
                    "conditional API declaration binding")
            elif kind == "must_use":
                require(module in {"jobs", "repair"} and [t.value for t in stream[i + 2:end]] == ["must_use"]
                    and [t.value for t in stream[end + 1:end + 3]] == ["pub", "fn"],
                    "unreviewed API function attribute")
            else:
                checked_attribute(stream, i, {"derive", "serde", "doc"})
        require(value not in ("type", "extern"), "API type alias/extern binding is outside capture subset")
        if value == "mod":
            prefix = [t.value for t in stream[item_begin:i]]
            require(i + 1 < len(stream) and stream[i + 1].value == "tests"
                and prefix == ["#", "[", "cfg", "(", "test", ")", "]"],
                "API module binding outside capture subset")
        if value in ("struct", "enum", "union") and i + 1 < len(stream):
            require(stream[i + 1].value not in {"String", "Vec", "Option", "u32", "u64", "i32", "bool"},
                "API declaration shadows a stencil primitive")
        if value == "use":
            end = i + 1
            while end < len(stream) and not (levels[end] and stream[end].value == ";"):
                end += 1
            require(end < len(stream), "unterminated API import")
            imports.append([t.value for t in stream[i:end + 1]])
        if i + 1 < len(stream) and stream[i + 1].value == "!":
            # The existing JSON_SCHEMA constant loads a schema string. An item
            # macro could instead introduce a hidden alias for a captured type.
            prefix = [t.value for t in stream[item_begin:i]]
            schema = prefix == ["pub", "const", "JSON_SCHEMA", ":", "&", "str", "="] and value == "include_str"
            socket_bound = (module == "jobs" and value == "cfg"
                and prefix == ["pub", "const", "MAX_SOCKET_PATH", ":", "usize", "=", "if"]
                and [t.value for t in stream[i + 2:closing(stream, i + 2) + 1]]
                    == ["(", "target_os", "=", '"linux"', ")"])
            require(schema or socket_bound, "API item macro outside capture subset")
        if value == ";" or value == "}":
            item_begin = i + 1
    expected = ["use serde::{Deserialize, Serialize};"]
    if module == "output":
        expected += ["use sail_ledger::{CheckRecord, Establishment, FunctionRecord, ObligationRecord, Outcome, Source, SourceSummary, Verdict,};"]
    elif module == "project":
        expected += ["use crate::output::Report;"]
    elif module == "repair":
        expected += ["use crate::project::ProjectResult;"]
    elif module == "jobs":
        expected = ["use std::collections::BTreeMap;", "use std::fmt::Write as _;",
            "use std::io::{self, Read, Write};", "use std::path::{Path, PathBuf};",
            "use serde::{Deserialize, Serialize};", "use serde_json::Value;"]
    else:
        raise SourceRefusal("unreviewed API declaration module")
    expected_tokens = [[token.value for token in tokens(text)] for text in expected]
    require(imports == expected_tokens, "API declaration binding environment changed")
    return imports


def record(source: str, name: str, kind: str,
        tuple_variants: dict[str, list[str]] | None = None) -> dict:
    """Capture every field/variant of one exact public declaration."""
    stream = tokens(source)
    levels = top_level(stream)
    locations = [i for i in range(len(stream) - 3)
        if levels[i] and [t.value for t in stream[i:i + 3]] == ["pub", kind, name]]
    require(len(locations) == 1, "missing or ambiguous declaration: " + name)
    begin = locations[0]
    # Inspect the complete immediately preceding attribute chain. cfg could
    # select a different Cargo declaration while a proof stencil sees this one.
    previous = begin - 1
    while previous >= 0 and stream[previous].value == "]":
        start = previous - 1
        depth = 1
        while start >= 0:
            if stream[start].value == "]":
                depth += 1
            elif stream[start].value == "[":
                depth -= 1
                if depth == 0:
                    break
            start -= 1
        require(start > 0 and stream[start - 1].value == "#", "invalid declaration attribute")
        checked_attribute(stream, start - 1, {"derive", "serde", "doc"})
        previous = start - 2
    at = begin + 3
    require(stream[at].value == "{", "generic/tuple/opaque declaration unsupported: " + name)
    end = closing(stream, at)
    body = stream[at + 1:end]
    fields: dict[str, list[str]] = {}
    variants: list[str] = []
    tuples: dict[str, list[str]] = {}
    at = 0
    while at < len(body):
        if body[at].value == "#":
            if (kind == "enum" and name == "Mode" and tuple_variants is not None
                    and [t.value for t in body[at:at + 5]] == ["#", "[", "default", "]", "Stock"]):
                at += 4
                continue
            at = checked_attribute(body, at, {"serde", "doc"}) + 1
            continue
        if kind == "enum":
            variant = body[at].value
            require(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", variant) is not None,
                "unsupported enum variant: " + name)
            require(variant not in variants, "duplicate enum variant")
            variants.append(variant)
            at += 1
            if at < len(body) and body[at].value == "(":
                require(tuple_variants is not None and variant in tuple_variants,
                    "non-unit enum unsupported: " + name)
                end_tuple = closing(body, at)
                actual = [t.value for t in body[at + 1:end_tuple]]
                require(actual == tuple_variants[variant], "enum payload representation changed: " + name)
                tuples[variant] = actual
                at = end_tuple + 1
            require(at == len(body) or body[at].value == ",", "non-unit enum unsupported: " + name)
            at += 1
            continue
        require(body[at].value == "pub" and at + 3 < len(body), "nonpublic/unsupported field: " + name)
        field = body[at + 1].value
        require(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", field) is not None
            and body[at + 2].value == ":", "invalid field: " + name)
        require(field not in fields, "duplicate field: " + name + "." + field)
        at += 3
        values: list[str] = []
        angle = 0
        stack: list[str] = []
        while at < len(body):
            value = body[at].value
            if value == "," and angle == 0 and not stack:
                break
            require(value not in ("!", "=", "#", "{" , "}"), "unsupported field type: " + name + "." + field)
            if value == "<":
                angle += 1
            elif value == ">":
                angle -= 1
                require(angle >= 0, "invalid generic type")
            elif value in ("(", "["):
                stack.append({"(": ")", "[": "]"}[value])
            elif value in (")", "]"):
                require(bool(stack) and stack.pop() == value, "invalid field type group")
            values.append(value)
            at += 1
        require(bool(values) and angle == 0 and not stack, "incomplete field type")
        fields[field] = values
        at += 1
    block = source[stream[begin].begin:stream[end].end].encode()
    result = {"name": name, "kind": kind, "fields": fields, "variants": variants,
        "declaration_sha256": hashlib.sha256(block).hexdigest()}
    if tuple_variants is not None:
        require(tuples == tuple_variants, "missing reviewed enum payload: " + name)
        result["tuple_fields"] = tuples
    return result


def key_stencil(snapshots: dict[str, bytes], expected: dict | None = None) -> tuple[bytes, dict]:
    """Project the complete real key identity/mode, with no invented fields."""
    path = "crates/sail-key/src/lib.rs"
    source = snapshots[path].decode()
    identity = record(source, "SolverIdentity", "struct")
    mode = record(source, "Mode", "enum", {"OptIn": ["String"]})
    require(identity["fields"] == {"name": ["String"], "version": ["String"],
        "binary_sha256": ["[", "u8", ";", "32", "]"], "platform": ["String"]},
        "solver identity representation changed")
    require(mode["variants"] == ["Default", "OptIn"], "key mode representation changed")
    inventory = {"source_sha256": hashlib.sha256(snapshots[path]).hexdigest(),
        "declarations": [identity, mode]}
    require(expected is None or expected == inventory, "key declaration inventory changed")
    lines = ["use vstd::prelude::*;", "verus! {"]
    for item in (identity, mode):
        lines.append("pub " + item["kind"] + " " + item["name"] + " {")
        if item["kind"] == "struct":
            lines += ["pub " + name + ": " + " ".join(value) + ","
                for name, value in item["fields"].items()]
        else:
            lines += [name + ("(" + " ".join(item["tuple_fields"][name]) + ")"
                if name in item["tuple_fields"] else "") + "," for name in item["variants"]]
        lines.append("}")
    lines.append("}")
    return ("\n".join(lines) + "\n").encode(), inventory


OUTPUT_FIELDS = {
    "Report": "schema_version mode command exit_code error summary crates engine solver",
    "CrateReport": "verus functions",
    "VerusResults": "errors success encountered_vir_error",
    "FunctionReport": "function verdict establishment evidence_tier unstable provenance obligations",
    "ProvenanceReport": "mode source checks_solved checks_served checks_unrecorded engine",
    "ObligationReport": "id",
    "Summary": "functions verified failed unknown provisional unstable",
    "EngineProvenance": "name manifest_sha256 established_by",
    "SolverInfo": "name version path sha256",
    "ErrorInfo": "",
}
PROJECT_FIELDS = {
    "TargetResult": "package_id target kind establishing_run reused requested complete scope_reuse report",
    "ProjectResult": "kind schema_version run_id workspace context_sha256 input_revision currentness exit_code error targets engine execution no_reuse",
    "RequestTarget": "package_id target kind",
    "ResolvedRequest": "run_id invocation workspace context_sha256 input_revision args engine function repair_package repair_target repair_kind targets",
    "RequestCapture": "version worker_id request",
    "EngineIdentity": "name manifest_sha256",
    "Execution": "verifier_invocations reused_targets",
    "NoReuse": "code reason verified_as",
}
OUTPUT_ENUMS = ("VerdictName", "EstablishmentName", "EvidenceTierName", "SourceSummaryName", "EngineEstablishment")
API_MODULES = ("output", "project", "jobs", "repair")


def candidate_forwarders(source: str) -> None:
    """Close the unverified sort boundary against report/target substitution.

    Correctness of the sort is unnecessary: the checked factories validate its
    candidate output. These thin forwarding bodies must nevertheless pass
    their actual borrowed report/target/project unchanged. The structural check
    is reviewed runner logic, not a claim that std sorting is formally proved.
    """
    expected = {
        "completed_presentation": """let orders: Vec<_> = if request.is_some()
            && matches!(project.currentness, crate::project::Currentness::Current)
            && project.exit_code == Some(0) {
                project.targets.iter().map(|target| candidate_order(&target.report)).collect()
            } else { Vec::new() };
            completed_presentation_ordered(project, request, &orders)""",
        "admit_declared_replay": """let orders = project.targets.iter()
            .map(|target| candidate_order(&target.report)).collect::<Vec<_>>();
            admit_declared_replay_ordered(project, original, current, &orders, paths, readback)""",
        "withhold_unbound_request_authority": """let orders: Vec<_> = if request.is_some()
            && matches!(project.currentness, crate::project::Currentness::Current)
            && project.exit_code == Some(0) {
                project.targets.iter().map(|target| candidate_order(&target.report)).collect()
            } else { Vec::new() };
            withhold_unbound_request_authority_ordered(project, request, &orders)""",
        "complete_submitted_request": """let orders: Vec<_> = project.targets.iter()
            .map(|target| candidate_order(&target.report)).collect();
            complete_submitted_request_ordered(arguments, mode, request, project, &orders)""",
        "complete_request": """let orders: Vec<_> = project.targets.iter()
            .map(|target| candidate_order(&target.report)).collect();
            complete_request_ordered(request, project, &orders)""",
        "complete_report": """let order = candidate_order(report);
            complete_report_ordered(report, policy, &order.forward, &order.inverse)""",
        "complete_exact_scope_report": """let order = candidate_order(report);
            complete_exact_scope_report_ordered(report, arguments, &order.forward, &order.inverse)""",
        "complete_exact_partial_request": """let orders = project.targets.iter()
            .map(|target| candidate_order(&target.report)).collect::<Vec<_>>();
            complete_exact_partial_request_ordered(request, project, &orders)""",
        "bind_replayed_exact_partial_project": """let orders = project.targets.iter()
            .map(|target| candidate_order(&target.report)).collect::<Vec<_>>();
            bind_replayed_exact_partial_project_ordered(project, original, current, &orders, readback)""",
        "bind_replayed_project": """let orders = project.targets.iter()
            .map(|target| candidate_order(&target.report)).collect::<Vec<_>>();
            bind_replayed_project_ordered(project, original, current, &orders, readback)""",
        "complete_whole_report": """let order = candidate_order(report);
            complete_whole_report_ordered(report, &order.forward, &order.inverse)""",
        "complete_target": """let order = candidate_order(&target.report);
            complete_target_ordered(target, &order.forward, &order.inverse)""",
        "complete_selection": """let orders: Vec<_> = project.targets.iter().map(|target| {
                if target.requested { candidate_order(&target.report) }
                else { ReportOrder { forward: Vec::new(), inverse: Vec::new(), } }
            }).collect(); complete_selection_ordered(project, &orders)""",
    }
    signatures = {
        "completed_presentation": "fn completed_presentation<'a>(project: &'a ProjectResult, request: Option<&'a ResolvedRequest>,) -> CompletedPresentation<'a>",
        "admit_declared_replay": "fn admit_declared_replay<'a, 'b>(project: ProjectResult, original: &'a ResolvedRequest, current: &'a ResolvedRequest, paths: Option<&RequestPathBinding<'a>>, readback: Option<&'a sail_acceptance::inputs::GuardedReadback<'b>>,) -> Option<DeclaredReplayAdmission<'a, 'b>>",
        "withhold_unbound_request_authority": "fn withhold_unbound_request_authority(project: &mut ProjectResult, request: Option<&ResolvedRequest>,) -> bool",
        "complete_submitted_request": "fn complete_submitted_request<'a>(arguments: &'a [String], mode: &'a Mode, request: &'a ResolvedRequest, project: &'a ProjectResult,) -> Option<SubmittedAuthority<'a>>",
        "complete_request": "fn complete_request<'a>(request: &'a ResolvedRequest, project: &'a ProjectResult,) -> Option<CompleteRequest<'a>>",
        "complete_report": "fn complete_report(report: &Report, policy: CompletionPolicy) -> Option<CompleteReport<'_>>",
        "complete_exact_scope_report": "fn complete_exact_scope_report<'a>(report: &'a Report, arguments: &'a [String],) -> Option<ExactScopeReport<'a>>",
        "complete_exact_partial_request": "fn complete_exact_partial_request<'a>(request: &'a ResolvedRequest, project: &'a ProjectResult,) -> Option<ExactPartialRequest<'a>>",
        "bind_replayed_exact_partial_project": "fn bind_replayed_exact_partial_project(project: &mut ProjectResult, original: &ResolvedRequest, current: &ResolvedRequest, readback: &sail_acceptance::inputs::GuardedReadback<'_>,) -> bool",
        "bind_replayed_project": "fn bind_replayed_project(project: &mut ProjectResult, original: &ResolvedRequest, current: &ResolvedRequest, readback: &sail_acceptance::inputs::GuardedReadback<'_>,) -> bool",
        "complete_whole_report": "fn complete_whole_report(report: &Report) -> Option<CompleteReport<'_>>",
        "complete_target": "fn complete_target(target: &TargetResult) -> Option<CompleteReport<'_>>",
        "complete_selection": "fn complete_selection(project: &ProjectResult) -> Option<CompleteSelection<'_>>",
        "candidate_order": "fn candidate_order(report: &Report) -> ReportOrder",
    }
    imports = [
        "use sail_acceptance::authority::CompletionPolicy;",
        "use crate::acceptance::{CompleteReport, CompleteRequest, CompleteSelection, CompletedPresentation, DeclaredReplayAdmission, ExactPartialRequest, ExactScopeReport, ReportOrder, RequestPathBinding, SubmittedAuthority, admit_declared_replay_ordered, bind_replayed_exact_partial_project_ordered, bind_replayed_project_ordered, complete_exact_partial_request_ordered, complete_exact_scope_report_ordered, complete_report_ordered, complete_request_ordered, complete_selection_ordered, complete_submitted_request_ordered, complete_target_ordered, complete_whole_report_ordered, completed_presentation_ordered, withhold_unbound_request_authority_ordered,};",
        "use crate::jobs::Mode;",
        "use crate::output::Report;", "use crate::project::{ProjectResult, ResolvedRequest, TargetResult};",
    ]
    stream = tokens(source)
    levels = top_level(stream)
    found = []
    actual_imports = []
    for at in range(len(stream) - 1):
        if levels[at] and stream[at].value == "use":
            end = at + 1
            while end < len(stream) and not (levels[end] and stream[end].value == ";"):
                end += 1
            require(end < len(stream), "incomplete candidate import")
            actual_imports.append([t.value for t in stream[at:end + 1]])
        if levels[at]:
            require(stream[at].value not in {"type", "struct", "enum", "union", "mod", "extern", "macro_rules", "static", "const"},
                "unexpected candidate module binding")
        if levels[at] and stream[at].value == "fn":
            name = stream[at + 1].value
            found.append(name)
            require(name in signatures, "extra ordinary authority adapter function")
            begin = at + 2
            while begin < len(stream) and stream[begin].value != "{":
                begin += 1
            require(begin < len(stream), "missing authority forwarding body")
            require([token.value for token in stream[at:begin]]
                == [token.value for token in tokens(signatures[name])], "candidate signature changed: " + name)
            if name == "candidate_order":
                continue
            require(at > 0 and stream[at - 1].value == "pub", "candidate export changed")
            end = closing(stream, begin)
            actual = [token.value for token in stream[begin + 1:end]]
            wanted = [token.value for token in tokens(expected[name])]
            require(actual == wanted, "untrusted authority forwarding changed: " + name)
    require(len(found) == 14 and set(found) == {*expected, "candidate_order"},
        "authority candidate adapter function inventory changed")
    require(actual_imports == [[t.value for t in tokens(text)] for text in imports],
        "candidate forwarding binding environment changed")


def api_stencils(snapshots: dict[str, bytes], expected: dict | None = None) -> tuple[dict[str, bytes], dict]:
    """Project actual declarations; every original field remains inventoried.

    Opaque ErrorInfo erases its payload, never the Option discriminant. Other
    dropped fields are unobserved presentation state. Accessing one in captured
    executable code fails Rust typing rather than receiving an assumed fact.
    """
    result: dict[str, bytes] = {}
    inventory: dict = {}
    for module, selected, enums in (
        ("output", OUTPUT_FIELDS, OUTPUT_ENUMS),
        ("project", PROJECT_FIELDS, ("Currentness", "VerifiedAs")),
        ("jobs", {}, ("Mode", "Currentness")),
        ("repair", {"Selection": "package target kind function"}, ()),
    ):
        source = snapshots["crates/sail-api/src/" + module + ".rs"].decode()
        bindings = binding_environment(source, module)
        declarations = [record(source, name, "struct") for name in selected]
        declarations += [record(source, name, "enum",
            {"Repair": [t.value for t in tokens("crate::repair::Selection")]}
                if module == "jobs" and name == "Mode" else None) for name in enums]
        stream = tokens(source)
        levels = top_level(stream)
        constant_name = "SCHEMA_VERSION" if module in {"output", "project"} else "VERSION"
        constants = [i for i in range(len(stream) - 7)
            if levels[i] and [t.value for t in stream[i:i + 3]] == ["pub", "const", constant_name]]
        require(len(constants) == 1, "missing or ambiguous schema version")
        constant = [t.value for t in stream[constants[0]:constants[0] + 8]]
        require(constant[:6] == ["pub", "const", constant_name, ":", "u32", "="]
            and constant[7] == ";" and constant[6].isdigit(), "unsupported schema version")
        inventory[module] = {"source_sha256": hashlib.sha256(source.encode()).hexdigest(), "bindings": bindings,
            "declarations": declarations, "schema_constant": constant,
            "retained_fields": selected}
        lines = ["use vstd::prelude::*;"]
        if module == "project":
            lines.append("use crate::output::{Report,ErrorInfo};")
        lines += ["verus! {", " ".join(constant)]
        if module == "project":
            capture_versions = [i for i in range(len(stream) - 7)
                if levels[i] and [t.value for t in stream[i:i + 3]]
                    == ["pub", "const", "REQUEST_CAPTURE_VERSION"]]
            require(len(capture_versions) == 1, "missing or ambiguous request capture version")
            require(capture_versions[0] == 0 or stream[capture_versions[0] - 1].value != "]",
                "attributed request capture version outside closed projection")
            capture_constant = [t.value for t in stream[capture_versions[0]:capture_versions[0] + 8]]
            require(capture_constant[:6] == ["pub", "const", "REQUEST_CAPTURE_VERSION", ":", "u32", "="]
                and capture_constant[7] == ";" and capture_constant[6].isdigit(),
                "unsupported request capture version")
            inventory[module]["request_capture_constant"] = capture_constant
            lines.append(" ".join(capture_constant))
        for declaration in declarations:
            name = declaration["name"]
            lines.append("pub " + declaration["kind"] + " " + name + " {")
            if declaration["kind"] == "enum":
                for variant in declaration["variants"]:
                    payload = declaration.get("tuple_fields", {}).get(variant)
                    lines.append(variant + ("(" + " ".join(payload).replace(": :", "::") + ")"
                        if payload is not None else "") + ",")
            else:
                names = set(selected[name].split())
                require(names <= declaration["fields"].keys(), "required field missing: " + name)
                lines += ["pub " + field + ": " + " ".join(value).replace(": :", "::") + ","
                    for field, value in declaration["fields"].items() if field in names]
            lines.append("}")
        lines.append("}")
        result[module + ".rs"] = ("\n".join(lines) + "\n").encode()
    if expected is not None:
        require(inventory == expected, "API declaration/stencil inventory changed")
    return result, inventory

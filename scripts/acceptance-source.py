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

# The reviewed patterns have no anchors or lookbehind; retain exact offsets.
_RAW_STRING_PREFIX = re.compile(r'(?:br|cr|r)(#*)"')
_CHAR_LITERAL = re.compile(r"'(?:\\.|[^'\\\n])'")
_LIFETIME = re.compile(r"'[A-Za-z_][A-Za-z_0-9]*")
_WORD = re.compile(r"[A-Za-z_][A-Za-z_0-9]*|[0-9][A-Za-z_0-9]*")


CHECKED_FUNCTION_MACROS = frozenset({"sail_verified", "sail_method", "sail_no_unwind_method", "sail_drop"})
ERASED_MACROS = frozenset({"sail_spec", "sail_impl_spec", "sail_proof", "sail_snapshot"})

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
    return result


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


def executable_envelopes(source: str) -> list[tuple[int, int]]:
    """Locate checked runtime bodies after balanced macro contract groups.

    This recognizes only the two reviewed checked-function envelope grammars. A
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
        if macro and token.value in ERASED_MACROS:
            i = closing(stream, i + 2) + 1
            continue
        if token.value in {"proof", "spec", "by"} and i + 1 < len(stream) and stream[i + 1].value == "{":
            i = closing(stream, i + 1) + 1
            continue
        if not macro or token.value not in CHECKED_FUNCTION_MACROS:
            i += 1
            continue
        end = closing(stream, i + 2)
        at = i + 3
        if token.value == "sail_drop":
            require([item.value for item in stream[at:at + 7]] ==
                ["fn", "drop", "(", "&", "mut", "self", ")"],
                "unsupported checked Drop signature")
            at += 7
            require(at < end and stream[at].value == "{" and closing(stream, at) == end - 1,
                "missing or trailing checked Drop body")
            bodies.append((at, end - 1))
            i = end + 1
            continue
        seen_fn = False
        while at < end and stream[at].value != "requires":
            value = stream[at].value
            require(value != "{", "runtime body before macro contracts")
            if value == "fn":
                require(not seen_fn, "multiple functions in checked envelope")
                seen_fn = True
            if value in {"(", "["}:
                at = closing(stream, at)
            at += 1
        require(seen_fn, "missing checked runtime function")
        for clause in ("requires", "ensures"):
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


class RuntimeProjection(NamedTuple):
    """Exact runtime tokens and their retained verification boundaries."""

    skeleton: tuple[str, ...]
    runtime: tuple[tuple[str, ...], ...]
    spans: tuple[tuple[int, int], ...]
    loops: tuple[tuple[str, ...], ...]
    proofs: tuple[tuple[tuple[str, ...], ...], ...]
    proof_positions: tuple[tuple[int, ...], ...]


def _runtime_projection(text: str) -> RuntimeProjection:
    """Project runtime tokens and retain every signature and loop contract.

    Source is parsed on each call; this guard caches no admission result.
    """
    stream = tokens(text)
    bodies = _executable_envelopes(stream)
    skeleton: list[str] = []
    executable: list[list[str]] = []
    spans: list[tuple[int, int]] = []
    loop_contracts: list[list[str]] = []
    proof_groups: list[tuple[tuple[str, ...], ...]] = []
    proof_positions: list[tuple[int, ...]] = []
    cursor = 0
    for begin, end in bodies:
        skeleton.extend(token.value for token in stream[cursor:begin])
        skeleton.append("<checked-runtime-body>")
        values: list[str] = []
        body_proofs: list[tuple[str, ...]] = []
        body_positions: list[int] = []
        annotations: dict[int, int] = {}
        at = begin
        while at <= end:
            token = stream[at]
            if at in annotations:
                at = annotations[at]
                continue
            macro = at + 2 <= end and stream[at + 1].value == "!"
            ghost = (macro and token.value in ERASED_MACROS)
            ghost_group = (token.value in {"proof", "spec", "by"}
                and at + 1 <= end and stream[at + 1].value == "{")
            if ghost or ghost_group:
                proof_end = closing(stream, at + (2 if ghost else 1)) + 1
                body_proofs.append(tuple(item.value for item in stream[at:proof_end]))
                body_positions.append(len(values))
                at = proof_end
                # Its trailing empty statements are erased too; otherwise a
                # proof-only false assertion can masquerade as runtime (#508).
                while at <= end and stream[at].value == ";":
                    at += 1
                continue
            require(token.value != "fn" and not (macro
                and token.value in CHECKED_FUNCTION_MACROS),
                "nested runtime function outside mutation capture subset")
            verification_only = {"assert", "assert_forall", "assume", "admit",
                "reveal", "reveal_with_fuel", "hide", "broadcast", "calc"}
            require(not (token.value in verification_only
                and at + 1 <= end and stream[at + 1].value != "!"),
                "verification-only syntax outside erased proof group")
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
        proof_groups.append(tuple(body_proofs))
        proof_positions.append(tuple(body_positions))
        cursor = end + 1
    skeleton.extend(token.value for token in stream[cursor:])
    return RuntimeProjection(tuple(skeleton), tuple(tuple(body) for body in executable),
        tuple(spans), tuple(tuple(contract) for contract in loop_contracts), tuple(proof_groups),
        tuple(proof_positions))


def _preserved_proof_positions(original: tuple[str, ...], changed: tuple[str, ...],
        original_positions: tuple[int, ...], changed_positions: tuple[int, ...]) -> None:
    """Allow runtime edits in only one interval between unchanged proof groups.

    Comparing exact intervals avoids ambiguous matching of repeated tokens.
    This deliberately refuses multi-interval controls; their intended faults
    need separate review rather than admitting relocated proof guidance.
    """
    def intervals(values: tuple[str, ...], positions: tuple[int, ...]) -> list[tuple[str, ...]]:
        boundaries = (0, *positions, len(values))
        return [values[left:right] for left, right in zip(boundaries, boundaries[1:])]

    old = intervals(original, original_positions)
    new = intervals(changed, changed_positions)
    require(sum(left != right for left, right in zip(old, new)) <= 1,
        "mutation changes runtime intervals on both sides of erased proof code")


def runtime_mutation(source: str, before: str, after: str) -> None:
    """Require a mutation of runtime code with unchanged signatures/contracts.

    Erasing whole bodies yields an exact token skeleton: contracts, erased
    items and every other surrounding declaration must remain identical. The
    selected envelope must include runtime tokens; selecting only a proof
    block is refused even if a broader executable body contains it. Checked
    loop invariants, decreases clauses and erased proof groups remain identical and do
    not supply runtime selector spans. This is an
    early source check; Stock's actual full function/obligation inventory still
    determines ordinary negative credit.
    """
    require(bool(before) and source.count(before) == 1 and before != after,
        "empty or ambiguous executable mutation")
    changed = source.replace(before, after)

    original_skeleton, original_runtime, spans, original_loops, original_proofs, original_positions = _runtime_projection(source)
    changed_skeleton, changed_runtime, _, changed_loops, changed_proofs, changed_positions = _runtime_projection(changed)
    require(original_skeleton == changed_skeleton,
        "mutation changes signature, contract or non-runtime source")
    require(original_loops == changed_loops, "mutation changes checked loop contract")
    require(original_proofs == changed_proofs, "mutation changes erased proof code")
    for old_body, new_body, old_positions, new_positions in zip(
            original_runtime, changed_runtime, original_positions, changed_positions):
        _preserved_proof_positions(old_body, new_body, old_positions, new_positions)
    begin = source.index(before)
    end = begin + len(before)
    require(any(begin <= lo and hi <= end for lo, hi in spans),
        "mutation selector contains no executable runtime tokens")
    require(len(original_runtime) == len(changed_runtime)
        and sum(left != right for left, right in zip(original_runtime, changed_runtime)) == 1,
        "mutation must change exactly one executable body, not only proof syntax")


def candidate_indexed_policy(source: str) -> None:
    """Close the reviewed private actual map/index owner's construction API.

    This supplements checked operation bodies; it supplies no validity, native
    primitive, clock, accounting, allocation or solver-origin assumption.
    """
    prelude = "#[cfg(sail_verus)]\nuse vstd::prelude::*;"
    require(source.count(prelude) == 1, "indexed owner conditional prelude changed")
    checked_item_envelopes(source.replace(prelude, ""))
    stream = tokens(source)
    values = [token.value for token in stream]
    require(not ({"derive", "type", "trait", "const", "static", "unsafe", "extern", "include"} & set(values)),
        "indexed owner unreviewed constructor/type escape")
    require(values.count("cfg") == 1 and values.count("impl") == 1,
        "indexed owner conditional/trait implementation escape")
    expected_imports = (
        "use vstd::prelude::*;", "use std::collections::BTreeMap;",
        "use crate::cache_candidate::{Entry, Slot};",
        "use crate::cache_counted::CountedSlots;", "use crate::cache_order;",
        "use crate::inputs::byte_eq;",
    )
    levels = top_level(stream)
    require(sum(token.value == "use" and level for token, level in zip(stream, levels, strict=True))
        == len(expected_imports), "indexed owner import surface changed")
    for item in expected_imports:
        wanted = [t.value for t in tokens(item)]
        require(sum(values[i:i + len(wanted)] == wanted for i in range(len(values) - len(wanted) + 1)) == 1,
            "indexed owner actual type/selector import changed")
    require(values.count("sail_code") == 1 and values.count("struct") == 1
        and values.count("enum") == 0, "indexed owner unreviewed executable type")
    expected_declaration = [token.value for token in tokens("""pub struct IndexedSlots<Value, Stamp> {
        slots: CountedSlots<Value, Stamp>, order: BTreeMap<u64, [u8; 32]>, }""")]
    require(sum(values[i:i + len(expected_declaration)] == expected_declaration
        for i in range(len(values) - len(expected_declaration) + 1)) == 1,
        "indexed owner must retain exactly one private actual map and index")
    start = values.index("IndexedSlots", values.index("struct"))
    begin = next(i for i in range(start, len(values)) if values[i] == "{")
    end = closing(stream, begin)
    require("pub" not in values[begin + 1:end], "indexed owner state fields must remain private")
    for view in (
        "pub closed spec fn slots_view(&self) -> CountedSlots<Value, Stamp> { self.slots }",
        "pub closed spec fn order_view(&self) -> Map<u64, [u8; 32]> { self.order@ }",
        "pub open spec fn valid(&self) -> bool { paired(self.slots_view(), self.order_view()) }",
        "pub open spec fn lookup_view(&self, key: Seq<u8>) -> Option<Slot<Value, Stamp>> { self.slots_view().lookup_view(key) }",
    ):
        wanted = [t.value for t in tokens(view)]
        require(sum(values[i:i + len(wanted)] == wanted
            for i in range(len(values) - len(wanted) + 1)) == 1,
            "indexed owner view must bind its actual private fields")
    expected_headers = {'empty': 'e9b3dde822a6ca59d5991a13ede4805abb25e105db54e1aa63fdde48e539985c',
     'owner_bytes': '6fee8003e429c64986e52054b59678cfa5ab82184b1dbd8d36e661d3737fe9b2',
     'get': '6941ee79079b88f3a261e5655c66a30c2408aebc9de1e8a91063db6141694a52',
     'order': '34c34424bfd964b17f90c13ce87acb99e570a5c633f66f4c056b4bab05b69068',
     'is_empty': '09c544d7d66ef2757d84a5e6a93d0220155419cc032d773967dca6d2a9ff810d',
     'len': '7b5362d175ab2a47172cd77cdf14917bcf656b66a114c4a7f91b88ad8587943c',
     'capacity': '31530c741a580b1fb2d8cb6d83ba1e1d6009eaf4017cb793cfefc6f6417f7186',
     'bucket_count': '81a0f518917116da8b22a9eaef0272b9ac2f10db0641082bcca07811f3f867f3',
     'insert_new': 'b8ade14028804b3b1dad1a11f4fa04112bce166bc18e2893e9088604d2747268',
     'take': '0edd77639d63fc7a2272290414021f0512a00d5546708dd59b6461292c5399dc',
     'touch': '33e9de282ab25dd3c9efb077f12b322f2464ce91442600ec68c6481f39514103',
     'take_oldest': '5ebaa4294db21abe7f1373c271e1a1d3f2574926e12a7ab5884ee915f846422a',
     'renumber': '6473451fc53500cc551cdfa762b72652bd4eedc735e4f1e6c2d635ec08e53130',
     'clear': '34c36c3183cc1d15ed7e1ec9a81eee7df966510a267877beef623bad3cd9f8a0'}
    envelopes = _executable_envelopes(stream)
    actual_headers = {}
    for i, token in enumerate(stream):
        if token.value == "fn" and stream[i - 1].value not in {"spec", "proof"}:
            require(stream[i - 1].value == "pub", "indexed owner hidden runtime entry")
            following = [(begin, end) for begin, end in envelopes if begin > i]
            require(bool(following), "indexed owner unchecked runtime function")
            begin, _ = following[0]
            name = stream[i + 1].value
            require(name not in actual_headers, "indexed owner duplicate runtime entry")
            actual_headers[name] = hashlib.sha256(repr(values[i - 1:begin]).encode()).hexdigest()
    require(actual_headers == expected_headers,
        "indexed owner runtime signature/precondition/postcondition changed")
    paired_start = next(i - 3 for i in range(3, len(stream))
        if stream[i].value == "fn" and stream[i + 1].value == "paired")
    paired_begin = next(i for i in range(paired_start, len(stream)) if stream[i].value == "{")
    paired_end = closing(stream, paired_begin)
    require(hashlib.sha256(repr(values[paired_start:paired_end + 1]).encode()).hexdigest()
        == "2426517e601f9dc05b731813c1d74546968f5e133d76dbe82a1fd18c8c17c1b0", "indexed owner full-key/tick correspondence predicate changed")


def checked_item_envelopes(source: str, *, proof: bool = False) -> None:
    """Validate exact source under the complete current envelope policy.

    Only built-in string and boolean inputs share successful results.
    Unusual argument subclasses retain the original uncached behavior.
    """
    if type(source) is not str or type(proof) is not bool:
        return _checked_item_envelopes(source, proof=proof)
    return _cached_item_envelopes(source, proof=proof)


@lru_cache(maxsize=128)
def _cached_item_envelopes(source: str, *, proof: bool) -> None:
    """Reuse successful syntax checks, retaining every other admission check.

    The validator reads literal local syntax sets and immutable tokenizer
    patterns. It does not read mutable binding or derive policies. Exceptions
    leave no cached success. Source discovery, declaration/import bindings,
    compiled programs, file hashes and final drift checks are not cached here.
    """
    return _checked_item_envelopes(source, proof=proof)


def _checked_item_envelopes(source: str, *, proof: bool = False) -> None:
    """Privileged modules cannot hide ordinary unchecked Rust constructors.

    Stock Verus intentionally ignores ordinary Rust outside its envelopes.
    Full-crate scope and function counters alone cannot detect such additions.
    Every runtime function here must belong to a checked function envelope;
    proof-only items are erased, and ordinary candidate adapters are separate.
    """
    stream = tokens(source)
    levels = top_level(stream)
    items = {"sail_code", "sail_spec", "sail_impl_spec", *CHECKED_FUNCTION_MACROS}
    functions = {"sail_spec", "sail_impl_spec", *CHECKED_FUNCTION_MACROS}
    if proof:
        items.add("verus")
        functions.add("verus")
    regions: list[tuple[int, int]] = []
    for i in range(len(stream) - 2):
        if stream[i].value in functions and stream[i + 1].value == "!":
            end = closing(stream, i + 2)
            regions.append((i, end))
            if stream[i].value in {"sail_spec", "sail_impl_spec"}:
                erased = [token.value for token in stream[i + 3:end]]
                if erased == [token.value for token in tokens("use vstd::prelude::*;")]:
                    continue
                declarations = {"type", "struct", "enum", "union", "trait", "impl", "const", "static"}
                # A Set.union call is an expression, not an erased declaration.
                require(all(value not in declarations or value == "union"
                        and position > 0 and erased[position - 1] == "."
                        and position + 1 < len(erased) and erased[position + 1] == "("
                        for position, value in enumerate(erased))
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
                and attribute[:9] == ["allow", "(", "clippy", ":", ":", "redundant_as_str", ",", "reason", "="]
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
            checked_item_envelopes(body, proof=proof)
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
                require([t.value for t in stream[i + 2:end]] == ["cfg", "(", "test", ")"]
                    and [t.value for t in stream[end + 1:end + 3]] == ["mod", "tests"],
                    "conditional API declaration binding")
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
            require(prefix == ["pub", "const", "JSON_SCHEMA", ":", "&", "str", "="]
                and value == "include_str", "API item macro outside capture subset")
        if value == ";" or value == "}":
            item_begin = i + 1
    expected = ["use serde::{Deserialize, Serialize};"]
    if module == "output":
        expected += ["use sail_ledger::{CheckRecord, Establishment, FunctionRecord, ObligationRecord, Outcome, Source, SourceSummary, Verdict,};"]
    else:
        expected += ["use crate::output::Report;"]
    expected_tokens = [[token.value for token in tokens(text)] for text in expected]
    require(imports == expected_tokens, "API declaration binding environment changed")
    return imports


def record(source: str, name: str, kind: str) -> dict:
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
    at = 0
    while at < len(body):
        if body[at].value == "#":
            at = checked_attribute(body, at, {"serde", "doc"}) + 1
            continue
        if kind == "enum":
            variant = body[at].value
            require(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", variant) is not None,
                "unsupported enum variant: " + name)
            require(variant not in variants, "duplicate enum variant")
            variants.append(variant)
            at += 1
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
    return {"name": name, "kind": kind, "fields": fields, "variants": variants,
        "declaration_sha256": hashlib.sha256(block).hexdigest()}


OUTPUT_FIELDS = {
    "Report": "schema_version command exit_code error summary crates engine",
    "CrateReport": "verus functions",
    "VerusResults": "errors success encountered_vir_error",
    "FunctionReport": "function verdict establishment evidence_tier unstable provenance obligations",
    "ProvenanceReport": "source checks_solved checks_served checks_unrecorded",
    "ObligationReport": "id",
    "Summary": "functions verified failed unknown provisional unstable",
    "EngineProvenance": "established_by",
    "ErrorInfo": "",
}
PROJECT_FIELDS = {
    "TargetResult": "package_id target kind requested complete scope_reuse report",
    "ProjectResult": "currentness exit_code error targets",
}
OUTPUT_ENUMS = ("VerdictName", "EstablishmentName", "EvidenceTierName", "SourceSummaryName", "EngineEstablishment")


def candidate_forwarders(source: str) -> None:
    """Close the unverified sort boundary against report/target substitution.

    Correctness of the sort is unnecessary: the checked factories validate its
    candidate output. These four thin forwarding bodies must nevertheless pass
    their actual borrowed report/target/project unchanged. The structural check
    is reviewed runner logic, not a claim that std sorting is formally proved.
    """
    expected = {
        "complete_report": """let order = candidate_order(report);
            complete_report_ordered(report, policy, &order.forward, &order.inverse)""",
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
        "complete_report": "fn complete_report(report: &Report, policy: CompletionPolicy) -> Option<CompleteReport<'_>>",
        "complete_whole_report": "fn complete_whole_report(report: &Report) -> Option<CompleteReport<'_>>",
        "complete_target": "fn complete_target(target: &TargetResult) -> Option<CompleteReport<'_>>",
        "complete_selection": "fn complete_selection(project: &ProjectResult) -> Option<CompleteSelection<'_>>",
        "candidate_order": "fn candidate_order(report: &Report) -> ReportOrder",
    }
    imports = [
        "use sail_acceptance::authority::CompletionPolicy;",
        "use crate::acceptance::{CompleteReport, CompleteSelection, ReportOrder, complete_report_ordered, complete_selection_ordered, complete_target_ordered, complete_whole_report_ordered,};",
        "use crate::output::Report;", "use crate::project::{ProjectResult, TargetResult};",
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
    require(len(found) == 5 and set(found) == {*expected, "candidate_order"},
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
        ("project", PROJECT_FIELDS, ("Currentness",)),
    ):
        source = snapshots["crates/sail-api/src/" + module + ".rs"].decode()
        bindings = binding_environment(source, module)
        declarations = [record(source, name, "struct") for name in selected]
        declarations += [record(source, name, "enum") for name in enums]
        stream = tokens(source)
        levels = top_level(stream)
        constants = [i for i in range(len(stream) - 7)
            if levels[i] and [t.value for t in stream[i:i + 3]] == ["pub", "const", "SCHEMA_VERSION"]]
        require(len(constants) == 1, "missing or ambiguous schema version")
        constant = [t.value for t in stream[constants[0]:constants[0] + 8]]
        require(constant[:6] == ["pub", "const", "SCHEMA_VERSION", ":", "u32", "="]
            and constant[7] == ";" and constant[6].isdigit(), "unsupported schema version")
        inventory[module] = {"source_sha256": hashlib.sha256(source.encode()).hexdigest(), "bindings": bindings,
            "declarations": declarations, "schema_constant": constant,
            "retained_fields": selected}
        lines = ["use vstd::prelude::*;"]
        if module == "project":
            lines.append("use crate::output::{Report,ErrorInfo};")
        lines += ["verus! {", " ".join(constant)]
        for declaration in declarations:
            name = declaration["name"]
            lines.append("pub " + declaration["kind"] + " " + name + " {")
            if declaration["kind"] == "enum":
                lines += [variant + "," for variant in declaration["variants"]]
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

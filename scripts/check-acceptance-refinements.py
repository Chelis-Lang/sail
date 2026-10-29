#!/usr/bin/env python3
"""Check captured production Rust bodies and their acceptance refinements.

Requires Python 3.11+. This runner is ordinary reviewed/tested code. Its reports
distinguish checked executable bodies, declaration projections and surrounding
runtime source that is bound to the commit but remains outside the proof.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "proofs/refinement/manifest.json"
SYNTAX = "crates/sail-acceptance/src/syntax.rs"
# This reviewed source uses the identical argument/type/body metavariables in
# Cargo and stock profiles. Changes require an explicit runner review, beyond
# merely updating a manifest. Runtime-only implementation drift must refuse.
SYNTAX_SHA256 = '59aa95f06f0b72b70d7bf5b07e56ac5bb9c7c8233449aa552e2e860b42c3b887'
LIB_SHA256 = '7491d6a48750e6f9eb4b95542a75ade6d8e24ff4f187982ea71d5c96ad4d8a74'
API_LIB_SHA256 = '6beca003b5ca18d6a580bef44294e8e2d35f7d3ba890c78122e3aea89dc78070'
IMPORT_SHA256 = {'crates/sail-acceptance/src/query.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/inputs.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/authority.rs': 'd0adfcd0a1e64a0addc3db112f9f9fc707f3f4c68bd7b907047a00b8d7f4abb4', 'crates/sail-api/src/acceptance.rs': 'afa1214ec210fc66be045700eea9103fa502d5ea671036c954667bd98c75de2e', 'crates/sail-acceptance/src/cache_bucket.rs': '0f1bd74898e2c02c3a9dc0703c4fcb15a32b5d4431f74172a26018ef4a0d1cd6', 'crates/sail-acceptance/src/cache_candidate.rs': '85238c0ed2426827074088424c45b3dc79a69e10d7c2d7f241cd5ead444f44f5', 'crates/sail-acceptance/src/cache_chain.rs': '4d9b3f7d879733de38602d3633ed2a12dafc2386ae621cc6899e7d60130e371b', 'crates/sail-acceptance/src/cache_counted.rs': '1df767f37e1ea099c348e412df93db8c27cd084a3337289c46fa73a318b9c846', 'crates/sail-acceptance/src/cache_indexed.rs': 'c059b97d820bf4f191c4ac54e7b7f6bcb3d89b31003f56f44e561dcc30eef4ae', 'crates/sail-acceptance/src/cache_order.rs': '0dfeaac0763cbedbf43aae474c01742258d53c1cfd8536c33e9df18870e308f7', 'crates/sail-acceptance/src/cache_slots.rs': '75121078b2cbc3942f6ec3f50b30335f8ce1257f86860752405dbb039678f642', 'crates/sail-api/src/request_mode.rs': 'a4bfe95fa72922b47e2352be0ec4d3210ba815ef8e9899908981db73b61ad68f', 'crates/sail-acceptance/src/parser_grammar.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_split.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_lex.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_holding.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_command.rs': 'fdac6af05d6d252c01c144b07227e6694dba6e53883d3f897d4c409245b7fd3f', 'crates/sail-acceptance/src/query_policy.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d'}
MODE_CORE = 'crates/sail-api/src/request_mode.rs'
HOLDING_CORE = 'crates/sail-acceptance/src/query_holding.rs'
HOLDING_NATIVE = 'crates/sail-shim/src/cached.rs'
HOLDING_NATIVE_SHA256 = '78d454b6c28dae7dc289a2b6aaf5f1bf92a8d7de172c322c411e8474fe32e40f'
FILENAME_NATIVE = 'crates/sail-verus/src/managed/source_inputs.rs'
FILENAME_NATIVE_SHA256 = 'b83b3ff01ed9fa797fe363de085108cd06fa058166db57e47d6481dff1321d35'
MODE_NATIVE = 'crates/sail-verus/src/adapter.rs'
MODE_NATIVE_SHA256 = '3f15d2cb3c8fa55e47bcbb3874eba1ab74adf2120943aa0e98bc43451962b045'
REQUIRED_CONTROLS = {'query': ['fifo_wrong_stack', 'partial_pair_consumes_head', 'replay_off_by_one', 'noisy_publication', 'corrupt_recovery', 'ungated_batch_release', 'incomplete_permutation', 'candidate-clock-start-exhausted', 'candidate-clock-reuses-ordinal', 'candidate-clock-wraps-exhaustion', 'candidate-clock-exhaustion-restarts-domain', 'candidate-entry-full-factory-always-refuses', 'candidate-entry-wrong-ticket-key', 'candidate-entry-wrong-slot-generation', 'candidate-entry-wrong-original-state', 'candidate-bound-entry-full-factory-always-refuses', 'candidate-observation-replaces-generation', 'candidate-touch-replaces-generation', 'candidate-completion-skips-original-key', 'candidate-completion-skips-original-generation', 'candidate-completion-skips-pending-state', 'candidate-completion-promotes-refusal', 'candidate-completion-full-operation-always-withholds', 'candidate-removal-skips-original-key', 'candidate-removal-skips-original-generation', 'candidate-removal-full-operation-always-withholds', 'candidate-constructor-swaps-original-metadata', 'candidate-original-key-getter-replaces-key', 'candidate-disk-arguments-replace-mode', 'candidate-disk-arguments-replace-reply', 'candidate-disk-arguments-replace-effort', 'candidate-disk-arguments-replace-command', 'candidate-disk-arguments-replace-provenance', 'candidate-disk-arguments-replace-solver', 'candidate-disk-getter-replaces-mode', 'candidate-disk-getter-replaces-reply', 'candidate-disk-getter-replaces-effort', 'candidate-disk-getter-replaces-command', 'candidate-disk-getter-replaces-provenance', 'candidate-disk-getter-replaces-solver', 'candidate-disk-getter-replaces-key', 'candidate-prepared-transfer-discards-original-pending', 'candidate-prepared-full-factory-always-refuses', 'candidate-prepared-factory-drops-issued-handle', 'candidate-prepared-factory-replaces-disk-key', 'candidate-result-projector-substitutes-error-kind', 'candidate-result-projector-always-withholds', 'candidate-accounting-starts-nonempty', 'candidate-accounting-bytes-projection-swapped', 'candidate-accounting-pending-projection-swapped', 'candidate-accounting-clear-keeps-bytes', 'candidate-accounting-clear-keeps-pending', 'candidate-accounting-reservation-omits-charge', 'candidate-accounting-reservation-omits-pending', 'candidate-accounting-reservation-misclassifies-slot', 'candidate-accounting-reservation-whole-always-refuses', 'candidate-accounting-retirement-omits-charge', 'candidate-accounting-settlement-omits-pending', 'candidate-accounting-retirement-classification-swapped', 'candidate-accounting-transition-replaces-original-variant', 'candidate-order-accounting-omitted', 'candidate-order-retirement-omitted', 'candidate-order-wrong-original-tick', 'candidate-order-retirement-applies-to-written', 'candidate-order-retirement-always-removes', 'candidate-order-clears-other-history', 'candidate-order-replaces-original-update', 'chain-read-prefix-only', 'chain-read-always-refuses', 'chain-take-prefix-only', 'chain-take-loses-visited-neighbors', 'chain-replacement-keeps-previous-match', 'chain-insert-drops-original-neighbors', 'chain-drain-omits-tail-detachment', 'chain-drop-omits-actual-drain', 'slots-read-wrong-route', 'slots-read-always-refuses', 'slots-vacant-insertion-installs-empty-chain', 'slots-insert-loses-original-return', 'slots-take-retires-colliding-neighbors', 'slots-take-loses-original-return', 'slots-clear-omitted', 'chain-touch-prefix-only', 'chain-touch-update-omitted', 'chain-touch-loses-visited-neighbors', 'slots-touch-wrong-route', 'slots-touch-wrong-recency', 'slots-touch-returns-new-instead-of-original-tick', 'counted-factory-starts-nonempty', 'counted-new-key-count-not-incremented', 'counted-replacement-count-treated-as-new', 'counted-len-reports-routes-instead-of-entries', 'counted-empty-always-refuses', 'counted-read-always-refuses', 'counted-insert-always-refuses', 'counted-insert-inverts-actual-absence', 'counted-insert-loses-counter-update', 'counted-insert-loses-original-overflow-refusal', 'counted-take-does-not-decrement', 'counted-take-increments', 'counted-take-loses-original-return', 'counted-touch-resets-entry-count', 'counted-touch-loses-original-recency', 'counted-capacity-underreported', 'counted-routes-reports-entry-count', 'counted-clear-retains-stale-count', 'generation-exhaustion-readback-always-false', 'actual-node-carrier-underreported', 'actual-map-owner-carrier-underreported', 'actual-route-chain-carrier-underreported', 'actual-counted-owner-carrier-underreported', 'order-transfer-omitted', 'order-transfer-clears-original', 'order-oldest-removal-omitted', 'order-oldest-returns-wrong-key', 'order-oldest-always-refuses', 'order-oldest-returns-wrong-tick', 'order-oldest-skips-original-first', 'indexed-factory-nonempty-index', 'indexed-read-always-refuses', 'indexed-count-reports-routes', 'indexed-capacity-underreported', 'indexed-insert-key-conflict-ignored', 'indexed-insert-tick-conflict-ignored', 'indexed-insert-index-update-omitted', 'indexed-insert-wrong-index-key', 'indexed-insert-all-refused', 'indexed-take-index-retirement-omitted', 'indexed-take-loses-other-slots', 'indexed-touch-conflict-ignored', 'indexed-touch-index-retirement-omitted', 'indexed-touch-index-write-omitted', 'indexed-oldest-takes-wrong-original', 'indexed-renumber-slot-update-omitted', 'indexed-renumber-loses-unprocessed-original', 'indexed-renumber-reports-zero', 'indexed-clear-retains-index', 'indexed-empty-always-refuses', 'indexed-empty-always-accepts', 'held-replace-drops-displaced-original', 'held-take-silently-discards-original', 'held-metadata-detach-discards-original', 'held-orphan-metadata-creates-annotation', 'held-release-before-reorders-original-current', 'held-replace-returns-wrong-original'], 'inputs': ['input-ignore-present-value', 'input-ignore-current-stamp', 'input-ignore-required-read-presence', 'input-ignore-inverse-permutation', 'filename-control-byte-refusal', 'filename-escape-prefix', 'filename-escape-value', 'filename-literal-byte', 'filename-cardinality', 'filename-constructive'], 'authority': ['verification-enabled', 'effective-verification-enabled', 'whole-terminal-summary', 'confirmed-function', 'whole-command', 'requested-coverage', 'summary-count', 'normal-exit', 'fresh-selection', 'obligation-uniqueness', 'provenance-counts', 'report-mode-missing-options-refuse', 'report-mode-effective-enabled', 'report-mode-effective-disabled', 'report-mode-original-separator', 'report-mode-preserves-prefix-no-verify', 'report-mode-keeps-disabled-original-flag', 'report-mode-exact-original-token', 'report-mode-no-wrong-enabled-append', 'report-mode-no-spurious-append', 'report-mode-required-separator-append', 'report-mode-required-disable-append', 'report-mode-exact-separator-marker', 'report-mode-exact-disable-marker', 'report-mode-full-operation-always-refuses'], 'parser': ['frame-ignore-doubled-quote', 'frame-ignore-input-comment', 'split-list-opening-byte', 'split-list-original-byte', 'split-list-comment-domain', 'split-list-completion-edge', 'split-list-chunk-cursor', 'split-list-chunk-original-order', 'split-list-incomplete-bytes', 'split-list-full-push-always-pending', 'split-list-full-chunk-overconsumes-suffix', 'split-list-copy-original-bytes', 'split-outer-atom-opening-byte', 'split-outer-atom-original-start', 'split-outer-atom-original-append', 'split-outer-atom-input-domain', 'split-outer-atom-cr-trim', 'split-outer-atom-delimiter-reprocess', 'split-outer-comment-cr-end', 'split-outer-idle-comment-side', 'split-outer-cursor-consumed-count', 'split-outer-list-chunk-offset', 'split-outer-full-transition-always-idle', 'split-outer-full-chunk-drops-atom-items', 'split-outer-full-chunk-drops-second-stray', 'split-outer-full-finish-drops-atom', 'split-outer-pending-item-readback', 'split-outer-full-copy-changes-idle-to-comment', 'split-chunk-transition-full-drops-items', 'split-chunk-transition-full-drops-outer-items', 'split-chunk-transition-full-zero-consumed', 'split-chunk-transition-full-zero-outer-consumed', 'lexer-trivia-original-start', 'lexer-trivia-cr-terminates-comment', 'lexer-word-delimiter-guard', 'lexer-word-original-byte', 'lexer-quoted-original-close', 'lexer-string-doubled-quote-step', 'lexer-string-closing-byte-included', 'lexer-binary-digit-domain', 'lexer-hex-original-kind', 'lexer-keyword-nonempty', 'lexer-full-next-always-empty', 'lexer-sticky-original-error', 'lexer-sticky-refusal-cannot-resume', 'lexer-constructor-original-source', 'lexer-full-copy-retains-failure', 'lexer-full-text-always-empty', 'output-single-response-always-accept', 'output-single-response-always-refuse', 'output-single-response-offset-zero-required', 'output-single-response-finished-state-required', 'parser-delimiter-byte-set', 'parser-stream-space-byte-set'], 'command': ['policy-tactic-head', 'policy-get-prefix', 'policy-whole-head', 'lexer-head-borrows-original-span', 'lexer-head-original-open-offset', 'lexer-head-symbol-kind-required', 'lexer-head-opening-kind-required', 'command-check-tactic-category', 'command-output-tactic-category', 'command-head-exact-push', 'command-head-get-prefix', 'command-decimal-original-digit', 'command-decimal-overflow', 'command-keyword-kind', 'command-keyword-original-offset', 'command-value-original-start', 'command-value-closing-depth', 'command-value-original-end', 'command-value-error-propagation', 'command-next-original-token', 'command-push-default-count', 'command-pop-distinct-kind', 'command-option-keyword-value-swap', 'command-get-info-keyword', 'command-classify-original-name', 'command-classify-always-refuses', 'command-expectation-original-readback', 'command-expectation-classifier-error', 'command-expectation-always-refuses']}
PARSER_NATIVE_SHA256 = {'crates/sail-smtlib/Cargo.toml': 'fc43ad36257fa518845876fdf142a962a550c174bec344edb88336de551b26e8', 'crates/sail-smtlib/src/lib.rs': '067e315883f7fc17a4609cab43e15ff83d2efa10b7d52164df366779a136e462', 'crates/sail-smtlib/src/command.rs': '0d86d41b3356676c5fbf2364e8ea9d52cc1942eb533d73f0d12fbf5b453cdde9', 'crates/sail-smtlib/src/sexpr.rs': '24e54367e5dc70927aede26c8cc4e9ecb8e946027ca5f25a2e3e0aa6a3c7f945', 'crates/sail-smtlib/src/error.rs': '7403bf10fcc8dc7e4e199dcd1deda767717423ccbec42235b648c9005f5892e2', 'crates/sail-smtlib/src/lex.rs': '7ef0d5b442b673afd01bcb7a8bdfc0701fc83ca9086ccad89b1d7259ddb3a834', 'crates/sail-smtlib/src/split.rs': 'e5837d77f15783ece7769240af7c140d9cb799d6ec5bee533419f272f3b030b6', 'Cargo.lock': '59c6b803fb6552661cd5363676cf0564a07033c083c1433b78ee620fd9dad7df'}
PARSER_NATIVE_BINDINGS = {'crates/sail-smtlib/Cargo.toml': ['native parser module/target assembly; not Stock-compiled'], 'crates/sail-smtlib/src/lib.rs': ['native parser module/target assembly; not Stock-compiled'], 'crates/sail-smtlib/src/command.rs': ['native shallow classifier facade delegates to checked query_command; not Stock-compiled'], 'crates/sail-smtlib/src/sexpr.rs': ['unchanged native s-expression consumer of original token spans'], 'crates/sail-smtlib/src/error.rs': ['native parser module/target assembly; not Stock-compiled'], 'crates/sail-smtlib/src/lex.rs': ['Lexer::new/clone', 'Iterator::next/tokenize', 'native_failure exact error-code/offset adapter', 'Token/TokenKind shared declaration reexports'], 'crates/sail-smtlib/src/split.rs': ['CommandSplitter/ResponseSplitter new/clone/push/finish', 'is_single_response exact shared delegate', 'Item/ItemKind shared declaration reexports'], 'Cargo.lock': ['reviewed internal parser dependency lock edge; not a Stock body']}

COMMAND_NATIVE_SHA256 = {'crates/sail-smtlib/src/command.rs': '0d86d41b3356676c5fbf2364e8ea9d52cc1942eb533d73f0d12fbf5b453cdde9', 'crates/sail-smtlib/src/lex.rs': '7ef0d5b442b673afd01bcb7a8bdfc0701fc83ca9086ccad89b1d7259ddb3a834', 'crates/sail-session/Cargo.toml': '4d1c12f98e87f7e40cf46febe3e667d9d5576e88b5e28ba5d8856bdc18e6ba65', 'crates/sail-session/src/lib.rs': '2e8697b1aaf0b11d7c5ad68283fad65d200307dca6724ade1ee3e4b0514b9722'}
COMMAND_NATIVE_BINDINGS = {'crates/sail-smtlib/src/command.rs': ['native command classify/error conversion and shared declaration reexports; not Stock-compiled'], 'crates/sail-smtlib/src/lex.rs': ['native lexical error-code and offset conversion; not Stock-compiled'], 'crates/sail-session/Cargo.toml': ['native Session dependency assembly; not Stock-compiled'], 'crates/sail-session/src/lib.rs': ['Session::expects_output delegates with the original print-success readback; other Session operations unproved']}

HOST_SYNTAX = 'crates/sail-acceptance/src/host_syntax.rs'
HOST_SYNTAX_SHA256 = 'b15f41f8272da9c311b7ef8393a8f30c047309355d2f952e699515598f0b936c'
WRITE_CORE = 'crates/sail-acceptance/src/cache_write.rs'
WRITE_NATIVE_SHA256 = {'crates/sail-cache/src/persist.rs': '956ec4c117fb2b722a9a2454a9f1a60adf76e58a5a61397b1f144d4894d23260', 'crates/sail-cache/src/lib.rs': 'cb93ecf4e90cd2b5cd2ffbd91c4b96bd3bd94371b66a2d6ed73227cb42aa64f1', 'crates/sail-cache/Cargo.toml': '6a1745183f4b19ec13a14efe27db5fca800733332d5782e446adb66f8e160666'}
WRITE_TRAIT_SHA256 = 'e1f217be87981cbd3d19140f85d8c7e8434568bd092cee9555efab9da58cf851'
WRITE_SCRIPT_IMPL_SHA256 = "b14cfdbc045ce226b03452823bf1706890c04c58817293134e9d1f2fc1ce12b4"
IMPORT_SHA256.update({
    WRITE_CORE: '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d',
    'crates/sail-acceptance/src/cache_record.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d',
})
REQUIRED_CONTROLS["persistence"] = ['record-ignore-header-newline', 'record-change-separator', 'record-keep-separator-in-reply', 'write-omitted-full-write', 'write-omitted-rename-call', 'write-premature-metadata', 'write-cleanup-unowned-name', 'write-substitute-original-error', 'write-omit-owned-cleanup']
IMPORT_SHA256.update({'crates/sail-acceptance/src/key_prefix.rs': '485fb1bca954ce3852d3feb51c3675ae57b2d83e63df3e09af91ab0cc7ed66ac', 'crates/sail-acceptance/src/query_delivery.rs': 'dd5692fc8c75546d449a2c623708ec6748a7be654391385107f4fa20e971d08e', 'crates/sail-acceptance/src/query_raw.rs': 'ec1eb6a68200f46d67214fd1b184c95edfcf6c7c4d39c19ff6f8d24ff18378c9', 'crates/sail-key/src/prefix_factory.rs': '22f9a04f035b109591f920519d7423c7de774bfc66315194b3939f291bf62988', 'crates/sail-acceptance/src/query_prefix.rs': 'e811a0f475ec2b7004bb3cea6fafac088f1826082706206f51b14c392c4822c9'})

COMPONENTS = {
    "persistence": {"models": [], "core": ["inputs", "cache_record", "cache_write"]},
    "query": {"models": ["context", "query_association"], "core": ['inputs', 'query', 'cache_candidate', 'cache_bucket', 'cache_chain', 'cache_slots', 'cache_counted', 'cache_order', 'cache_indexed', 'query_holding']},
    "inputs": {"models": ["context"], "core": ["inputs"]},
    "authority": {"models": ["context", "authority"], "core": ["inputs", "authority"]},
    "parser": {"models": [], "core": ["inputs", "parser_grammar", "query_split", "query_lex"]},
    "command": {"models": [], "core": ["inputs", "parser_grammar", "query_lex", "query_policy", "query_command"]},
}

# Real typed construction is checked; these complete native adapters are only
# reviewed/source-bound primitive and caller boundaries, not Stock-compiled.
KEY_NATIVE = 'crates/sail-key/src/lib.rs'
KEY_FACTORY = 'crates/sail-key/src/prefix_factory.rs'
PREFIX_NATIVE_BINDINGS = {'crates/sail-key/src/lib.rs': ['Sha256Host', 'PrefixHasher', 'SessionPrefix', 'CommandFrame', 'InputDelivery', 'DeliveredInput'], 'crates/sail-shim/src/cached.rs': ['SolverPipe', 'Solving', 'start_solver', 'observe', 'run', 'finish_run', 'finish_input', 'Pairer::expect', 'Pairer::finish'], 'crates/sail-shim/src/trace.rs': ['Tap prefix exactness guard']}
PREFIX_NATIVE_SHA256 = {'crates/sail-key/src/lib.rs': '1c159a81f44d0cc81dae6550eed621da61d6102aa23f5f09705e981b4eca0109', 'crates/sail-shim/src/cached.rs': '78d454b6c28dae7dc289a2b6aaf5f1bf92a8d7de172c322c411e8474fe32e40f', 'crates/sail-shim/src/trace.rs': '6f09131bf1b31289d7c12acaab284d2b6818c914d41000e75b2405988ed187ce'}
PREFIX_HOST_TRAITS = {'crates/sail-acceptance/src/key_prefix.rs': ('HashHost', '07062a121a098f5dee4b51714ff0b575b1e6568347408344bc4aea5f3fe8be9a'), 'crates/sail-acceptance/src/query_delivery.rs': ('InputPipe', '127030cacff4e231147ffc843933effa0c754b3ad1963e460c5e981bcde92220')}
PREFIX_SCRIPT_IMPLS = {'crates/sail-acceptance/src/key_prefix.rs': ('RecordingHash', 'de6e87c96d820b1610f24bc6b31aa3a64ba0e8d329446d476e08fcedcdcf5c3e'), 'crates/sail-acceptance/src/query_delivery.rs': ('ScriptPipe', '8378afe345bf6880d519cf675e3b5db5e4b01f6900781bef674f6c1fd7019d72')}
REQUIRED_CONTROLS["prefix"] = ['key-solver-platform', 'key-mode-domain', 'key-raw-bytes', 'key-end-domain', 'key-counter-refusal', 'key-feed-order', 'key-silence-domain', 'key-state-copy', 'key-length-word', 'key-typed-platform', 'key-typed-optin', 'owner-command-feed', 'owner-tail-feed', 'owner-command-endpoint', 'owner-sticky-refusal', 'delivery-omit-actual-pipe-write', 'delivery-hash-different-write-bytes', 'delivery-heal-failed-delivery', 'delivery-omit-terminal-key-binding', 'delivery-accept-closed-pipe', 'delivery-omit-owned-pipe-close', 'delivery-seed-drops-mode', 'delivery-seal-without-freezing', 'delivery-seal-unmatched-prefix', 'delivery-write-after-seal', 'delivery-token-different-prefix', 'delivery-token-different-domain', 'delivery-parser-drops-mode', 'owner-policy-head', 'owner-original-append', 'owner-command-end', 'owner-hash-tail', 'raw-ignore-command-bytes', 'raw-ignore-gap', 'raw-skip-separators', 'frame-premature-root-close', 'head-ignore-framing']
COMPONENTS["prefix"] = {"models": [], "core": ["inputs", "parser_grammar", "query_lex", "query_policy", "query_raw", "key_prefix", "query_prefix", "query_delivery"]}


def module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


COMMON = module("sail_acceptance_model_checker", "scripts/check-acceptance-proofs.py")
SOURCE = module("sail_acceptance_source_capture", "scripts/acceptance-source.py")
require = COMMON.require
Refusal = COMMON.Refusal


def validate_assembly(snapshots: dict[str, bytes]) -> None:
    """Bind proof module paths and private tokens to the real Cargo assembly."""
    library = "crates/sail-acceptance/src/lib.rs"
    require(hashlib.sha256(snapshots[library]).hexdigest() == LIB_SHA256,
        "production module assembly changed; explicit runner review required")
    require(hashlib.sha256(snapshots["crates/sail-api/src/lib.rs"]).hexdigest() == API_LIB_SHA256,
        "API factory module assembly changed; explicit runner review required")
    leaf = tomllib.loads(snapshots["crates/sail-acceptance/Cargo.toml"].decode())
    require(set(leaf) == {"package", "lints"} and leaf["lints"] == {"workspace": True},
        "custom leaf target/dependency/feature/build configuration")
    package = leaf["package"]
    require(set(package) == {"name", "description", "version", "edition", "rust-version", "publish", "license", "repository"}
        and package["name"] == "sail-acceptance"
        and all(package[key] == {"workspace": True} for key in
            ("version", "edition", "rust-version", "publish", "license", "repository")),
        "unexpected executable leaf package")
    workspace = tomllib.loads(snapshots["Cargo.toml"].decode())
    require(not ({"patch", "replace"} & set(workspace)), "workspace dependency overrides require review")
    protected_packages = ("sail-acceptance", "sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon", "sail-smtlib", "sail-session", "sail-key")
    for name in protected_packages:
        require(workspace["workspace"]["dependencies"][name] == {"path": "crates/" + name},
            "workspace protected dependency redirected: " + name)
    api = tomllib.loads(snapshots["crates/sail-api/Cargo.toml"].decode())
    require(set(api) == {"package", "dependencies", "target", "lints"}
        and set(api["package"]) == set(package)
        and api["package"]["name"] == "sail-api"
        and all(api["package"][key] == {"workspace": True} for key in
            ("version", "edition", "rust-version", "publish", "license", "repository")),
        "API target/feature/build assembly changed")
    for name in ("sail-api", "sail-cache", "sail-shim", "sail-verus", "sail-daemon", "sail-smtlib"):
        manifest = tomllib.loads(snapshots["crates/" + name + "/Cargo.toml"].decode())
        require(manifest["dependencies"]["sail-acceptance"] == {"workspace": True}, "runtime dependency redirected")
    smt = tomllib.loads(snapshots["crates/sail-smtlib/Cargo.toml"].decode())
    require(set(smt) == {"package", "dependencies", "dev-dependencies", "lints"}
        and smt["dependencies"] == {"sail-acceptance": {"workspace": True}}, "parser target/dependency assembly changed")
    session = tomllib.loads(snapshots["crates/sail-session/Cargo.toml"].decode())
    require(set(session) == {"package", "dependencies", "dev-dependencies", "lints"}
        and session["dependencies"] == {"sail-smtlib": {"workspace": True}},
        "Session target/dependency assembly changed")
    locked = tomllib.loads(snapshots["Cargo.lock"].decode())
    parser_locks = [package for package in locked["package"] if package["name"] == "sail-smtlib"]
    require(len(parser_locks) == 1 and parser_locks[0]["dependencies"] == ["proptest", "sail-acceptance"],
        "parser internal dependency lock changed")
    lex = snapshots["crates/sail-acceptance/src/query_lex.rs"].decode()
    lex_tokens = [token.value for token in SOURCE.tokens(lex)]
    text_methods = [i for i, token in enumerate(lex_tokens[:-1])
        if token == "fn" and lex_tokens[i + 1] == "text"]
    require(len(text_methods) == 1 and lex_tokens[text_methods[0] - 5:text_methods[0]]
        == ["#", "[", "must_use", "]", "pub"], "original Token::text must-use annotation lost")
    key_manifest = tomllib.loads(snapshots["crates/sail-key/Cargo.toml"].decode())
    require(set(key_manifest) == {"package", "dependencies", "dev-dependencies", "lints"}
        and set(key_manifest["package"]) == set(package)
        and key_manifest["package"]["name"] == "sail-key"
        and all(key_manifest["package"][key] == {"workspace": True} for key in
            ("version", "edition", "rust-version", "publish", "license", "repository"))
        and key_manifest["lints"] == {"workspace": True}
        and key_manifest["dependencies"] == {"sail-acceptance": {"workspace": True},
            "sail-session": {"workspace": True}, "sha2": {"workspace": True}},
        "key factory target/dependency assembly changed")
    protected = {
        "crates/sail-acceptance/src/query.rs": ("Fifo", "BoundPair", "CorrelatedQueue", "ReplayCursor", "PublicationBatch", "RecoveryRecord"),
        "crates/sail-acceptance/src/inputs.rs": ("OriginalCapture", "OriginalGuard", "Readback"),
        "crates/sail-api/src/acceptance.rs": ("CompleteReport", "CompleteSelection"),
        "crates/sail-acceptance/src/cache_candidate.rs": ("Clock", "Slot", "Ticket", "Held", "Accounting", "CandidateWrite", "WriteArguments", "Prepared"),
        "crates/sail-acceptance/src/cache_bucket.rs": ("Bucket",),
        "crates/sail-acceptance/src/cache_chain.rs": ("Chain",),
        "crates/sail-acceptance/src/cache_slots.rs": ("Slots",),
        "crates/sail-acceptance/src/cache_counted.rs": ("CountedSlots",),
        "crates/sail-acceptance/src/cache_indexed.rs": ("IndexedSlots",),
        "crates/sail-acceptance/src/query_split.rs": ("AtomFrame", "StreamPosition", "ListFrame", "StreamSplitter"),
        "crates/sail-acceptance/src/query_lex.rs": ("Lexer",),
        HOLDING_CORE: ("Holding",),
        "crates/sail-acceptance/src/query_raw.rs": ("RawTranscript", "PrefixFeed"),
        "crates/sail-acceptance/src/key_prefix.rs": ("Prefix", "RecordingHash"),
        "crates/sail-acceptance/src/query_prefix.rs": ("QueryPrefix", "FramedCommand"),
        "crates/sail-acceptance/src/query_delivery.rs": ("Delivery", "Delivered", "ScriptPipe"),
        WRITE_CORE: ("WriteCommit", "WriteScript"),
    }
    for path, names in protected.items():
        stream = SOURCE.tokens(snapshots[path].decode())
        for name in names:
            declarations = [i for i in range(len(stream) - 2)
                if [t.value for t in stream[i:i + 3]] == ["pub", "struct", name]]
            require(len(declarations) == 1, "missing/ambiguous protected token " + name)
            at = declarations[0] + 3
            while at < len(stream) and stream[at].value != "{":
                at += 1
            require(at < len(stream), "missing protected fields")
            end = SOURCE.closing(stream, at)
            fields = stream[at + 1:end]
            require(not any(token.value == "pub" and level
                for token, level in zip(fields, SOURCE.top_level(fields), strict=True)),
                "public authority/state field permits unverified construction: " + name)
            # Deriving a new constructor (Default/Deserialize) would bypass the
            # function inventory and private-field minting boundary.
            previous = declarations[0] - 1
            while previous >= 0 and stream[previous].value == "]":
                start = previous - 1
                depth = 1
                while start >= 0:
                    if stream[start].value == "]": depth += 1
                    elif stream[start].value == "[":
                        depth -= 1
                        if depth == 0: break
                    start -= 1
                require(start > 0 and stream[start - 1].value == "#", "invalid token attribute")
                attribute = [t.value for t in stream[start + 1:previous]]
                require(attribute == ["derive", "(", "Debug", ")"]
                    or path == "crates/sail-acceptance/src/cache_candidate.rs" and name == "Ticket"
                        and attribute == ["derive", "(", "Debug", ",", "Clone", ",", "Copy", ")"]
                    or attribute and attribute[0] == "doc", "authority type derives unchecked constructor")
                previous = start - 2


def dependency_paths(component: str) -> list[str]:
    """The closed executable/model dependency set for one complete driver."""
    entry = COMPONENTS[component]
    result = [SYNTAX]
    if component in {"persistence", "prefix"}:
        result.append(HOST_SYNTAX)
    result += ["proofs/acceptance/" + stem + ".rs" for stem in entry["models"]]
    result += ["crates/sail-acceptance/src/" + stem + ".rs" for stem in entry["core"]]
    result.append("proofs/refinement/" + component + ".rs")
    if component == "persistence":
        result += list(WRITE_NATIVE_SHA256)
    if component == "prefix":
        result += [KEY_NATIVE, KEY_FACTORY, *[p for p in PREFIX_NATIVE_SHA256 if p != KEY_NATIVE]]
    if component == "parser":
        result += list(PARSER_NATIVE_SHA256)
    if component == "command":
        result += list(COMMAND_NATIVE_SHA256)
    if component == "query":
        result.append(HOLDING_NATIVE)
    if component == "inputs":
        result.append(FILENAME_NATIVE)
    if component == "authority":
        result += ["crates/sail-api/src/acceptance.rs", "crates/sail-api/src/acceptance_sort.rs",
            MODE_CORE, "crates/sail-api/src/output.rs", "crates/sail-api/src/project.rs", MODE_NATIVE]
    return result


def source_roles(entry: dict) -> dict:
    """Keep actual checked bodies separate from their bound native caller."""
    return {
        "checked_source": [path for path in entry["dependencies"] if path not in
            {SYNTAX, HOST_SYNTAX, *PREFIX_NATIVE_SHA256, *WRITE_NATIVE_SHA256, *PARSER_NATIVE_SHA256, *COMMAND_NATIVE_SHA256, "crates/sail-api/src/acceptance_sort.rs",
                "crates/sail-api/src/output.rs", "crates/sail-api/src/project.rs", MODE_NATIVE, HOLDING_NATIVE,
                FILENAME_NATIVE}],
        "reviewed_native_prefix": {path: PREFIX_NATIVE_BINDINGS[path]
            for path in entry["dependencies"] if path in PREFIX_NATIVE_BINDINGS},
        "reviewed_prefix_host": [path for path in entry["dependencies"] if path in PREFIX_HOST_TRAITS],
        "reviewed_native_parser": {path: PARSER_NATIVE_BINDINGS[path]
            for path in entry["dependencies"] if path in PARSER_NATIVE_BINDINGS},
        "reviewed_native_command": {path: COMMAND_NATIVE_BINDINGS[path]
            for path in entry["dependencies"] if path in COMMAND_NATIVE_BINDINGS},
        "reviewed_syntax": [p for p in entry["dependencies"] if p in {SYNTAX, HOST_SYNTAX}],
        "reviewed_persistence_host": [p for p in entry["dependencies"] if p == WRITE_CORE],
        "reviewed_native_persistence": {p: ["native file/serde operations and actual DiskCache caller; not Stock-compiled"]
            for p in entry["dependencies"] if p in WRITE_NATIVE_SHA256},
        "declaration_projections": [path for path in entry["dependencies"]
            if path.endswith(("/output.rs", "/project.rs")) or path == KEY_NATIVE],
        "structurally_checked_forwarders": [path for path in entry["dependencies"]
            if path.endswith("/acceptance_sort.rs")],
        "bound_native_callers": {
            **{path: ["report_verification_arguments", "cargo_driver_options_with", "effective_verus_arguments"]
                for path in entry["dependencies"] if path == MODE_NATIVE},
            **{path: ["held_entry", "Pairer::drain", "Pairer::release", "Pairer::finish"]
                for path in entry["dependencies"] if path == HOLDING_NATIVE},
            **{path: ["dependencies", "update"]
                for path in entry["dependencies"] if path == FILENAME_NATIVE},
        },
    }


def compiled_subset(path: str, content: bytes) -> None:
    """Refuse source loading, unchecked specs and alternate executable cfgs."""
    if path in PREFIX_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == PREFIX_NATIVE_SHA256[path],
            "native prefix/hash/delivery adapter changed; explicit source review required")
        return
    if path == FILENAME_NATIVE:
        require(hashlib.sha256(content).hexdigest() == FILENAME_NATIVE_SHA256,
            "actual filename admission caller changed; explicit native binding review required")
        return
    if path == HOLDING_NATIVE:
        require(hashlib.sha256(content).hexdigest() == HOLDING_NATIVE_SHA256,
            "actual held-response caller changed; explicit native binding review required")
        return
    if path == MODE_NATIVE:
        require(hashlib.sha256(content).hexdigest() == MODE_NATIVE_SHA256,
            "actual report-mode caller changed; explicit native binding review required")
        return
    if path == HOST_SYNTAX:
        require(hashlib.sha256(content).hexdigest() == HOST_SYNTAX_SHA256,
            "host trait erasure implementation changed; explicit runner review required")
        return
    if path in WRITE_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == WRITE_NATIVE_SHA256[path],
            "native persistence adapter changed; explicit source review required")
        return
    if path == SYNTAX:
        require(hashlib.sha256(content).hexdigest() == SYNTAX_SHA256,
            "syntax erasure implementation changed; explicit runner review required")
        return
    if path.startswith("proofs/acceptance/"):
        COMMON.standalone(content.decode())
        return
    if path in PARSER_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == PARSER_NATIVE_SHA256[path],
            "native parser adapter/assembly changed; explicit source review required")
        return
    if path in COMMAND_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == COMMAND_NATIVE_SHA256[path],
            "native command adapter/Session assembly changed; explicit source review required")
        return
    text = content.decode()
    if path in IMPORT_SHA256:
        stream = SOURCE.tokens(text)
        imports = []
        for i, token in enumerate(stream):
            if token.value == "use":
                end = i
                while end < len(stream) and stream[end].value != ";":
                    end += 1
                require(end < len(stream), "unterminated captured import")
                imports.append([item.value for item in stream[i:end + 1]])
        require(hashlib.sha256(json.dumps(imports, separators=(",", ":")).encode()).hexdigest()
            == IMPORT_SHA256[path], "executable/proof import bindings changed; explicit runner review required")
    # Only proof imports may differ between builds. They do not supply an
    # executable branch. Actual code remains in the same source envelopes.
    import re
    text = re.sub(r"#\[cfg\(sail_verus\)\]\s*use\s+(?:vstd::|super::inputs::)[\sA-Za-z_0-9:*,{}]+;", "", text)
    if path == "crates/sail-acceptance/src/cache_counted.rs":
        proof_import = "#[cfg(sail_verus)]\nuse crate::cache_slots::{reticked_lookup, row_count};"
        require(text.count(proof_import) == 1, "counted cache proof import changed")
        text = text.replace(proof_import, "")
    values = [token.value for token in SOURCE.tokens(text)]
    forbidden = {"mod", "extern", "macro_rules", "cfg", "cfg_attr", "assume", "admit", "external_body",
        "external_fn_specification", "assume_specification", "external", "trusted", "axiom"}
    require(not (forbidden & set(values)), "compiled source outside capture subset: " + path)
    loading = {"include", "include_str", "include_bytes", "env", "option_env",
        "file", "line", "column", "module_path"}
    require(not any(value in loading and values[i + 1] == "!"
        for i, value in enumerate(values[:-1])), "environment-dependent macro source: " + path)
    # The untrusted sorter is deliberately a sibling of the private token
    # constructors. Every privileged function must be visible to verification.
    if path in PREFIX_HOST_TRAITS:
        checked_prefix_envelopes(path, text)
    elif path == WRITE_CORE:
        checked_persistence_envelopes(text)
    elif path.endswith("/acceptance_sort.rs"):
        SOURCE.candidate_forwarders(text)
    else:
        SOURCE.checked_item_envelopes(text, proof=path.startswith("proofs/refinement/"))


def checked_persistence_envelopes(text: str) -> None:
    """Admit only the reviewed host declarations and fully checked script impl.

    The complete original bytes still enter Stock. Removing these exact token
    spans here only lets the ordinary envelope guard inspect the remainder;
    no arbitrary trait, implementation or unchecked constructor is allowed.
    """
    stream = SOURCE.tokens(text)
    spans = []
    code_regions = [(i, SOURCE.closing(stream, i + 2)) for i in range(len(stream) - 2)
        if stream[i].value == "sail_code" and stream[i + 1].value == "!"]
    for i, token in enumerate(stream):
        if token.value == "sail_trait" and i + 2 < len(stream) and stream[i + 1].value == "!":
            end = SOURCE.closing(stream, i + 2)
            values = [t.value for t in stream[i + 3:end]]
            require(hashlib.sha256(repr(values).encode()).hexdigest() == WRITE_TRAIT_SHA256,
                "persistence low-level host contract changed; explicit review required")
            spans.append((token.begin, stream[end].end, "trait"))
        if token.value == "impl" and i + 1 < len(stream) and stream[i + 1].value == "PersistenceHost":
            require(any(begin < i < end for begin, end in code_regions),
                "persistence script implementation outside checked envelope")
            begin = next(j for j in range(i, len(stream)) if stream[j].value == "{")
            end = SOURCE.closing(stream, begin)
            values = [t.value for t in stream[i:end + 1]]
            require(hashlib.sha256(repr(values).encode()).hexdigest() == WRITE_SCRIPT_IMPL_SHA256,
                "persistence checked script implementation changed; explicit review required")
            spans.append((token.begin, stream[end].end, "script"))
    require(sorted(kind for _, _, kind in spans) == ["script", "trait"],
        "persistence host/script omitted or duplicated")
    for begin, end, _ in sorted(spans, reverse=True):
        text = text[:begin] + " " * (end - begin) + text[end:]
    SOURCE.checked_item_envelopes(text)


def checked_prefix_envelopes(path: str, text: str) -> None:
    """Admit only the reviewed host declarations and fully checked script impl.

    The complete original bytes still enter Stock. Removing these exact token
    spans here only lets the ordinary envelope guard inspect the remainder;
    no arbitrary trait, implementation or unchecked constructor is allowed.
    """
    stream = SOURCE.tokens(text)
    spans = []
    code_regions = [(i, SOURCE.closing(stream, i + 2)) for i in range(len(stream) - 2)
        if stream[i].value == "sail_code" and stream[i + 1].value == "!"]
    for i, token in enumerate(stream):
        if token.value == "sail_trait" and i + 2 < len(stream) and stream[i + 1].value == "!":
            end = SOURCE.closing(stream, i + 2)
            values = [t.value for t in stream[i + 3:end]]
            require(hashlib.sha256(repr(values).encode()).hexdigest() == PREFIX_HOST_TRAITS[path][1],
                "prefix low-level host contract changed; explicit review required")
            spans.append((token.begin, stream[end].end, "trait"))
        if token.value == "impl" and i + 1 < len(stream) and stream[i + 1].value == PREFIX_HOST_TRAITS[path][0]:
            require(any(begin < i < end for begin, end in code_regions),
                "prefix script implementation outside checked envelope")
            begin = next(j for j in range(i, len(stream)) if stream[j].value == "{")
            end = SOURCE.closing(stream, begin)
            values = [t.value for t in stream[i:end + 1]]
            require(hashlib.sha256(repr(values).encode()).hexdigest() == PREFIX_SCRIPT_IMPLS[path][1],
                "prefix checked script implementation changed; explicit review required")
            spans.append((token.begin, stream[end].end, "script"))
    require(sorted(kind for _, _, kind in spans) == ["script", "trait"],
        "prefix host/script omitted or duplicated")
    for begin, end, _ in sorted(spans, reverse=True):
        text = text[:begin] + " " * (end - begin) + text[end:]
    SOURCE.checked_item_envelopes(text)


def inventory(root: Path, manifest: dict, snapshots: dict[str, bytes]) -> list[dict]:
    """Validate every root, actual source closure and selected negative control."""
    require(manifest.get("schema") == "sail-production-refinements-v1", "unknown refinement manifest")
    entries = manifest.get("components", [])
    require(len(entries) == len(COMPONENTS)
        and {entry["id"] for entry in entries} == set(COMPONENTS), "refinement root omitted or duplicated")
    actual = {str(path.relative_to(root)) for path in (root / "proofs/refinement").rglob("*.rs")}
    require(actual == {"proofs/refinement/" + name + ".rs" for name in COMPONENTS},
        "unlisted or missing refinement source")
    require({str(path.relative_to(root)) for path in (root / "crates/sail-acceptance/src").rglob("*.rs")}
        == {"crates/sail-acceptance/src/" + name + ".rs"
            for name in ("lib", "syntax", "query", "inputs", "authority", "cache_bucket", "cache_candidate", "cache_chain", "cache_slots", "cache_counted", "cache_order", "cache_indexed", "parser_grammar", "query_split", "query_lex", "query_holding", "query_command", "query_policy", "host_syntax", "cache_record", "cache_write", "key_prefix", "query_raw", "query_prefix", "query_delivery")},
        "unlisted executable leaf source")
    for entry in entries:
        component = entry["id"]
        require(entry.get("source") == "proofs/refinement/" + component + ".rs", "wrong refinement source")
        require(entry.get("dependencies") == dependency_paths(component), "refinement dependency closure changed")
        if entry.get("discovery_pending"):
            require(component in {"parser", "command", "persistence", "prefix"} and entry.get("verified") is None and entry.get("functions") == [],
                "invalid pending named-root discovery")
        else:
            require(type(entry.get("verified")) is int and entry["verified"] > 0, "invalid verified count")
            require(bool(entry.get("functions")) and len(entry["functions"]) == len(set(entry["functions"])),
                "missing or duplicate full function inventory")
        require([control["name"] for control in entry.get("negative_controls", [])]
            == REQUIRED_CONTROLS[component], "selected mutation inventory omitted, duplicated or reordered")
        for path in entry["dependencies"]:
            if component == "authority" and path.endswith(("/output.rs", "/project.rs")):
                continue  # Only the mechanically selected declarations compile.
            compiled_subset(path, snapshots[path])
        for control in entry["negative_controls"]:
            path = control["source"]
            require(path in entry["dependencies"] and path != SYNTAX,
                "mutation does not target a checked dependency")
            before, after = control["before"].encode(), control["after"].encode()
            require(bool(before) and before != after and snapshots[path].count(before) == 1,
                "empty or ambiguous executable mutation")
            SOURCE.runtime_mutation(snapshots[path].decode(), control["before"], control["after"])
            compiled_subset(path, snapshots[path].replace(before, after))
    SOURCE.api_stencils(snapshots, manifest.get("api_declarations"))
    require(manifest.get("api_declarations") is not None, "missing API representation inventory")
    SOURCE.key_stencil(snapshots, manifest.get("key_declarations"))
    require(manifest.get("key_declarations") is not None, "missing key representation inventory")
    return entries


def captured_driver(entry: dict, snapshots: dict[str, bytes], control: dict | None = None) -> tuple[str, bytes, dict[str, bytes]]:
    """Use fixed module bindings and exact source bytes; never reopen a path."""
    component = entry["id"]
    require(entry["dependencies"] == dependency_paths(component), "unexpected dependency closure")
    sources = {path: snapshots[path] for path in entry["dependencies"]}
    if control is not None:
        path = control["source"]
        before, after = control["before"].encode(), control["after"].encode()
        require(path in sources and sources[path].count(before) == 1 and before != after, "invalid captured mutation")
        SOURCE.runtime_mutation(sources[path].decode(), control["before"], control["after"])
        sources[path] = sources[path].replace(before, after)
    files = {"syntax.rs": sources[SYNTAX]}
    declarations = ["#![forbid(unsafe_code)]", "#![allow(unused_imports)]",
        "use vstd::prelude::*;", "extern crate self as sail_acceptance;",
        '#[macro_use] #[path="syntax.rs"] mod syntax;']
    if component in {"persistence", "prefix"}:
        files["host_syntax.rs"] = sources[HOST_SYNTAX]
        declarations.append('#[macro_use] #[path="host_syntax.rs"] mod host_syntax;')
    for stem in COMPONENTS[component]["core"]:
        filename = "core_" + stem + ".rs"
        files[filename] = sources["crates/sail-acceptance/src/" + stem + ".rs"]
        declarations += ['#[path="' + filename + '"] pub mod ' + stem + ';',
            "pub use " + stem + " as " + stem + "_core;"]
    for stem in COMPONENTS[component]["models"]:
        filename = "model_" + stem + ".rs"
        files[filename] = sources["proofs/acceptance/" + stem + ".rs"]
        name = "authority_model" if stem == "authority" else stem
        declarations.append('#[path="' + filename + '"] pub mod ' + name + ';')
    if component == "authority":
        stencils, _ = SOURCE.api_stencils(sources)
        files.update(stencils)
        files["api_acceptance.rs"] = sources["crates/sail-api/src/acceptance.rs"]
        files["acceptance_sort.rs"] = sources["crates/sail-api/src/acceptance_sort.rs"]
        files["request_mode.rs"] = sources[MODE_CORE]
        declarations += ["pub mod output;", "pub mod project;",
            '#[path="api_acceptance.rs"] pub mod acceptance;', "mod acceptance_sort;",
            '#[path="request_mode.rs"] pub mod request_mode;']
    if component == "prefix":
        files["key_types.rs"], _ = SOURCE.key_stencil(sources)
        files["prefix_factory.rs"] = sources[KEY_FACTORY]
        declarations += ['#[path="key_types.rs"] pub mod key_types;',
            'pub use key_types::{Mode, SolverIdentity};',
            '#[path="prefix_factory.rs"] pub mod prefix_factory;']
    files["refinement.rs"] = sources[entry["source"]]
    declarations += ['#[path="refinement.rs"] pub mod refinement;', "fn main() {}"]
    for path, data in sources.items():
        if not (component == "authority" and path.endswith(("/output.rs", "/project.rs"))):
            compiled_subset(path, data)
    return "check_" + component + ".rs", ("\n".join(declarations) + "\n").encode(), files


def captured_tree(root: Path, commit: str, paths: list[str]) -> dict[str, bytes]:
    """Read canonical Git blobs in one batch, with no extraction to disk."""
    require(all("\n" not in path and "\r" not in path for path in paths), "unsupported Git path")
    retained = {"HOME", "PATH", "USER", "SYSTEMROOT", "TMPDIR", "LANG", "LC_ALL"}
    environment = {key: value for key, value in os.environ.items() if key in retained}
    environment.update(GIT_NO_REPLACE_OBJECTS="1", GIT_CONFIG_NOSYSTEM="1",
        GIT_CONFIG_GLOBAL=os.devnull, GIT_OPTIONAL_LOCKS="0")
    data = subprocess.check_output(["git", "--no-replace-objects", "-c", "safe.directory=" + str(root),
        "-c", "core.fsmonitor=false", "-C", str(root), "cat-file", "--batch"],
        input="".join(commit + ":" + path + "\n" for path in paths).encode(), env=environment)
    result: dict[str, bytes] = {}
    at = 0
    for path in paths:
        end = data.find(b"\n", at)
        require(end >= at, "missing Git object header")
        header = data[at:end].split()
        require(len(header) == 3 and header[1] == b"blob" and header[2].isdigit(), "unexpected Git object")
        length = int(header[2])
        begin = end + 1
        at = begin + length
        require(at < len(data) and data[at] == 10, "truncated Git object")
        require(path not in result, "duplicate Git path")
        result[path] = data[begin:at]
        at += 1
    require(at == len(data), "unexpected trailing Git output")
    require(set(result) == set(paths), "source capture omitted a path")
    return result


def working_source_matches(root: Path, snapshots: dict[str, bytes]) -> None:
    """Recheck both content and auto-discovery before and after verification."""
    require(not any((root / "crates" / name / "build.rs").exists()
        for name in ("sail-acceptance", "sail-api", "sail-smtlib", "sail-session", "sail-cache", "sail-key")), "unreviewed protected build script")
    expected = {path for path in snapshots if path.startswith("crates/")
        and ("/src/" in path and path.endswith(".rs") or path.endswith("/Cargo.toml"))}
    actual = {str(path.relative_to(root)) for crate in (root / "crates").iterdir() if crate.is_dir()
        for path in list((crate / "src").rglob("*.rs")) + list(crate.glob("Cargo.toml"))}
    require(actual == expected, "working source discovery changed")
    for path, content in snapshots.items():
        relative = Path(path)
        require(not any((root / parent).is_symlink() for parent in [relative, *relative.parents]),
            "symlink in captured source path")
        require((root / path).is_file() and (root / path).read_bytes() == content,
            "working source differs from Git capture")


def require_discovered(entries: list[dict]) -> None:
    """No pending root can authorize a normal acceptance run."""
    require(not any(entry.get("discovery_pending") for entry in entries),
        "complete Stock discovery pending; no qualification may be reported")


def check(root: Path, archive: Path, install: Path, out: Path, timeout: int) -> None:
    """Qualify every selected actual-body proof and executable guard mutation."""
    started = time.monotonic()
    require(not out.is_relative_to(root.resolve()), "put raw refinement evidence outside checkout")
    commit = COMMON.git(root, "rev-parse", "HEAD")
    all_paths = COMMON.git(root, "ls-tree", "-r", "--name-only", commit).splitlines()
    paths = [path for path in all_paths if (
        path.startswith("crates/") and ("/src/" in path and path.endswith(".rs") or path.endswith("/Cargo.toml"))
        or path.startswith("proofs/") and path.endswith((".rs", ".json"))
        or path in {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "corpus/toolchain.toml", "xtask/src/layering.rs",
            "scripts/check-acceptance-proofs.py", "scripts/check-acceptance-refinements.py",
            "scripts/acceptance-source.py"})]
    require(MANIFEST in paths, "refinement manifest is not committed")
    snapshots = captured_tree(root, commit, paths)
    working_source_matches(root, snapshots)
    validate_assembly(snapshots)
    require(not COMMON.git(root, "status", "--porcelain=v1", "--", *paths), "refinement source is uncommitted")
    manifest = json.loads(snapshots[MANIFEST], object_pairs_hook=COMMON.unique_object)
    entries = inventory(root, manifest, snapshots)
    require_discovered(entries)
    pins = tomllib.loads(snapshots["corpus/toolchain.toml"].decode())
    hosts = {("Darwin", "arm64"): "aarch64-apple-darwin", ("Linux", "x86_64"): "x86_64-unknown-linux-gnu"}
    host = hosts.get((platform.system(), platform.machine()))
    require(host is not None, "unsupported direct refinement platform")
    asset = pins["assets"][host]
    runtime = COMMON.validated_runtime(archive, install, asset)
    environment = COMMON.proof_environment(install, pins["verus"]["rust_toolchain"])
    rustc = subprocess.check_output(["rustup", "run", pins["verus"]["rust_toolchain"], "rustc", "-vV"], env=environment, text=True)
    require("commit-hash: " + pins["verus"]["rust_commit"] in rustc.splitlines(), "Rust compiler identity mismatch")
    out.mkdir(parents=True, exist_ok=False)
    results = []
    for entry in entries:
        for index, control in enumerate([None, *entry["negative_controls"]]):
            label = entry["id"] + "-" + str(index)
            name = "positive" if control is None else control["name"]
            driver, data, files = captured_driver(entry, snapshots, control)
            with tempfile.TemporaryDirectory(prefix="sail-stock-refinement-") as temporary:
                scratch = Path(temporary)
                for filename, content in files.items():
                    (scratch / filename).write_bytes(content)
                (scratch / driver).write_bytes(data)
                command = [str(install / "verus"), str(scratch / driver), "--cfg", "sail_verus",
                    "--no-cheating", "--output-json", "--num-threads", "1"]
                begin = time.monotonic()
                try:
                    result = subprocess.run(command, cwd=scratch, env=environment, text=True,
                        capture_output=True, timeout=timeout)
                except subprocess.TimeoutExpired as error:
                    for suffix, value in (("stdout", error.stdout), ("stderr", error.stderr)):
                        (out / (label + "." + suffix)).write_text(value.decode(errors="replace")
                            if isinstance(value, bytes) else value or "")
                    raise Refusal("refinement timeout in " + label) from error
            (out / (label + ".stdout")).write_text(result.stdout)
            (out / (label + ".stderr")).write_text(result.stderr)
            require(not COMMON.resource_failure(result.stdout + result.stderr), "refinement resource limit reached")
            document = COMMON.checked_document(result.stdout, result.returncode, entry, pins, broken=control is not None)
            results.append({"component": entry["id"], "control": name, "negative": control is not None,
                "seconds": time.monotonic() - begin, "exit": result.returncode,
                "driver_sha256": hashlib.sha256(data).hexdigest(),
                "compiled_source_sha256": {path: hashlib.sha256(value).hexdigest() for path, value in files.items()},
                "verification": document["verification-results"], "command": command})
            print(entry["id"] + ": " + name + " passed", flush=True)
    require(COMMON.git(root, "rev-parse", "HEAD") == commit, "checked commit changed")
    working_source_matches(root, snapshots)
    inventory(root, manifest, snapshots)
    require(COMMON.validated_runtime(archive, install, asset) == runtime, "stock runtime changed")
    report = {"commit": commit, "source_sha256": {path: hashlib.sha256(value).hexdigest()
        for path, value in snapshots.items()}, "stock_archive_sha256": asset["sha256"],
        "runtime_sha256": runtime, "rustc": rustc, "cases": results,
        "api_declarations": manifest["api_declarations"],
        "source_roles": {entry["id"]: source_roles(entry) for entry in entries},
        "bound_runtime_source_is_not_all_verified": True,
        "total_seconds": time.monotonic() - started,
        "scope": "checked executable bodies and component refinements; surrounding I/O, origin and service composition remain explicit obligations"}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    (out / "report.md").write_text("# Production acceptance refinements\n\nCommit `" + commit
        + "`. All " + str(len(results)) + " selected actual-body checks pass through direct stock Verus.\n\n"
        + report["scope"] + ". Runtime files bound by the receipt are not all verified bodies.\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stock-archive", type=Path, required=True)
    parser.add_argument("--stock-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    try:
        require(args.timeout > 0, "timeout must be positive")
        check(ROOT, args.stock_archive.resolve(), args.stock_dir.resolve(), args.out.resolve(), args.timeout)
    except (Refusal, SOURCE.SourceRefusal, OSError, ValueError, KeyError, TypeError,
            subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print("error[SAIL9002]: production refinement check refused: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

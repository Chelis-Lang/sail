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
RETAINED_BRIDGE = "crates/sail-daemon/src/jobs/store.rs"
KEY_NATIVE = "crates/sail-key/src/lib.rs"
KEY_FACTORY = "crates/sail-key/src/prefix_factory.rs"
KEY_CORE = "crates/sail-acceptance/src/key_prefix.rs"
WRITE_CORE = 'crates/sail-acceptance/src/cache_write.rs'
DELIVERY_CORE = 'crates/sail-acceptance/src/query_delivery.rs'
OUTPUT_CORE = 'crates/sail-acceptance/src/query_output.rs'
PUBLICATION_CORE = 'crates/sail-acceptance/src/query_publication.rs'
OUTPUT_OWNER_CORE = 'crates/sail-acceptance/src/query_output_owner.rs'
HOLDING_CORE = 'crates/sail-acceptance/src/query_holding.rs'
REPLAY_CORE = 'crates/sail-acceptance/src/query_replay.rs'
OPTIONS_CORE = 'crates/sail-acceptance/src/query_options.rs'
CHECKPOINT_CORE = 'crates/sail-acceptance/src/query_checkpoints.rs'
PIPE_CORE = 'crates/sail-acceptance/src/stream_capture.rs'
# Exact reviewed one-read event types and endpoint read primitive; no full-output oracle.
PIPE_GHOST_ITEMS = frozenset(['6fcc1d7016aa6a4e7715d36438bf650ee7ccb2408a8d21d4b98848cba606dc43'])
PIPE_HOST_TRAITS = frozenset(['78590e0391f7573656049a517dd0fb8c8adcd1587e82e6de2987b4648919ca28'])
# These exact native adapters are reviewed/bound, not compiled proof bodies.
QUERY_NATIVE_BINDINGS = {'crates/sail-cache/src/lib.rs': ['DiskCache::put_scoped'], 'crates/sail-cache/src/persist.rs': ['NativeWriteHost', 'write_record'], 'crates/sail-shim/src/cached.rs': ['SolverPipe', 'Solving', 'start_solver', 'Pairer', 'output_side', 'Storing', 'spawn_storer', 'SolverOwnerIdentity', 'SolverProcess', 'TerminalInput', 'StoreCommand', 'Entry', 'HeldMetadata', 'held_entry', 'batch_observed', 'InputStatus', 'queue_stores', 'observe', 'ObservedSession', 'option_spans', 'statistics_spans', 'Served', 'ServedReply native alias', 'ReplayCheck', 'served_storage_bytes'], 'crates/sail-session/src/lib.rs': ['Session::new', 'Session::option', 'Session::expects_output', 'Session::apply', 'Session::apply_inner'], 'crates/sail-smtlib/src/lib.rs': ['native module and error assembly'], 'crates/sail-smtlib/src/command.rs': ['classify', 'value_span', 'CommandKind::output'], 'crates/sail-smtlib/src/lex.rs': ['Lexer', 'Token', 'is_delimiter'], 'crates/sail-smtlib/src/split.rs': ['CommandSplitter', 'ResponseSplitter', 'is_single_response'], 'crates/sail-smtlib/src/error.rs': ['Error', 'ErrorCode'], 'crates/sail-session/Cargo.toml': ['native Session dependency assembly'], 'crates/sail-smtlib/Cargo.toml': ['native SMT helper dependency assembly']}
QUERY_NATIVE_SHA256 = {'crates/sail-cache/src/lib.rs': '106f1bd0fff663e6302be566de7b9e9c789e1e8433d354192e20f13db14b09dc', 'crates/sail-cache/src/persist.rs': '956ec4c117fb2b722a9a2454a9f1a60adf76e58a5a61397b1f144d4894d23260', 'crates/sail-shim/src/cached.rs': '4d90c3f5712c6f189d50747379aa179f1712b046e9ad33ffdb20ebb0996cbb9e', 'crates/sail-session/src/lib.rs': 'fd830d2d2c57a809104e1d0a7c28f92859e96717325a4eb62fea3d2c66a6d5b1', 'crates/sail-smtlib/src/lib.rs': '067e315883f7fc17a4609cab43e15ff83d2efa10b7d52164df366779a136e462', 'crates/sail-smtlib/src/command.rs': 'c0a25df1643c702bdf641148f427a693dbcefb3392792f04752375dc9cbede2f', 'crates/sail-smtlib/src/lex.rs': '703af8b5867b1c17906af7ab9018b8a69c4342ebe7fa01725eac342c68d0122d', 'crates/sail-smtlib/src/split.rs': 'fb5a7973f51ad628d9651a4568cd629cb26b833c8a50041c8ca5edc60e209a44', 'crates/sail-smtlib/src/error.rs': '7403bf10fcc8dc7e4e199dcd1deda767717423ccbec42235b648c9005f5892e2', 'crates/sail-session/Cargo.toml': '4d1c12f98e87f7e40cf46febe3e667d9d5576e88b5e28ba5d8856bdc18e6ba65', 'crates/sail-smtlib/Cargo.toml': '383de243765b26d92dcf01ea604911e9b254ce9014a9da5deb8ab3872be19587'}
QUERY_HOST_TRAITS = {'crates/sail-acceptance/src/cache_write.rs': ['e1f217be87981cbd3d19140f85d8c7e8434568bd092cee9555efab9da58cf851'], 'crates/sail-acceptance/src/query_delivery.rs': ['127030cacff4e231147ffc843933effa0c754b3ad1963e460c5e981bcde92220']}
QUERY_SCRIPT_IMPLS = {'crates/sail-acceptance/src/cache_write.rs': ['b14cfdbc045ce226b03452823bf1706890c04c58817293134e9d1f2fc1ce12b4'], 'crates/sail-acceptance/src/query_delivery.rs': ['8378afe345bf6880d519cf675e3b5db5e4b01f6900781bef674f6c1fd7019d72']}
# Presentation callers are bound native source; only their shared API helper compiles.
PRESENTATION_NATIVE_BINDINGS = {'crates/sail-api/src/evidence.rs': ['effective'], 'crates/sail-mcp/src/summary.rs': ['effective_verdict', 'effective_evidence_tier'], 'crates/sail-mcp/src/lib.rs': ['native summary module assembly'], 'crates/sail-mcp/Cargo.toml': ['native MCP API dependency assembly'], 'crates/sail-daemon/src/jobs/product.rs': ['CompletedObservation::presentation', 'completed_with_request', 'explain', 'compare', 'publish_baseline', 'baseline_with_request'], 'crates/sail-cli/src/managed_cli.rs': ['verify human-only byte consumer before ProjectResult execution']}
PRESENTATION_NATIVE_SHA256 = {'crates/sail-api/src/evidence.rs': '0f6234f0bf8339986e5f29aa0a5c198908252cb85af10b8c9714769aafa0fab9', 'crates/sail-mcp/src/summary.rs': 'bd23b43209cbc51ac2108716bbd3d2d45643c7dda817140e0b60bd574f07f772', 'crates/sail-mcp/src/lib.rs': '7d11b1436211816863d21905ab030a4840b1795ad92f64b973033ec19807e217', 'crates/sail-mcp/Cargo.toml': '2a42a3c1ec74362a4a62233ffe88eab6952c90882a1e9695744fa6a1d3b650c0', 'crates/sail-daemon/src/jobs/product.rs': 'ee923438cae29230e0eef4774655d96c98177abba3d4efa26d58a71b5a986861', 'crates/sail-cli/src/managed_cli.rs': 'a1cdf6f81e9ace5c42a4b4d3381bf3d5f7817d691f962d3487e6188dc30c642f'}
# These closed native compiler/request/store adapters are bound in full and
# remain outside the stock-compiled bodies. Their OS/process provenance remains open.
INPUT_NATIVE_SHA256 = {'crates/sail-verus/src/adapter.rs': 'd22daa5169530ecacbcbd991840f404adaf24ceda15975ea8c2123a8d1ee3903',
 'crates/sail-verus/src/adapter_attempts.rs': 'f8713751833e4de1bd17a3b97ac2e4b801c1c8137281eb2939d3b6849f528777',
 'crates/sail-verus/src/engine.rs': 'a6ca795294e29f76b8f7647713da1ef1d8b79e2f1df7281f1fc7fe5869c1f0d2',
 'crates/sail-verus/src/managed/mod.rs': '7d1a4092e364b466a4da4247fb91418af8930fbd8316e83b8641d8f1b8b9cff9',
 'crates/sail-verus/src/managed/command_environment.rs': '43b34bd36c7c6c2f22a9bc34a39bd77737e65a3c167855143985cbddbe87508f',
 'crates/sail-verus/src/managed/runner.rs': '7ccafef55d2f17e5777648e5e46e021d06ac2f6c2eab1d0b300491a86578f080',
 'crates/sail-verus/src/managed/request.rs': 'dc079a01a60d64ee482540f5023deba9025cbc45010b6b2d1b56f2f7519bbaba',
 'crates/sail-verus/src/managed/noop.rs': 'bf41abde757dcbc9ecb7e1575a0df581faad2f9036b3bf99ff13e67a9cac614f',
 'crates/sail-verus/src/managed/shipped.rs': '4785c4450b09daeb049574458adf3dda63f79455ec7a0bff113e86aa4e47709a',
 'crates/sail-verus/src/managed/shipped/compiler.rs': '3006fbb0015519b56a12c8ec30bc2da0f4d47593d5b603cd602e0d8a3e7900ca',
 'crates/sail-verus/src/managed/shipped/native.rs': '7acc4ef691a8128ff22b0a1dc3e94af33b6e0037b550681ad35229323f3b5a0f',
 'crates/sail-verus/src/managed/shipped/native_reads.rs': '3fa562e94e38cb8f4489e072c60ea3cd7776ede5352ada907a29adaf63f5f580',
 'crates/sail-verus/src/managed/retained/driver.rs': 'bf6fcfe336d0ff9f0594263cff0a34613faa8ef6f271723ee7575859247540e0',
 'crates/sail-verus/src/managed/retained/compiler.rs': '6f75bff2863f40c58b7b9d331bdcb10852993808a01984f31cc94757f151fe2f',
 'crates/sail-verus/src/managed/retained/native.rs': '7b4bf88dc3fabc99497de450d6f99a0f6d39b6da60f653f63deaade7910992fa',
 'crates/sail-verus/src/managed/retained/producers.rs': '501246e0ccd8ec5bd5025fb985cb104adf8c40a540d4b93b03b4b645ac18ba4c',
 'crates/sail-daemon/src/jobs/product.rs': 'ee923438cae29230e0eef4774655d96c98177abba3d4efa26d58a71b5a986861',
 'crates/sail-daemon/src/jobs/result_cache.rs': '7d63c09a1e623d2204e64f593adc32e1527c04786ca57439aab1e64600680b64',
 'crates/sail-verus/src/managed/command_pipe.rs': 'aaccb4db7cce2648b4a1acdc5da658ea2b53abbcc24956310deac5c7b6e6eb31',
 'crates/sail-api/src/lib.rs': 'c0d86d594bb374650fae43024f49264a4c6f840c8b0f73ab8092d620afdc7fed',
 'crates/sail-api/src/stamp.rs': 'fafb1eea946f08ea4a1e534a0b3c634094c862f63eb8b9251b9e76c498d0dda8',
 'crates/sail-verus/src/adapter_verus_channel.rs': '2229526e8081f0d7309a30c4f3acdf04062d19363b6c1152aa98aab44182d421',
 'crates/sail-verus/src/managed/build_environment.rs': 'de6270ac49a38906d99e076ffede26ed9d562e0d4022226254622b4f2d6c34d3',
 'crates/sail-verus/src/managed/build_inputs.rs': '75c53c32c2775d52eb652cbd52b8518e1956779e0623302c2970b8b56e33a00c',
 'crates/sail-verus/src/managed/build_inputs_family.rs': 'ad22f4e115f9644243c429462cae1f2563e11fb338c50944116ca42acdda33e3',
 'crates/sail-verus/src/managed/build_inputs_sources.rs': '04dc4ef8b47f360d486dcb5524c1288ec036f5936b5a74866b72dc4306058411',
 'crates/sail-verus/src/managed/build_inputs_package_environment.rs': '9eb5e5550b8ea76287cd0d8c717c9a0b494db3b0ae8196dd908429704b8adc6b',
 'crates/sail-verus/src/managed/build_unit_plan.rs': '3e76d01766dcc3d92d3af6538cacf834bce210f2d1a9a6a2273c31b764dd874d',
 'crates/sail-verus/src/managed/dependency_snapshot.rs': '5f9af1aaf0cfc8eeaf2611ee84d878e5a21e95a5dd05b1377bb9f40cb924c2b2',
 'crates/sail-verus/src/managed/environment.rs': '41e37d497023ccd874af84e95318f80c83e3a2e4f6c6eb613ac02d93fb0ddb6f',
 'crates/sail-verus/src/managed/files.rs': '6fe922e0141ce156f91d445e97c4d5653033aeedf69c7a0bde17c36ba8395445',
 'crates/sail-verus/src/managed/ghost_environment.rs': '7ad1fe857be95fe2aeb1e5b83677d72b7b3508269858f5e05e6db0446f2e2f0e',
 'crates/sail-verus/src/managed/input.rs': '5899dfb6f7e5663c0e9e7325ea29abf49bb657cdd155c59048348f636f91c292',
 'crates/sail-verus/src/managed/noop_origins.rs': '56763925ca9b7c7ef8621ba1671f48097734161d87a2bdc0b8747c03b1694dd4',
 'crates/sail-verus/src/managed/read_check.rs': 'f187434c72bbb4abdbf58466ffd7a10648f7e29be8729eac7679954d84f7d7ad',
 'crates/sail-verus/src/managed/receipts.rs': 'b8d499a531b9dd5d17ad26fabdf08e5191cc7123e803a153f03b1f755339e3c6',
 'crates/sail-verus/src/managed/retained/archive.rs': '1fa79d3167f4b496176745ac884fced1863037dac7c16fbd2cedd410aa092ee5',
 'crates/sail-verus/src/managed/retained/history.rs': '40c15b7834f07a242ef300674cad6c27d0126746db2bb98a25ede0b8c665040e',
 'crates/sail-verus/src/managed/retained/mod.rs': 'da72364cd1759a0bd577fae9803fc34a0de323dad1ab9c5943a4e6e54328fc4f',
 'crates/sail-verus/src/managed/retained/phase_fields.rs': '594552f03e3f3a7742b5ad75964b1f90f830325c1d06abdf18b178d09c503664',
 'crates/sail-verus/src/managed/source_inputs.rs': '15ce88b850f577b14372eb8586000e4c15e4ec8958a65dbef02e56d8e73d5395',
 'crates/sail-verus/src/managed/store.rs': '3cf8473ed05acb46c8b5bcbe7a23b18886188087e52dd5cea1abe5e225112c9e',
 'crates/sail-verus/src/managed/tool_digests.rs': 'c53d675ab062eafb6ba29b97e753d8630d5aef69c663cf91eae2016dc3565329',
 'crates/sail-verus/src/managed/transport.rs': '8d602d2b84a9b517a1d281b0ed3fd4f586cb68c1c45692970721b103c6d0c416',
 'crates/sail-verus/src/managed/visible_output.rs': '67dd94f1db4336c6aff782d92572cd4360dad3ff08c55f2274e443d33ace1bcb',
 'crates/sail-verus/src/parallel.rs': 'c2f4ab8e7268ca28fe106255ed13cdba0838004283cc1f7b47b3f4e6a9d9a483',
 'crates/sail-verus/src/lib.rs': '812c8b05e10aeb73f7cda4a7289d9d23d40ea3d0639d1bad1d95e612d1e0291b',
 'crates/sail-verus/Cargo.toml': '7433b377b5a8a1cc4c9717ca90936ff39d81007b57de42eb2220a37b27d76cee'}
# Native key types, SHA adapter, fresh-state factory and module/import bindings
# are reviewed in their complete enclosing source; they are not all proved.
KEY_NATIVE_SHA256 = 'b363a7cd49229556a8e876af2da2bb7362ba9c4c2874f34bc092c4b59bf6e630'
KEY_HOST_TRAITS = frozenset({"07062a121a098f5dee4b51714ff0b575b1e6568347408344bc4aea5f3fe8be9a"})
KEY_RECORDING_IMPLS = frozenset({"de6e87c96d820b1610f24bc6b31aa3a64ba0e8d329446d476e08fcedcdcf5c3e"})
# The native classify/apply_currentness/withhold_unbound_authority bridges have
# been reviewed to forward the actual project and readback to the shared code.
# Their enclosing native binding environment is pinned in full; the module's
# I/O, serialization, cache selection and other service operations are not proved.
RETAINED_BRIDGE_SHA256 = 'f1214b5c3cada2004a55eb359e8ec62724a8fa5349ec50ca7d3b9e6ce09f38f5'
# This reviewed source uses the identical argument/type/body metavariables in
# Cargo and stock profiles. Changes require an explicit runner review, beyond
# merely updating a manifest. Runtime-only implementation drift must refuse.
SYNTAX_SHA256 = 'deb8606ce9d123997dd286ae3e184c7496bdaf5e659543a7be724baffd9adc70'
LIB_SHA256 = '6bd34ec1405ac5e0cead7f66bbf735c095967f242accfd8c019ecfd8f459481a'
API_LIB_SHA256 = 'c0d86d594bb374650fae43024f49264a4c6f840c8b0f73ab8092d620afdc7fed'
# Binding inventories cover both ordinary and proof-only imports. In particular,
# an erased import cannot shadow a Cargo glob with a different checked callee.
IMPORT_SHA256 = {'crates/sail-acceptance/src/query.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/inputs.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/authority.rs': 'd0adfcd0a1e64a0addc3db112f9f9fc707f3f4c68bd7b907047a00b8d7f4abb4', 'crates/sail-api/src/acceptance.rs': 'b73f6472c2f8c2713ed14b6af483f37a2ff6faedb4f35758837e568a623c002c', 'crates/sail-acceptance/src/key_prefix.rs': 'abcb768a7dca61a2e4248956921314487e845306a6335e944610f3c5bb90e5b8', 'crates/sail-key/src/prefix_factory.rs': '22f9a04f035b109591f920519d7423c7de774bfc66315194b3939f291bf62988', 'crates/sail-acceptance/src/query_policy.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_prefix.rs': '6c28aff345658a06686d88714020c77540412e4b5403912081c62141c82059b0', 'crates/sail-acceptance/src/phase_inputs.rs': 'e31c1fd6ae8c583a1fb26630289887e8aa3b0eaae302c997a571085167c83532', 'crates/sail-acceptance/src/cache_write.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_delivery.rs': 'dd5692fc8c75546d449a2c623708ec6748a7be654391385107f4fa20e971d08e', 'crates/sail-acceptance/src/query_output.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_publication.rs': '5b42b5e8b9a5cdc3fc2c046bb501eb353d630a73b668b7a2e6a8ae857cbb8fab', 'crates/sail-acceptance/src/stream_capture.rs': '18c75f1316fdcfe0ddaae6f01a1a4a261ee5a0e48f1aec2d4a0d5649f2e5e9da', 'crates/sail-acceptance/src/query_output_owner.rs': 'a39dc18af6d81e5a8685b606379b6421026478c1544bd33da36231fb10f62a7f', 'crates/sail-acceptance/src/query_holding.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_checkpoints.rs': '3a451c460b5bb877272130b7081c5f715678a4577e4d117e9d516c8558ad935c', 'crates/sail-acceptance/src/query_options.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_replay.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d'}
# Exact reviewed ghost declarations and abstract primitive contracts. These
# permit no arbitrary erased types or host implementations. The native host
# implementation is a separate obligation, never discharged by a trait bound.
INPUT_GHOST_ITEMS = frozenset({'34a7aa7c26994229805fe92e7e544f340920dbd3416ea1dcff4e4c8f7ea82697'})
INPUT_HOST_TRAITS = frozenset({'d5f071d9dcd1b4ec056b39af49e32cad198b1ebe2a4f692ce837364c83ca0691', 'ec46570421f11527e24f55ff2737c1f9165486cfb02a197809c4d7b8b1aaab71'})
REQUIRED_CONTROLS = {'query': ['fifo_wrong_stack', 'partial_pair_consumes_head', 'replay_off_by_one', 'noisy_publication', 'corrupt_recovery', 'ungated_batch_release', 'incomplete_permutation', 'raw-ignore-command-bytes', 'raw-ignore-gap', 'raw-skip-separators', 'record-ignore-header-newline', 'record-change-separator', 'record-keep-separator-in-reply', 'frame-premature-root-close', 'frame-ignore-doubled-quote', 'frame-ignore-input-comment', 'head-ignore-framing', 'response-ignore-line-terminator', 'response-ignore-list-framing', 'key-solver-platform', 'key-mode-domain', 'key-raw-bytes', 'key-end-domain', 'key-counter-refusal', 'key-feed-order', 'key-silence-domain', 'key-state-copy', 'key-length-word', 'key-typed-platform', 'key-typed-optin', 'head-complete-leading-trivia', 'head-complete-untruncated', 'policy-tactic-head', 'policy-get-prefix', 'policy-whole-head', 'owner-policy-head', 'owner-original-append', 'owner-command-end', 'owner-hash-tail', 'write-omitted-full-write', 'write-omitted-rename-call', 'write-premature-metadata', 'write-cleanup-unowned-name', 'write-substitute-original-error', 'write-omit-owned-cleanup', 'delivery-omit-actual-pipe-write', 'delivery-hash-different-write-bytes', 'delivery-heal-failed-delivery', 'delivery-omit-terminal-key-binding', 'delivery-accept-closed-pipe', 'delivery-omit-owned-pipe-close', 'delivery-seed-drops-mode', 'delivery-seal-without-freezing', 'delivery-seal-unmatched-prefix', 'delivery-write-after-seal', 'delivery-token-different-prefix', 'delivery-token-different-domain', 'delivery-parser-drops-mode', 'output-exact-appended-bytes', 'output-contiguous-start', 'output-bounded-end', 'output-response-grammar', 'output-recorded-endpoint', 'output-exact-frame-bytes', 'output-eof-required', 'output-full-coverage', 'output-healthy-origin', 'output-no-healing-after-eof', 'output-repeat-eof-refuses', 'output-actual-eof-marker', 'output-terminal-original-buffer', 'output-streaming-refuses', 'output-frame-always-refuse', 'output-seal-always-refuse', 'publication-observation-full-factory-always-refuses', 'publication-observation-skips-existing-closure-gate', 'observed-release-full-operation-always-refuses', 'observed-release-without-typed-observation', 'frame-copy-drops-original-bytes', 'frame-copy-replaces-original-ordinal', 'owned-frame-full-copy-always-refuses', 'owned-frame-copy-ignores-original-lease', 'owned-controller-frame-always-refuses', 'completed-stream-membership-ignores-owner', 'owned-publication-full-factory-always-refuses', 'owned-publication-ignores-common-child-lease', 'publication-frame-membership-ignores-owner', 'owned-publication-release-always-refuses', 'batch-borrow-omits-original-entries', 'held-replace-drops-displaced-original', 'held-take-silently-discards-original', 'held-metadata-detach-discards-original', 'held-orphan-metadata-creates-annotation', 'held-release-before-reorders-original-current', 'held-replace-returns-wrong-original', 'checkpoint-whole-validator-always-refuses', 'checkpoint-missing-permutation-check', 'checkpoint-ignores-original-command-bytes', 'checkpoint-ignores-original-prefix-key', 'checkpoint-hashes-command-without-original-leading-input', 'frozen-checkpoint-substitutes-head-for-command', 'frozen-checkpoint-substitutes-original-start', 'captured-writer-always-withholds-nonempty-tape', 'captured-writer-retains-wrong-acknowledged-tape', 'delivery-seal-drops-original-tape', 'full-original-binding-validator-always-refuses', 'original-binding-skips-acknowledged-prefix-validation', 'original-binding-drops-complete-fresh-domain', 'original-binding-accepts-swapped-original-ordinals', 'original-binding-accepts-other-output-owner', 'original-binding-substitutes-zero-replay-prefix', 'owned-output-replaces-original-completion-cursor', 'owned-output-never-advances-original-cursor', 'replay-completion-always-refuses', 'replay-completion-mints-before-drained', 'option-head-original-bytes', 'option-keyword-whole-atom', 'option-keyword-complete-boundary', 'option-keyword-colon', 'option-keyword-leading-trivia', 'option-value-leading-trivia', 'option-value-whole-atom', 'option-value-trailing-data', 'option-keyword-value-swap', 'option-keyword-omission', 'option-scalar-value-omission', 'option-post-readback-mismatch', 'option-use-print-readback', 'option-use-limit-readback', 'option-refusal-sticky', 'option-reset-clears-both', 'option-ordinary-retains-state', 'option-actual-byte-comparison', 'option-state-full-transition-always-refuses', 'replay-statistics-original-head', 'replay-statistics-original-keyword', 'replay-statistics-trailing-arguments', 'replay-statistics-complete-command', 'replay-statistics-keyword-gap', 'replay-statistics-forged-exception', 'replay-original-command-retention', 'replay-original-cache-bytes-retention', 'replay-statistics-full-framing', 'replay-exact-original-bytes', 'replay-original-served-order', 'replay-exact-one-comparison', 'replay-sticky-divergence', 'replay-clean-short-prefix', 'replay-refusal-latched', 'replay-original-remaining-count', 'replay-full-comparison-always-refuses', 'replay-full-statistics-mint-always-refuses', 'replay-join-requires-comparisons', 'replay-join-original-output-owner', 'replay-join-original-cursor-count', 'replay-full-output-join-always-refuses', 'replay-prefix-retained-reproduction', 'replay-prefix-original-output-ordinal', 'replay-prefix-original-served-ordinal', 'replay-prefix-original-frame-bytes', 'replay-prefix-reproduction-cannot-ignore-bytes'], 'inputs': ['input-ignore-present-value', 'input-ignore-current-stamp', 'input-ignore-required-read-presence', 'input-ignore-inverse-permutation', 'input-provider-wrong-frame-child', 'input-provider-terminal-count', 'input-provider-required-read-frame', 'input-provider-required-read-presence', 'input-opened-object-binding', 'input-read-final-handle', 'input-read-final-path', 'input-read-chunk-bytes', 'input-read-eof-required', 'input-read-cache-object', 'input-read-requested-binding', 'input-environment-byte-name', 'input-environment-escaped-name', 'input-kernel-hash-bytes', 'input-kernel-final-handle-observation', 'input-kernel-final-path-observation', 'input-kernel-open-observation-order', 'input-kernel-buffer-required', 'input-kernel-read-failure', 'input-kernel-always-error', 'input-guarded-readback-change', 'input-guarded-readback-availability', 'input-guarded-readback-always-unknown', 'phase-run', 'phase-unit', 'phase-slot', 'phase-unit-manifest', 'phase-pid', 'phase-binding', 'phase-complete', 'phase-normal-wait', 'phase-count', 'phase-known-pair', 'phase-verification-fields', 'phase-compilation-fields', 'phase-no-foreign-fields', 'phase-always-refuse', 'phase-final-descriptor', 'phase-program', 'phase-argument-vector', 'phase-directory', 'phase-overrides', 'phase-override-value', 'row-checksum-role', 'row-size-key', 'row-tracked-value', 'row-env-absent', 'row-env-present', 'row-drop-final', 'row-fact-drop-final', 'row-fact-role', 'row-always-refuse', 'row-size-high-byte', 'filename-control-byte-refusal', 'filename-escape-prefix', 'filename-escape-value', 'filename-literal-byte', 'filename-cardinality', 'filename-constructive', 'environment-parent-rows', 'environment-parent-key-coverage', 'environment-override-coverage', 'environment-no-extra-or-stale-values', 'environment-clear-policy', 'environment-inherit-policy', 'environment-override-value', 'environment-inherited-value', 'environment-raw-name-key', 'environment-unreadable-refusal', 'environment-constructive', 'command-environment-actual-readback-extra', 'command-environment-actual-readback-omitted', 'command-environment-inheritance-retention', 'command-environment-original-builder-getter', 'command-environment-constructive', 'owned-config-env-cardinality', 'owned-config-env-row', 'owned-config-original-command', 'owned-config-map-binding', 'owned-config-constructive', 'owned-start-descriptor', 'owned-start-pid', 'owned-start-constructive', 'owned-completion-owner', 'owned-completion-pid', 'owned-completion-descriptor', 'owned-completion-wait', 'owned-completion-policy', 'owned-completion-output', 'owned-completion-status', 'owned-completion-constructive', 'pipe-read-original-owner', 'pipe-read-original-role', 'pipe-read-after-eof', 'pipe-read-zero-capacity-is-not-eof', 'pipe-read-fatal-failure-sticky', 'pipe-read-retryable-error-constructive', 'pipe-read-exact-prefix', 'pipe-read-exact-prefix-values', 'pipe-seal-requires-real-eof', 'pipe-seal-requires-no-failure', 'pipe-token-original-owner', 'pipe-token-original-role', 'pipe-token-unavailable-never-authority', 'pipe-capture-invalid-role', 'pipe-completion-stdout-origin', 'pipe-completion-stderr-origin', 'pipe-completion-refuses-missing-origin', 'pipe-completion-preserves-output-roles', 'pipe-seal-full-factory-always-refuses', 'pipe-completion-full-factory-always-refuses'], 'authority': ['verification-enabled', 'effective-verification-enabled', 'whole-terminal-summary', 'confirmed-function', 'whole-command', 'requested-coverage', 'summary-count', 'normal-exit', 'fresh-selection', 'obligation-uniqueness', 'provenance-counts', 'request-original-context', 'request-independent-coverage', 'request-verification-enabled', 'request-current', 'request-repair-function', 'request-positive', 'request-repair-presence', 'submission-arguments', 'submission-mode', 'submission-whole-authority', 'submission-positive', 'driver-token-bytes', 'driver-token-coverage', 'driver-sources-order', 'process-wait-required', 'process-distinct-slot', 'process-duplicate-stream', 'process-stderr-binding', 'process-recovery-bytes', 'process-payload-binding', 'process-refusal-recorded', 'retained-original-current', 'retained-complete-readback', 'retained-exact-revision', 'retained-view-currentness', 'retained-request-admission', 'retained-clear-refused-labels', 'retained-preserve-positive', 'target-exact-cargo-name', 'target-rust-name-normalization', 'target-kind-binding', 'process-configure-slot', 'decline-actual-zero', 'engine-complete-protocol', 'decline-refusal', 'repair-decline', 'origin-slot', 'origin-pid', 'origin-descriptor', 'feature-raw-origin', 'feature-available', 'feature-name', 'feature-normal-zero', 'feature-decision', 'feature-model-admission', 'replay-guard-content', 'replay-guard-availability', 'replay-guard-revision', 'replay-request-context', 'replay-request-invocation', 'replay-request-arguments', 'replay-request-targets', 'replay-transfer-readback', 'replay-transfer-original-authority', 'replay-transfer-report-preservation', 'replay-transfer-always-refuses', 'replay-readback-always-refuses', 'scope-require-focused', 'scope-exact-arguments', 'scope-plain-report', 'scope-successful-report', 'scope-always-refuse', 'whole-replay-full-binder-always-refuses', 'partial-target-full-factory-always-refuses', 'partial-request-full-factory-always-refuses', 'partial-transfer-full-binder-always-refuses', 'partial-transfer-full-binder-always-accepts', 'partial-report-raw-scope-check-removed', 'partial-report-engine-check-removed', 'partial-report-record-check-removed', 'partial-target-whole-label-checks-removed', 'partial-target-establishing-identity-check-removed', 'partial-target-recorded-scope-binding-removed', 'partial-target-raw-scope-check-removed', 'partial-request-coverage-check-removed', 'partial-request-target-report-check-removed', 'partial-transfer-independent-request-check-removed', 'partial-transfer-original-authority-check-removed', 'partial-transfer-actual-readback-check-removed', 'partial-transfer-report-inventory-cleared', 'partial-transfer-original-establishment-reset', 'retained-submission-full-factory-always-refuses', 'retained-submission-full-factory-skips-live-invocation', 'request-path-binding-full-factory-always-refuses', 'request-path-binding-full-factory-skips-raw-comparison', 'request-path-withholding-full-operation-always-accepts', 'request-path-withholding-full-operation-always-refuses', 'request-path-withholding-keeps-partial-scope', 'request-path-withholding-keeps-whole-completion', 'full-admission-always-refuses', 'full-admission-constructs-unchecked-witnesses', 'admission-binds-native-paths-to-the-wrong-request', 'admission-skips-both-original-report-binders', 'admission-uses-whole-binder-for-partial', 'admission-uses-partial-binder-for-whole', 'presentation-restores-original-run-identity', 'presentation-provisional-override', 'presentation-confirmed-verified', 'presentation-confirmed-failed', 'presentation-confirmed-unknown', 'presentation-confirmed-provisional', 'presentation-report-original-establishment', 'presentation-report-original-verdict', 'presentation-full-fields-always-provisional', 'presentation-full-report-always-provisional', 'completed-view-original-request', 'completed-view-no-unchecked-whole', 'completed-view-request-projection', 'completed-view-whole-projection-always-withholds', 'completed-view-full-factory-always-withholds-whole', 'empty-human-candidate-full-operation-always-refuses', 'empty-human-footer-full-operation-always-refuses', 'empty-human-footer-allows-nonempty-stdout', 'empty-human-duration-ignores-complete-suffix', 'empty-human-candidate-ignores-json-consumer', 'empty-human-candidate-ignores-original-header', 'empty-human-candidate-allows-observed-reports', 'empty-human-candidate-allows-compile-only', 'empty-human-candidate-allows-focused-command', 'empty-human-candidate-allows-real-verifier-invocations', 'empty-human-factory-full-operation-always-refuses', 'empty-human-factory-ignores-current-request', 'empty-human-factory-ignores-original-stdout', 'empty-human-factory-constructs-unchecked-native-paths', 'empty-human-factory-constructs-unchecked-readback']}
COMPONENTS = {
    "query": {"models": ["context", "query_association"], "core": ["inputs", "query", "key_prefix", "query_policy", "query_prefix", "cache_write", "query_delivery", "query_output", "query_publication", "query_output_owner", "query_holding", "query_checkpoints", "query_options", "query_replay"]},
    "inputs": {"models": ["context"], "core": ["inputs", "authority", "stream_capture", "phase_inputs"]},
    "authority": {"models": ["context", "authority"], "core": ["inputs", "authority", "stream_capture", "phase_inputs"]},
}


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
    protected_packages = ("sail-acceptance", "sail-api", "sail-key", "sail-cache", "sail-shim", "sail-verus", "sail-daemon")
    # The temporal extension is a separate workspace (ADR 0083); no protected
    # workspace dependency may redirect or select an alternate feature assembly.
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
    key = tomllib.loads(snapshots["crates/sail-key/Cargo.toml"].decode())
    require(set(key) == {"package", "dependencies", "dev-dependencies", "lints"}
        and set(key["package"]) == set(package) and key["package"]["name"] == "sail-key"
        and all(key["package"][field] == {"workspace": True} for field in
            ("version", "edition", "rust-version", "publish", "license", "repository"))
        and key["dependencies"] == {name: {"workspace": True}
            for name in ("sail-acceptance", "sail-session", "sha2")}
        and key["dev-dependencies"] == {name: {"workspace": True}
            for name in ("proptest", "sail-smtlib")}
        and key["lints"] == {"workspace": True}, "key target/dependency/build assembly changed")
    require(hashlib.sha256(snapshots[KEY_NATIVE]).hexdigest() == KEY_NATIVE_SHA256,
        "native key declarations/primitives/callers changed; explicit review required")
    for name in ("sail-api", "sail-key", "sail-cache", "sail-shim", "sail-verus", "sail-daemon"):
        manifest = tomllib.loads(snapshots["crates/" + name + "/Cargo.toml"].decode())
        require(manifest["dependencies"]["sail-acceptance"] == {"workspace": True}, "runtime dependency redirected")
    protected = {
        "crates/sail-acceptance/src/query.rs": ("Fifo", "BoundPair", "CorrelatedQueue", "ReplayCursor", "ReplayedPrefix", "PublicationBatch", "RecoveryRecord", "RawTranscript", "PrefixFeed"),
        "crates/sail-acceptance/src/inputs.rs": ("OriginalCapture", "OriginalGuard", "Readback", "GuardedReadback", "ProviderSeal", "OpenedRead", "ReadChunk", "ReadSeal", "CompletedRead"),
        "crates/sail-acceptance/src/authority.rs": ("AttemptLedger", "UnconfiguredAttempt", "ReservedAttempt", "RunningAttempt", "ObservedAttempt", "AttemptIdentity", "DeclinedEngine", "MissingEngineFeature"),
        KEY_CORE: ("Prefix",),
        WRITE_CORE: ("WriteCommit",),
        DELIVERY_CORE: ("Delivery", "Delivered"),
        OUTPUT_CORE: ("Transcript", "Frame", "FramePayload", "CompleteOutput"),
        OUTPUT_OWNER_CORE: ("StreamOutput", "StreamFrame", "OwnedFrame", "CompletedStream", "OwnedPublication"),
        HOLDING_CORE: ("Holding",),
        OPTIONS_CORE: ("OptionState",),
        REPLAY_CORE: ("ServedReply", "Replay"),
        KEY_NATIVE: ("SessionPrefix", "CommandFrame", "OwnedCommand", "InputDelivery", "DeliveredInput", "ObservedPublication", "OwnedObservedPublication"),
        PIPE_CORE: ("PipeCapture", "SealedPipe"),
        PUBLICATION_CORE: ("PublicationObservation",),
        "crates/sail-acceptance/src/phase_inputs.rs": ("PhaseRequest", "PhaseInputs", "EnvironmentProjection", "CommandEnvironment", "ConfiguredIntent", "OwnedCompilerAttempt"),
        "crates/sail-acceptance/src/query_prefix.rs": ("QueryPrefix", "FramedCommand", "FrozenCommand"),
        "crates/sail-api/src/acceptance.rs": ("CompleteReport", "ExactScopeReport", "CompleteSelection", "CompleteRequest", "CompletedPresentation", "SubmissionBinding", "SubmittedAuthority", "ReplayReadback", "ExactPartialRequest", "RequestPathBinding", "DeclaredReplayAdmission", "EmptyHumanOutput"),
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
                    or attribute and attribute[0] == "doc", "authority type derives unchecked constructor")
                previous = start - 2


def dependency_paths(component: str) -> list[str]:
    """The closed executable/model dependency set for one complete driver."""
    entry = COMPONENTS[component]
    result = [SYNTAX]
    result += ["proofs/acceptance/" + stem + ".rs" for stem in entry["models"]]
    result += ["crates/sail-acceptance/src/" + stem + ".rs" for stem in entry["core"]]
    result.append("proofs/refinement/" + component + ".rs")
    if component == "inputs":
        result += ["proofs/refinement/phase_inputs.rs", "proofs/refinement/stream_capture.rs"]
    if component == "query":
        result += [KEY_NATIVE, KEY_FACTORY, *QUERY_NATIVE_BINDINGS]
    if component in {"inputs", "authority"}:
        result += list(INPUT_NATIVE_SHA256)
    if component == "authority":
        result += ["crates/sail-api/src/acceptance.rs", "crates/sail-api/src/acceptance_sort.rs",
            *["crates/sail-api/src/" + name + ".rs" for name in SOURCE.API_MODULES],
            RETAINED_BRIDGE]
        result += [path for path in PRESENTATION_NATIVE_BINDINGS if path not in result]
    return result


def declaration_source(path: str) -> bool:
    """Only the inventoried declarations of these actual API modules compile."""
    return path == KEY_NATIVE or path in {"crates/sail-api/src/" + name + ".rs" for name in SOURCE.API_MODULES}


def compiled_subset(path: str, content: bytes) -> None:
    """Refuse source loading, unchecked specs and alternate executable cfgs."""
    if path == SYNTAX:
        require(hashlib.sha256(content).hexdigest() == SYNTAX_SHA256,
            "syntax erasure implementation changed; explicit runner review required")
        return
    if path in PRESENTATION_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == PRESENTATION_NATIVE_SHA256[path],
            "native presentation caller/assembly changed; explicit source review required")
        return
    if path == RETAINED_BRIDGE:
        require(hashlib.sha256(content).hexdigest() == RETAINED_BRIDGE_SHA256,
            "native retained bridge changed; explicit implementation/binding review required")
        return
    if path in INPUT_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == INPUT_NATIVE_SHA256[path],
            "native compiler/request/store adapter changed; explicit source/binding review required")
        return
    if path in QUERY_NATIVE_SHA256:
        require(hashlib.sha256(content).hexdigest() == QUERY_NATIVE_SHA256[path],
            "native query/write primitive changed; explicit source/binding review required")
        return
    if path == KEY_NATIVE:
        require(hashlib.sha256(content).hexdigest() == KEY_NATIVE_SHA256,
            "native key declaration/primitive bindings changed")
        return
    if path.startswith("proofs/acceptance/"):
        COMMON.standalone(content.decode())
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
    text = re.sub(r"#\[cfg\(sail_verus\)\]\s*use\s+(?:vstd::|super::inputs::|crate::authority::|crate::inputs::|crate::stream_capture::|crate::key_prefix::)[\sA-Za-z_:*,{}]+;", "", text)
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
    if path.endswith("/acceptance_sort.rs"):
        SOURCE.candidate_forwarders(text)
    else:
        inputs = path == "crates/sail-acceptance/src/inputs.rs"
        SOURCE.checked_item_envelopes(text, proof=path.startswith("proofs/refinement/"),
            reviewed_ghost_items=INPUT_GHOST_ITEMS if inputs else PIPE_GHOST_ITEMS if path == PIPE_CORE else frozenset(),
            reviewed_host_traits=INPUT_HOST_TRAITS if inputs else PIPE_HOST_TRAITS if path == PIPE_CORE else KEY_HOST_TRAITS if path == KEY_CORE else frozenset(QUERY_HOST_TRAITS.get(path, ())),
            reviewed_trait_impls=KEY_RECORDING_IMPLS if path == KEY_CORE else frozenset(QUERY_SCRIPT_IMPLS.get(path, ())))


def check_control_loops(before: bytes, after: bytes) -> None:
    """Refuse obvious loop removal before spending a native proof run.

    This conservative macro check supplements the authoritative verifier
    obligation/function counts; it does not establish complete scope equality.
    """
    require(before.count(b"sail_while!") == after.count(b"sail_while!"),
        "mutation changes checked loop syntax")


def inventory(root: Path, manifest: dict, snapshots: dict[str, bytes]) -> list[dict]:
    """Validate every root, actual source closure and selected negative control."""
    require(manifest.get("schema") == "sail-production-refinements-v1", "unknown refinement manifest")
    entries = manifest.get("components", [])
    require(len(entries) == len(COMPONENTS)
        and {entry["id"] for entry in entries} == set(COMPONENTS), "refinement root omitted or duplicated")
    actual = {str(path.relative_to(root)) for path in (root / "proofs/refinement").rglob("*.rs")}
    require(actual == {"proofs/refinement/" + name + ".rs" for name in (*COMPONENTS, "phase_inputs", "stream_capture")},
        "unlisted or missing refinement source")
    require({str(path.relative_to(root)) for path in (root / "crates/sail-acceptance/src").rglob("*.rs")}
        == {"crates/sail-acceptance/src/" + name + ".rs"
            for name in ("lib", "syntax", "query", "inputs", "authority", "key_prefix", "query_policy", "query_prefix", "phase_inputs", "cache_write", "query_delivery", "query_output", "query_publication", "query_output_owner", "query_holding", "query_checkpoints", "query_options", "query_replay", "stream_capture")},
        "unlisted executable leaf source")
    for entry in entries:
        component = entry["id"]
        require(entry.get("source") == "proofs/refinement/" + component + ".rs", "wrong refinement source")
        require(entry.get("dependencies") == dependency_paths(component), "refinement dependency closure changed")
        require(type(entry.get("verified")) is int and entry["verified"] > 0, "invalid verified count")
        require(bool(entry.get("functions")) and len(entry["functions"]) == len(set(entry["functions"])),
            "missing or duplicate full function inventory")
        require([control["name"] for control in entry.get("negative_controls", [])]
            == REQUIRED_CONTROLS[component], "selected mutation inventory omitted, duplicated or reordered")
        for path in entry["dependencies"]:
            if component == "authority" and declaration_source(path):
                continue  # Only the mechanically selected declarations compile.
            compiled_subset(path, snapshots[path])
        for control in entry["negative_controls"]:
            path = control["source"]
            require(path in entry["dependencies"] and path != SYNTAX,
                "mutation does not target a checked dependency")
            before, after = control["before"].encode(), control["after"].encode()
            require(bool(before) and before != after and snapshots[path].count(before) == 1,
                "empty or ambiguous executable mutation")
            check_control_loops(before, after)
            if path.startswith("crates/"):
                SOURCE.runtime_mutation(snapshots[path].decode(), before.decode(), after.decode())
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
        sources[path] = sources[path].replace(before, after)
    files = {"syntax.rs": sources[SYNTAX]}
    declarations = ["#![forbid(unsafe_code)]", "#![allow(unused_imports)]",
        "use vstd::prelude::*;", "extern crate self as sail_acceptance;",
        '#[macro_use] #[path="syntax.rs"] mod syntax;']
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
    if component == "query":
        files["key_types.rs"], _ = SOURCE.key_stencil(sources)
        files["prefix_factory.rs"] = sources[KEY_FACTORY]
        declarations += ['#[path="key_types.rs"] mod key_types;',
            'pub use key_types::{SolverIdentity, Mode};', 'mod prefix_factory;']
    if component == "authority":
        stencils, _ = SOURCE.api_stencils(sources)
        files.update(stencils)
        files["api_acceptance.rs"] = sources["crates/sail-api/src/acceptance.rs"]
        files["acceptance_sort.rs"] = sources["crates/sail-api/src/acceptance_sort.rs"]
        declarations += ["pub mod " + name + ";" for name in SOURCE.API_MODULES]
        declarations += [
            '#[path="api_acceptance.rs"] pub mod acceptance;', "mod acceptance_sort;"]
    files["refinement.rs"] = sources[entry["source"]]
    declarations += ['#[path="refinement.rs"] pub mod refinement;']
    if component == "inputs":
        files["phase_refinement.rs"] = sources["proofs/refinement/phase_inputs.rs"]
        declarations += ['#[path="phase_refinement.rs"] pub mod phase_refinement;']
        files["pipe_refinement.rs"] = sources["proofs/refinement/stream_capture.rs"]
        declarations += ['#[path="pipe_refinement.rs"] pub mod pipe_refinement;']
    declarations += ["fn main() {}"]
    for path, data in sources.items():
        if not (component == "authority" and declaration_source(path)):
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
        for name in ("sail-acceptance", "sail-api", "sail-key")), "unreviewed protected build script")
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


def source_roles(entry: dict) -> dict:
    """Keep compiled bodies, primitive contracts and bound native source distinct."""
    return {
        "checked_source": [path for path in entry["dependencies"] if path not in
            {SYNTAX, RETAINED_BRIDGE, "crates/sail-api/src/acceptance_sort.rs",
                *QUERY_NATIVE_BINDINGS, *PRESENTATION_NATIVE_BINDINGS, *INPUT_NATIVE_SHA256} and not declaration_source(path)],
        "reviewed_syntax": [SYNTAX],
        "abstract_primitive_contracts": {path: ["FileReadHost", "OwnerIdentity"]
            if path.endswith("/inputs.rs") else ["PersistenceHost"] if path == WRITE_CORE
            else ["InputPipe"] if path == DELIVERY_CORE
            else ["CompilerPipeReadHost"] if path == PIPE_CORE else ["HashHost"]
            for path in entry["dependencies"] if path in
                {"crates/sail-acceptance/src/inputs.rs", KEY_CORE, WRITE_CORE, DELIVERY_CORE, PIPE_CORE}},
        "reviewed_native_primitives": {path: ["Sha256Host", "PrefixHasher::new", "SessionPrefix",
            "CommandFrame", "OwnedCommand", "InputDelivery", "DeliveredInput",
            "ObservedPublication", "OwnedObservedPublication"]
            for path in entry["dependencies"] if path == KEY_NATIVE},
        "reviewed_presentation_callers": {path: PRESENTATION_NATIVE_BINDINGS[path]
            for path in entry["dependencies"] if path in PRESENTATION_NATIVE_BINDINGS},
        "reviewed_query_io": {path: QUERY_NATIVE_BINDINGS[path]
            for path in entry["dependencies"] if path in QUERY_NATIVE_BINDINGS},
        "bound_native_input_source": [path for path in entry["dependencies"]
            if path in INPUT_NATIVE_SHA256],
        "reviewed_native_bridges": {path: ["classify", "apply_currentness", "withhold_unbound_authority"]
            for path in entry["dependencies"] if path == RETAINED_BRIDGE},
        "declaration_projections": [path for path in entry["dependencies"] if declaration_source(path)],
        "structurally_checked_forwarders": [path for path in entry["dependencies"]
            if path.endswith("/acceptance_sort.rs")],
    }


def selected_entries(entries: list[dict], component: str | None) -> list[dict]:
    """Select one complete root only after validating the full root inventory."""
    require(len(entries) == len(COMPONENTS)
        and {entry["id"] for entry in entries} == set(COMPONENTS),
        "refinement root omitted or duplicated")
    require(component is None or component in COMPONENTS, "unknown refinement component")
    return entries if component is None else [entry for entry in entries if entry["id"] == component]


def parse_partition(value: str) -> tuple[int, int]:
    """Decode the canonical zero-based CI partition spelling."""
    parts = value.split("/")
    require(len(parts) == 2, "partition must be INDEX/COUNT")
    try:
        index, count = (int(part) for part in parts)
    except ValueError as error:
        raise Refusal("partition must be INDEX/COUNT") from error
    require(value == f"{index}/{count}" and 0 <= index < count and count >= 2,
        "invalid refinement partition")
    return index, count


def partition_cases(entry: dict, partition: tuple[int, int] | None) -> list[tuple[int, dict | None]]:
    """Every partition checks the whole positive root and its disjoint controls."""
    cases = list(enumerate([None, *entry["negative_controls"]]))
    if partition is None:
        return cases
    index, count = partition
    require(type(index) is int and type(count) is int and 0 <= index < count
        and 2 <= count <= len(entry["negative_controls"]), "invalid refinement partition")
    return [(ordinal, control) for ordinal, control in cases
        if ordinal == 0 or (ordinal - 1) % count == index]


def check(root: Path, archive: Path, install: Path, out: Path, timeout: int,
        component: str | None = None, partition: tuple[int, int] | None = None) -> None:
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
    entries = selected_entries(inventory(root, manifest, snapshots), component)
    require(partition is None or component is not None, "partition requires one explicit component")
    planned = [(entry, partition_cases(entry, partition)) for entry in entries]
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
    for entry, cases in planned:
        for index, control in cases:
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
        "key_declarations": manifest["key_declarations"],
        "validated_components": list(COMPONENTS),
        "checked_components": [entry["id"] for entry in entries],
        "partition": None if partition is None else {"index": partition[0], "count": partition[1]},
        "complete_component_controls": partition is None,
        "complete_suite": component is None and partition is None,
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
    parser.add_argument("--component", choices=tuple(COMPONENTS),
        help="check one complete root and all its controls; default checks every root")
    parser.add_argument("--partition", help="CI INDEX/COUNT: whole positive root plus disjoint guard controls; requires --component")
    args = parser.parse_args()
    try:
        require(args.timeout > 0, "timeout must be positive")
        partition = None if args.partition is None else parse_partition(args.partition)
        check(ROOT, args.stock_archive.resolve(), args.stock_dir.resolve(), args.out.resolve(), args.timeout, args.component, partition)
    except (Refusal, SOURCE.SourceRefusal, OSError, ValueError, KeyError, TypeError,
            subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print("error[SAIL9002]: production refinement check refused: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

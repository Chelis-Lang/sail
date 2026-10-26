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
# These exact native adapters are reviewed/bound, not compiled proof bodies.
QUERY_NATIVE_BINDINGS = {'crates/sail-cache/src/lib.rs': ['DiskCache::put_scoped'], 'crates/sail-cache/src/persist.rs': ['NativeWriteHost', 'write_record'], 'crates/sail-shim/src/cached.rs': ['SolverPipe', 'Solving', 'start_solver', 'Pairer']}
QUERY_NATIVE_SHA256 = {'crates/sail-cache/src/lib.rs': '106f1bd0fff663e6302be566de7b9e9c789e1e8433d354192e20f13db14b09dc', 'crates/sail-cache/src/persist.rs': '956ec4c117fb2b722a9a2454a9f1a60adf76e58a5a61397b1f144d4894d23260', 'crates/sail-shim/src/cached.rs': '66244d4097fe3299a106c0da6a8adb78f4ae90e73fa8fbae494e06604c6a47be'}
QUERY_HOST_TRAITS = {'crates/sail-acceptance/src/cache_write.rs': ['e1f217be87981cbd3d19140f85d8c7e8434568bd092cee9555efab9da58cf851'], 'crates/sail-acceptance/src/query_delivery.rs': ['127030cacff4e231147ffc843933effa0c754b3ad1963e460c5e981bcde92220']}
QUERY_SCRIPT_IMPLS = {'crates/sail-acceptance/src/cache_write.rs': ['b14cfdbc045ce226b03452823bf1706890c04c58817293134e9d1f2fc1ce12b4'], 'crates/sail-acceptance/src/query_delivery.rs': ['8378afe345bf6880d519cf675e3b5db5e4b01f6900781bef674f6c1fd7019d72']}
# These closed native compiler/request/store adapters are bound in full and
# remain outside the stock-compiled bodies. Their OS/process provenance remains open.
INPUT_NATIVE_SHA256 = {'crates/sail-verus/src/adapter.rs': '560df9ca1e2552b2df416f0d726677b919c785de3cb4ef7989cffad5b3620484', 'crates/sail-verus/src/adapter_attempts.rs': '23b2630ca3e065c21b2cff83cc555f830853eabd095de084af57fcb4358f8390', 'crates/sail-verus/src/engine.rs': '31eed396f2c97eb343dbc6596e2975da4b7464ed89856755adc95ed158db7dee', 'crates/sail-verus/src/managed/mod.rs': '0c85a5dd96e2af1cfda837742a11263c4d6b343cb4d9eeb436c8f1d03f86c9a3', 'crates/sail-verus/src/managed/command_environment.rs': '20f5d83a7cc5778467b840936bed1a8fdd22b8c724da4db1c55eafec76ea1071', 'crates/sail-verus/src/managed/runner.rs': '4e859e23217cb7c704dcf2500ecc7e9e72f442733652292e0043ed848ec91874', 'crates/sail-verus/src/managed/request.rs': 'dc079a01a60d64ee482540f5023deba9025cbc45010b6b2d1b56f2f7519bbaba', 'crates/sail-verus/src/managed/noop.rs': 'bf554605e9758b58dc634fb63dd19f3ce039b82b4bab3e27b3dbbcbead88893c', 'crates/sail-verus/src/managed/shipped.rs': '9b75f3e7bbc9b7fc46dee18448e9d2aadbbd9dfe185285f090b9ec3a654bd761', 'crates/sail-verus/src/managed/shipped/compiler.rs': 'ccf013b3f20798e3539f84a5893efdf82e14faaadd62d33a0b53637b9d3b5498', 'crates/sail-verus/src/managed/shipped/native.rs': '44fe0484d960e237d89972fd623d859e6080b9288db41d713403b7a93aefb3bc', 'crates/sail-verus/src/managed/shipped/native_reads.rs': '3fa562e94e38cb8f4489e072c60ea3cd7776ede5352ada907a29adaf63f5f580', 'crates/sail-verus/src/managed/retained/driver.rs': 'bd39ae74d8614a0d8457bac1dfa2e419b39bd5cce5bfea2765863d081ce2516a', 'crates/sail-verus/src/managed/retained/compiler.rs': 'e0a92daa80b5296cad7e10950b30fc1d49610250d1f00e3fa2dca8782df1da49', 'crates/sail-verus/src/managed/retained/native.rs': '7b4bf88dc3fabc99497de450d6f99a0f6d39b6da60f653f63deaade7910992fa', 'crates/sail-verus/src/managed/retained/producers.rs': '501246e0ccd8ec5bd5025fb985cb104adf8c40a540d4b93b03b4b645ac18ba4c', 'crates/sail-daemon/src/jobs/product.rs': '8333b8ad44f4f061dc0c1c2ee31e0128a2aa3ab4a63795364e1dee76328ea90d', 'crates/sail-daemon/src/jobs/result_cache.rs': '7d63c09a1e623d2204e64f593adc32e1527c04786ca57439aab1e64600680b64'}
# Native key types, SHA adapter, fresh-state factory and module/import bindings
# are reviewed in their complete enclosing source; they are not all proved.
KEY_NATIVE_SHA256 = "6cd5a45f3ec7c6f1e08d428a17370ef55f45e5d14bc463e081830db50f16ae5f"
KEY_HOST_TRAITS = frozenset({"07062a121a098f5dee4b51714ff0b575b1e6568347408344bc4aea5f3fe8be9a"})
KEY_RECORDING_IMPLS = frozenset({"de6e87c96d820b1610f24bc6b31aa3a64ba0e8d329446d476e08fcedcdcf5c3e"})
# The native classify/apply_currentness/withhold_unbound_authority bridges have
# been reviewed to forward the actual project and readback to the shared code.
# Their enclosing native binding environment is pinned in full; the module's
# I/O, serialization, cache selection and other service operations are not proved.
RETAINED_BRIDGE_SHA256 = 'd8645f52943fbf27b643e0fed073eaff11f4debaf8b478d829da2896ee42acd2'
# This reviewed source uses the identical argument/type/body metavariables in
# Cargo and stock profiles. Changes require an explicit runner review, beyond
# merely updating a manifest. Runtime-only implementation drift must refuse.
SYNTAX_SHA256 = 'deb8606ce9d123997dd286ae3e184c7496bdaf5e659543a7be724baffd9adc70'
LIB_SHA256 = "b2f75455527a64744043e406b17a5398164ec6853e33de277d22553234278339"
API_LIB_SHA256 = "125f9959d72bf6bbff719526cdaad169301b9c1e156db5b46ea79b91d4c50491"
# Binding inventories cover both ordinary and proof-only imports. In particular,
# an erased import cannot shadow a Cargo glob with a different checked callee.
IMPORT_SHA256 = {'crates/sail-acceptance/src/query.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/inputs.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/authority.rs': 'd0adfcd0a1e64a0addc3db112f9f9fc707f3f4c68bd7b907047a00b8d7f4abb4', 'crates/sail-api/src/acceptance.rs': '9f8ef81ea899c983acc574bb767129676586f597927d670398d92899b02527b7', 'crates/sail-acceptance/src/key_prefix.rs': 'abcb768a7dca61a2e4248956921314487e845306a6335e944610f3c5bb90e5b8', 'crates/sail-key/src/prefix_factory.rs': '22f9a04f035b109591f920519d7423c7de774bfc66315194b3939f291bf62988', 'crates/sail-acceptance/src/query_policy.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_prefix.rs': '6c28aff345658a06686d88714020c77540412e4b5403912081c62141c82059b0', 'crates/sail-acceptance/src/phase_inputs.rs': 'ee4babd632d862027f60d819530f1b50044f244caa94f318bd35e6658d567f4c', 'crates/sail-acceptance/src/cache_write.rs': '6440b768cb4ee15dd9522af6feee194bf50df30981cbc02a8cb5598b23e6a92d', 'crates/sail-acceptance/src/query_delivery.rs': 'dd5692fc8c75546d449a2c623708ec6748a7be654391385107f4fa20e971d08e'}
# Exact reviewed ghost declarations and abstract primitive contracts. These
# permit no arbitrary erased types or host implementations. The native host
# implementation is a separate obligation, never discharged by a trait bound.
INPUT_GHOST_ITEMS = frozenset({'34a7aa7c26994229805fe92e7e544f340920dbd3416ea1dcff4e4c8f7ea82697'})
INPUT_HOST_TRAITS = frozenset({'d5f071d9dcd1b4ec056b39af49e32cad198b1ebe2a4f692ce837364c83ca0691'})
PHASE_HOST_TRAITS = frozenset({'ec46570421f11527e24f55ff2737c1f9165486cfb02a197809c4d7b8b1aaab71'})
REQUIRED_CONTROLS = {'query': ['fifo_wrong_stack', 'partial_pair_consumes_head', 'replay_off_by_one', 'noisy_publication', 'corrupt_recovery', 'ungated_batch_release', 'incomplete_permutation', 'raw-ignore-command-bytes', 'raw-ignore-gap', 'raw-skip-separators', 'record-ignore-header-newline', 'record-change-separator', 'record-keep-separator-in-reply', 'frame-premature-root-close', 'frame-ignore-doubled-quote', 'frame-ignore-input-comment', 'head-ignore-framing', 'response-ignore-line-terminator', 'response-ignore-list-framing', 'key-solver-platform', 'key-mode-domain', 'key-raw-bytes', 'key-end-domain', 'key-counter-refusal', 'key-feed-order', 'key-silence-domain', 'key-state-copy', 'key-length-word', 'key-typed-platform', 'key-typed-optin', 'head-complete-leading-trivia', 'head-complete-untruncated', 'policy-tactic-head', 'policy-get-prefix', 'policy-whole-head', 'owner-policy-head', 'owner-original-append', 'owner-command-end', 'owner-hash-tail', 'write-omitted-full-write', 'write-omitted-rename-call', 'write-premature-metadata', 'write-cleanup-unowned-name', 'write-substitute-original-error', 'write-omit-owned-cleanup', 'delivery-omit-actual-pipe-write', 'delivery-hash-different-write-bytes', 'delivery-heal-failed-delivery', 'delivery-omit-terminal-key-binding', 'delivery-accept-closed-pipe', 'delivery-omit-owned-pipe-close', 'delivery-seed-drops-mode', 'delivery-seal-without-freezing', 'delivery-seal-unmatched-prefix', 'delivery-write-after-seal', 'delivery-token-different-prefix', 'delivery-token-different-domain', 'delivery-parser-drops-mode'], 'inputs': ['input-ignore-present-value', 'input-ignore-current-stamp', 'input-ignore-required-read-presence', 'input-ignore-inverse-permutation', 'input-provider-wrong-frame-child', 'input-provider-terminal-count', 'input-provider-required-read-frame', 'input-provider-required-read-presence', 'input-opened-object-binding', 'input-read-final-handle', 'input-read-final-path', 'input-read-chunk-bytes', 'input-read-eof-required', 'input-read-cache-object', 'input-read-requested-binding', 'input-environment-byte-name', 'input-environment-escaped-name', 'input-kernel-hash-bytes', 'input-kernel-final-handle-observation', 'input-kernel-final-path-observation', 'input-kernel-open-observation-order', 'input-kernel-buffer-required', 'input-kernel-read-failure', 'input-kernel-always-error', 'input-guarded-readback-change', 'input-guarded-readback-availability', 'input-guarded-readback-always-unknown', 'phase-run', 'phase-unit', 'phase-slot', 'phase-unit-manifest', 'phase-pid', 'phase-binding', 'phase-complete', 'phase-normal-wait', 'phase-count', 'phase-known-pair', 'phase-verification-fields', 'phase-compilation-fields', 'phase-no-foreign-fields', 'phase-always-refuse', 'phase-final-descriptor', 'phase-program', 'phase-argument-vector', 'phase-directory', 'phase-overrides', 'phase-override-value', 'row-checksum-role', 'row-size-key', 'row-tracked-value', 'row-env-absent', 'row-env-present', 'row-drop-final', 'row-fact-drop-final', 'row-fact-role', 'row-always-refuse', 'row-size-high-byte', 'filename-control-byte-refusal', 'filename-escape-prefix', 'filename-escape-value', 'filename-literal-byte', 'filename-cardinality', 'filename-constructive', 'environment-parent-rows', 'environment-parent-key-coverage', 'environment-override-coverage', 'environment-no-extra-or-stale-values', 'environment-clear-policy', 'environment-inherit-policy', 'environment-override-value', 'environment-inherited-value', 'environment-raw-name-key', 'environment-unreadable-refusal', 'environment-constructive', 'command-environment-actual-readback-extra', 'command-environment-actual-readback-omitted', 'command-environment-inheritance-retention', 'command-environment-original-builder-getter', 'command-environment-constructive', 'owned-config-env-cardinality', 'owned-config-env-row', 'owned-config-original-command', 'owned-config-map-binding', 'owned-config-constructive', 'owned-start-descriptor', 'owned-start-pid', 'owned-start-constructive', 'owned-completion-owner', 'owned-completion-pid', 'owned-completion-descriptor', 'owned-completion-wait', 'owned-completion-policy', 'owned-completion-output', 'owned-completion-status', 'owned-completion-constructive'], 'authority': ['verification-enabled', 'effective-verification-enabled', 'whole-terminal-summary', 'confirmed-function', 'whole-command', 'requested-coverage', 'summary-count', 'normal-exit', 'fresh-selection', 'obligation-uniqueness', 'provenance-counts', 'request-original-context', 'request-independent-coverage', 'request-verification-enabled', 'request-current', 'request-repair-function', 'request-positive', 'request-repair-presence', 'submission-arguments', 'submission-mode', 'submission-whole-authority', 'submission-positive', 'driver-token-bytes', 'driver-token-coverage', 'driver-sources-order', 'process-wait-required', 'process-distinct-slot', 'process-duplicate-stream', 'process-stderr-binding', 'process-recovery-bytes', 'process-payload-binding', 'process-refusal-recorded', 'retained-original-current', 'retained-complete-readback', 'retained-exact-revision', 'retained-view-currentness', 'retained-request-admission', 'retained-clear-refused-labels', 'retained-preserve-positive', 'target-exact-cargo-name', 'target-rust-name-normalization', 'target-kind-binding', 'process-configure-slot', 'decline-actual-zero', 'engine-complete-protocol', 'decline-refusal', 'repair-decline', 'origin-slot', 'origin-pid', 'origin-descriptor', 'feature-raw-origin', 'feature-available', 'feature-name', 'feature-normal-zero', 'feature-decision', 'feature-model-admission', 'replay-guard-content', 'replay-guard-availability', 'replay-guard-revision', 'replay-request-context', 'replay-request-invocation', 'replay-request-arguments', 'replay-request-targets', 'replay-transfer-readback', 'replay-transfer-original-authority', 'replay-transfer-report-preservation', 'replay-transfer-always-refuses', 'replay-readback-always-refuses', 'scope-require-focused', 'scope-exact-arguments', 'scope-plain-report', 'scope-successful-report', 'scope-always-refuse', 'whole-replay-full-binder-always-refuses', 'partial-target-full-factory-always-refuses', 'partial-request-full-factory-always-refuses', 'partial-transfer-full-binder-always-refuses', 'partial-transfer-full-binder-always-accepts', 'partial-report-raw-scope-check-removed', 'partial-report-engine-check-removed', 'partial-report-record-check-removed', 'partial-target-whole-label-checks-removed', 'partial-target-establishing-identity-check-removed', 'partial-target-recorded-scope-binding-removed', 'partial-target-raw-scope-check-removed', 'partial-request-coverage-check-removed', 'partial-request-target-report-check-removed', 'partial-transfer-independent-request-check-removed', 'partial-transfer-original-authority-check-removed', 'partial-transfer-actual-readback-check-removed', 'partial-transfer-report-inventory-cleared', 'partial-transfer-original-establishment-reset', 'retained-submission-full-factory-always-refuses', 'retained-submission-full-factory-skips-live-invocation', 'request-path-binding-full-factory-always-refuses', 'request-path-binding-full-factory-skips-raw-comparison', 'request-path-withholding-full-operation-always-accepts', 'request-path-withholding-full-operation-always-refuses', 'request-path-withholding-keeps-partial-scope', 'request-path-withholding-keeps-whole-completion']}
COMPONENTS = {
    "query": {"models": ["context", "query_association"], "core": ["inputs", "query", "key_prefix", "query_policy", "query_prefix", "cache_write", "query_delivery"]},
    "inputs": {"models": ["context"], "core": ["inputs", "authority", "phase_inputs"]},
    "authority": {"models": ["context", "authority"], "core": ["inputs", "authority", "phase_inputs"]},
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
    # sail-daemon's optional `temporal` feature is chosen by each member
    # (ADR 0080); only disabling default features is accepted, never a redirect.
    feature_selectable = {"sail-daemon"}
    for name in protected_packages:
        entry = workspace["workspace"]["dependencies"][name]
        expected = [{"path": "crates/" + name}]
        if name in feature_selectable:
            expected.append({"path": "crates/" + name, "default-features": False})
        require(entry in expected, "workspace protected dependency redirected: " + name)
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
        "crates/sail-acceptance/src/query.rs": ("Fifo", "BoundPair", "CorrelatedQueue", "ReplayCursor", "PublicationBatch", "RecoveryRecord", "RawTranscript", "PrefixFeed"),
        "crates/sail-acceptance/src/inputs.rs": ("OriginalCapture", "OriginalGuard", "Readback", "GuardedReadback", "ProviderSeal", "OpenedRead", "ReadChunk", "ReadSeal", "CompletedRead"),
        "crates/sail-acceptance/src/authority.rs": ("AttemptLedger", "UnconfiguredAttempt", "ReservedAttempt", "RunningAttempt", "ObservedAttempt", "AttemptIdentity", "DeclinedEngine", "MissingEngineFeature"),
        KEY_CORE: ("Prefix",),
        WRITE_CORE: ("WriteCommit",),
        DELIVERY_CORE: ("Delivery", "Delivered"),
        "crates/sail-acceptance/src/phase_inputs.rs": ("PhaseRequest", "PhaseInputs", "EnvironmentProjection", "CommandEnvironment", "ConfiguredIntent", "OwnedCompilerAttempt"),
        "crates/sail-acceptance/src/query_prefix.rs": ("QueryPrefix", "FramedCommand"),
        "crates/sail-api/src/acceptance.rs": ("CompleteReport", "ExactScopeReport", "CompleteSelection", "CompleteRequest", "SubmissionBinding", "SubmittedAuthority", "ReplayReadback", "ExactPartialRequest", "RequestPathBinding"),
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
        result.append("proofs/refinement/phase_inputs.rs")
    if component == "query":
        result += [KEY_NATIVE, KEY_FACTORY, *QUERY_NATIVE_BINDINGS]
    if component in {"inputs", "authority"}:
        result += list(INPUT_NATIVE_SHA256)
    if component == "authority":
        result += ["crates/sail-api/src/acceptance.rs", "crates/sail-api/src/acceptance_sort.rs",
            *["crates/sail-api/src/" + name + ".rs" for name in SOURCE.API_MODULES],
            RETAINED_BRIDGE]
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
    text = re.sub(r"#\[cfg\(sail_verus\)\]\s*use\s+(?:vstd::|super::inputs::|crate::authority::|crate::inputs::)[\sA-Za-z_:*,{}]+;", "", text)
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
            reviewed_ghost_items=INPUT_GHOST_ITEMS if inputs else frozenset(),
            reviewed_host_traits=INPUT_HOST_TRAITS if inputs else PHASE_HOST_TRAITS if path == "crates/sail-acceptance/src/phase_inputs.rs" else KEY_HOST_TRAITS if path == KEY_CORE else frozenset(QUERY_HOST_TRAITS.get(path, ())),
            reviewed_trait_impls=KEY_RECORDING_IMPLS if path == KEY_CORE else frozenset(QUERY_SCRIPT_IMPLS.get(path, ())))


def inventory(root: Path, manifest: dict, snapshots: dict[str, bytes]) -> list[dict]:
    """Validate every root, actual source closure and selected negative control."""
    require(manifest.get("schema") == "sail-production-refinements-v1", "unknown refinement manifest")
    entries = manifest.get("components", [])
    require(len(entries) == len(COMPONENTS)
        and {entry["id"] for entry in entries} == set(COMPONENTS), "refinement root omitted or duplicated")
    actual = {str(path.relative_to(root)) for path in (root / "proofs/refinement").rglob("*.rs")}
    require(actual == {"proofs/refinement/" + name + ".rs" for name in (*COMPONENTS, "phase_inputs")},
        "unlisted or missing refinement source")
    require({str(path.relative_to(root)) for path in (root / "crates/sail-acceptance/src").rglob("*.rs")}
        == {"crates/sail-acceptance/src/" + name + ".rs"
            for name in ("lib", "syntax", "query", "inputs", "authority", "key_prefix", "query_policy", "query_prefix", "phase_inputs", "cache_write", "query_delivery")},
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
        "source_roles": {entry["id"]: {
            "checked_source": [path for path in entry["dependencies"] if path not in
                {SYNTAX, RETAINED_BRIDGE, "crates/sail-api/src/acceptance_sort.rs", *QUERY_NATIVE_BINDINGS} and not declaration_source(path)],
            "reviewed_syntax": [SYNTAX],
            "abstract_primitive_contracts": {path: ["FileReadHost"]
                if path.endswith("/inputs.rs") else ["PersistenceHost"] if path == WRITE_CORE
                else ["InputPipe"] if path == DELIVERY_CORE else ["HashHost"]
                for path in entry["dependencies"] if path in {"crates/sail-acceptance/src/inputs.rs", KEY_CORE, WRITE_CORE, DELIVERY_CORE}},
            "reviewed_native_primitives": {path: ["Sha256Host", "PrefixHasher::new", "SessionPrefix", "CommandFrame", "InputDelivery", "DeliveredInput"]
                for path in entry["dependencies"] if path == KEY_NATIVE},
            "reviewed_query_io": {path: QUERY_NATIVE_BINDINGS[path]
                for path in entry["dependencies"] if path in QUERY_NATIVE_BINDINGS},
            "reviewed_native_bridges": {path: ["classify", "apply_currentness", "withhold_unbound_authority"]
                for path in entry["dependencies"] if path == RETAINED_BRIDGE},
            "declaration_projections": [path for path in entry["dependencies"]
                if declaration_source(path)],
            "structurally_checked_forwarders": [path for path in entry["dependencies"]
                if path.endswith("/acceptance_sort.rs")],
        } for entry in entries},
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

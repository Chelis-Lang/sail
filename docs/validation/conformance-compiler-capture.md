# Cargo conformance compiler capture

| Field | Current state |
|---|---|
| Tracking | #632, #716; parallel scaling remains #629 |
| Design | [ADR 0113](../decisions/0113-conformance-invocation-capture.md) |
| Runtime | Existing primary stock and fork pins; no runtime/solver changes |
| Implementation | Per-invocation capture in plain, shim/cache and engine arms; explicit project and nonempty selection guards |
| Status | Qualified at `1be7ba0a`; current-main rebase gate passed, protected PR checks remain |
| Linux | Native c7i.4xlarge/us-east-2: workspace 1,351 pass/53 skip, solver 50 pass, full E2E including extra |
| Conformance | All four arms: PR 33 Same/311 functions; full fast 147 Same/3,451 functions; standard pass-through/fill/engine 159 Same, warm 158 Same/one unstable Anvil; 13,166 functions, 32 drift units retained; no different/infra results |
| Mutation controls | 18 HumanEval mutants plus cold/revert/forced-revert: 21 Same; generator outputs identical in four runs |
| Acceptance | 35 stock-Verus model cases and 316 production-refinement cases pass; these qualify existing components, not the whole capture service |
| Mac | Exact `1be7ba0a`: host clippy and 1,338 tests pass/53 skip; book source/build/links pass; focused capture/process controls and actual EMFILE refusal pass |
| Rebased gate | `20925d364e0fd32e343c811542d69946615716b4`: full native workspace gate passes, 1,374 tests/53 skip; log SHA-256 `71ccae82ae56080ea388f6120497a01aa37d48fcd6bd5f04cd8903d07c96a885` |
| Reports | Archive SHA-256 `0f18ab127347da5d6cad9f428b3866edbe65194be7b497efe3d05230b9307657` |

All current Linux runners above exited zero on 2026-11-12. The standard
warm arm retains one unstable Anvil result: repeated plain runs reorder
verification-progress JSON notes. The original outcome remains unstable. A strict focused rerun of the same
Anvil unit at the same code source passes all four arms (each one Same,
143 functions, one drift unit) with zero unstable/different/infra results. No
comparison rule changes. Rerun archive SHA-256:
`c9c08b6ed9758f27e88d1c71b44a4cbf59b7d55a4997ceb221f6b1b3d2926e24`. Raw logs,
reports and captures remain outside the repository. The original failed
attempts below retain their own source identity and result. Earlier parallel
workspace evidence has the explicit scope described below and does not replace
current broad-corpus qualification.

The rebase onto main `d706abfc` preserves every checked model and production
source from the actual `1be7ba0a` refinement run. Four of its 338 bound source
paths change: the API root's reviewed campaign module addition, the separately
qualified ledger correction, the unselected mutation module assembly, and the
checker's separately reviewed API-root binding constant. The actual acceptance
runs remain attributed to `1be7ba0a`; this source audit does not relabel them as
new executions on the rebase.

The original strict VI-0039 failures remain in the
[reproduction](../../corpus/repros/verus-parallel-json/README.md). This
implementation does not repair Cargo's public stdout framing. A line torn
within a write, a missing/extra invocation, any unmatched probe, incomplete
capture or exhausted interleaving search remains a failure. Original
aggregate streams and every compiler record are retained. This gate is
separate from broad-corpus and scaling performance qualification.

The consumer verification skill has no command or default-verification change;
this is contributor tooling. Documentation changes are ADR 0113, the error
reference, testing guide, changelog and this qualification record.

## Failed implementation attempts

| Date | Source | Result | Raw log SHA-256 |
|---|---|---|---|
| 2026-11-11 | `72aa59ac` | Parallel process tests: two pass, one refuses scratch creation with `AlreadyExists`; fixture now adds a process-local atomic sequence. No host gate credited. | `1110b6c06f4007a4a13612514efab722136c0257c4ff60286c795816ecf16fee` |
| 2026-11-11 | `72aa59ac` | Independent native review rejects named build-script probe identity: two adversarial controls demonstrate source-specific statuses can swap. Complete arguments now bind every invocation; replacement qualification pending. | `da5ad53f887d132225e847c10ca998338bbfaa36c053f79a5df5b761cc3888c4` |
| 2026-11-11 | `e2d210ed` | Independent live staged controls reject omitted physical packed-option channels / outcome reassociation and a failed reservation accepted after a prior probe. Format-2 requested/effective channels and a separate capture health record are implemented; replacement qualification pending. | `7c5bb9b7ccf66881117908c7f5e1220d034373870bae1d10f405eab8fe5b9a58` |
| 2026-11-11 | `e2d210ed` | Real Mac parallel stock-versus-stock/shim managed arm fails on shared harness log directory (`multiset.smt_transcript` removal); original complete report/captures retained. No parallel pass. | Archive `d629038eb946a106138fd8ffeb2c8161865ee113d66e490261dcc609fefdf048` |
| 2026-11-11 | `5f888071` | Linux standard first arm: 156 Same, one Different (Anvil's randomized autocfg instance labels), two infrastructure refusals (binary metadata probes). Not a standard pass; later stages did not run. Full records retained; replacement source has strict stdin binding and known native probe handling. | `b5c0151a0941d5e385af714c7531448ef59eae6955c4a7024722ed783bb3292b` |
| 2026-11-11 | `1dc11f5d` | Independent live review rejects unbound stdin: different piped sources can exchange 0/7 outcomes. It also reproduces the binary-probe refusal. Complete stdin/format-3 replacement and bounded recheck pending. | Stdin archive `78325be8c36964e6ea3903e80663853b9d9a4a3fe940bd46d099816c74235f13` |
| 2026-11-11 | `5f888071` | Mac generated parallel stock/shim passes; fork arm differs on a selected-package `-vV` structured JSON version field. Adjacent Rust verbose text and actual verification fields match; the initial probe rule was stricter than the requested existing structured-stdout oracle. | Archive `701193925dbfb9fc31434d3df54031f7d2004a49d80e0f94acb31f49fffc1b77` |
| 2026-11-11 | `928e1725` | Independent staged control: an earlier valid job followed by an inaccessible health record can hide a failed probe (tee exit 125) and classify Same after health restoration. Preflight now reserves its incomplete marker before checking either driver or health; replacement qualification pending. | `e7440f3dde034e80c44f9600db630cad9d9f59160096037783dd768ba3d8c66d` |
| 2026-11-11 | `97f162b9` | Health-preflight controls pass; independent actual-rustc staged control rejects generated-probe handling with duplicate crate-name flags: arbitrary second names can disappear from identity despite distinct arguments. Recognition now requires exactly one separate flag and preserves all other input bytes. Replacement qualification pending. | `dda699ea7ab1069b536a1d19de73cd20d9678fa6c2707750edc6e3d8ed6dbed9` |
| 2026-11-11 | `6e0b70bc` | Independent actual-rustc controls reject a literal `<source>/empty` colliding with a real source-root path, in both argv and the physical packed channel. Typed literal/rooted identities now preserve complete prefix and suffix spelling. Replacement qualification pending. | `693e229646692d9807e6f9fa3c11c19a9094012c97286d37e282acd69b07401d` |
| 2026-11-11 | `09008e1c` | Typed-root and all earlier identity/preflight controls pass. Independent staged controls reject simultaneous reservation/health-write failure, or cleared capture environment: a hidden tee125 can disappear and paired invalid runs classify Same. A companion prototype supplies an independent witness; later rows record its rejected lifecycle and detachment limits. | `c68df592380ee8e44b2b5f47d267aef81a87b0ad1ecfa6f5876a50f7b1b95ee1` |
| 2026-11-11 | `ead79896` | Companion prototype: 71 focused controls pass, including dual-channel and cleared-environment failure; host clippy refuses an oversized helper. No host pass credited. | `ebe5ede618de6e9d9d21f89ffc20765f7f8d69a09e23c1c696a2542b1bc6b6e8` |
| 2026-11-11 | `69286690` | Refactored companion: focused controls pass; host clippy refuses an implicit PathBuf clone. Explicit clone fixes style; replacement host qualification is separate. | `9b0f48ef2df34a525982abafbcf7bd1b94918fdf359c0dfeeb0b39ab9d93355a` |
| 2026-11-11 | `928e1725` | Historical native Linux gate, solver, full E2E, PR/full-fast four-arm suites and the first standard arm pass. The second standard arm is interrupted to replace the independently rejected failure witness; remaining arms, controls and proofs did not finish and earn no pass. | `a6363cae50c052b8087662699efd5e7453c207999211504c01b80d49dc73c372` |
| 2026-11-11 | `75113f1a` | All 30 earlier controls pass; independent native read-only-stage witness rejects PID-based cleanup: alias retirement can fail before the companion is reaped. Replacement signals only the caller's matching current owned group, never a PID from an artifact path; a retained-alias/foreign-process control is added. | `d6cdc08278720353f5a534c0ac525c29e1a7f1942aab2bd0ad6fdafdcc03996c` |
| 2026-11-11 | `f5d3285a` | Current-group abort and foreign-process controls pass; host clippy refuses a 101-line helper. Equivalent optional finalization expression keeps the helper within the configured limit; no lint exception. | `888ef4cf61ceb96fee4494388f8ac98244cd3d009f5a069043c0d33753ca100d` |

| 2026-11-11 | `178455a9` | Independent detached hidden-probe witness rejects current-group signaling: the probe clears capture environment, exits 125, and paired invalid runs classify Same. Parent-owned TCP completion replaces group signaling; replacement qualification pending. | `2b9309901519d2c4c6e8bfb5573396670d5bd85d1b8976292af813ba850803c5` |

| 2026-11-11 | `0a9ab14d` | TCP completion: 73 native Linux focused checks pass (two control, 58 library, five CLI, eight process). Clippy rejects three style issues; no full gate credited. | Raw SHA-256 `0c7c93580daf2ec4c35db40042493d125560678e4f3b08552c701df49478844c`; replacement lint run pending. |

| 2026-11-11 | `4eb575e8` | Native Linux 73 focused/clippy and Mac 1,338 host tests pass (53 skipped); independent paired production control rejects missing fatal-entry witness under EMFILE: hidden entered tee125 can disappear and classify Same against a healthy plain arm. Safe stdin-release witness retry replaces that fatal entry path; qualification and new lock-edge binding review pending. | `65517d3ee10d09d1bc5be0c03ebb40493820ef6a860680ec75e148c58869af44` |

The log collision is a harness setup defect: `run_unit` formerly added one
unit-wide `--log-dir` to Cargo-Verus. The actual product driver in `adapter.rs`
binds `runtime.sidecars.join(identity.key())` and each target's own `logs`
directory. The correction uses separate compiler-owned directories through
that same packed-option helper; it does not disable logging, serialize Cargo,
ignore diagnostics or alter solver input. The failed run remains failed.

Independent raw review of the subsequent `1dc11f5d` parallel runs confirms
nine complete compiler documents in every selected run, with 1,882 functions
and zero comparison differences in both stock/shim and fork arms. Aggregate
stdout actually tears after seven documents in stock plain/Sail and fork
plain/plain-2; the other two selected streams parse all nine. Every stream
still has a complete byte/newline interleaving witness. Stale confirmation
folders from earlier failed attempts are not selected by these reports'
`plain_runs = 2` and earn no credit. This framing evidence does not qualify
the later stdin/binary changes or the full standard corpus.

The actual rustix metadata probe retained from the failed Linux arm has argv
`--crate-type=rlib --emit=metadata --target x86_64-unknown-linux-gnu -o - -`,
2,262 stdout bytes, exit zero and SHA-256
`3e7c95b09300d42c1fe9aa722143f764dc009acbcef1b4bb388fdc0c9fcdf5a0`.
Raw record archive SHA-256 is
`1172bfd28c27c549eaf87b95cb67703d540e27923cf5d73a2b8f7f11c5be868a`.

The independent fatal-entry review approves exact `33645a50` in its bounded
scope. Actual entered hidden tee EMFILE failures retain exit 125 and SAIL3018,
but now refuse production pairing. The old delegate still demonstrates false
Same as a negative control. The review also reruns prior identity, stdin,
probe, metadata and lifecycle controls; 53 source-capture tests plus eight
exact-byte acceptance/drift controls and an additional lock mutation pass.
Review SHA-256: `091e325dcc0a6f54825651e681abba9766de669a1dfd1f79cbce7fa066d8658b`;
native delivery: `29febde6dd8585db2ddcf4ecb8b854eb6a211f08e8a376ae490723b639746877`.

The new lock adds only the contributor's Unix dependency on already locked
nix 0.31.3; no external version or feature changes. The independent reviewer
explicitly approves only the parser-lock binding advance from 8cf710f0 to
`9d3b3c962dab6b5523db7409697758b8c1df8d322fc73bdb94c7e81ecc1c7685`.
All parser/proof/checker bodies remain unchanged. Current full native Linux,
Mac, real-project conformance and affected proof/refinement qualification are
separate remaining gates. The bounded review claims no system-wide ENFILE or
exhaustive process-entry-failure theorem.

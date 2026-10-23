# Production integration qualification

Assessed October 23, 2026. Qualified executable source is
`64a49aa5aa43f69b452424dd8c39bc9079d03569`, tree
`7716977b53a74dae3de955964a43d656877e7541`. It combines native temporal jobs,
neutral source assurance and bounded mutation retention with protected main
`e03dfa0b0186a9a834c66d7ca7e349a2e36c78bb`. The final delivery preserves the qualified Rust, Cargo and runtime inputs.
The benchmark-only main update below is separate from runtime qualification.
Protected delivery requires every hosted gate.

The native worker links D.I.V.E.
`1025cd6a0510183bc16abc1e6e3eea4f9305c5a9`. Neutral macro and grammar bytes
match `b3ea2007fa31a71e867c2278860ebe4b5573d34c`. Corpus entries, runtime pins
and default deadlines are unchanged. This qualification establishes no performance
improvement, positive native neutral-source mutation kill, application trace
capture or full realizability.

## Current cumulative source

The delivery also incorporates protected main
`fc29023ae0578cf435aa295a604ee6d5a09f4a39`. Its only non-Markdown changes
relative to the previously qualified integration are `scripts/perf-matrix.py`
and `scripts/test-perf-matrix.py`. Passing-workload rows now require successful
proofs, Vest receives the required proof limit on both arms, and a failed
worktree baseline cannot qualify a benchmark. These benchmark entrypoints are outside the runtime
compile and job-execution dependency paths. Rust, Cargo, runtime pins, corpus and
conformance runners are unchanged. The affected Python checks, final native
workspace gate and rebuilt installed-service controls have separate per-head
receipts in [PR 353](https://github.com/Chelis-Lang/sail/pull/353). Existing
Linux/Mac runtime and managed-path results retain their original source identity;
this benchmark correction establishes no performance improvement.

Exact Cargo target names stay distinct even when Rust normalizes their crate
names; ambiguous legacy observations and duplicate reports are refused with
`SAIL3107`. Source revisions and retained read checks bind symlink destinations
and linked ancestors. Retargeting an alias invalidates evidence even when both
destinations were already hashed. Legacy records cannot establish Current.

Guarded observations share complete source reads only within one verification
call and only under their original mutation stamp and observed namespace.
Uncovered inputs, uncertainty and changed aliases require fresh reads. Compiler
identity is not memoized and source hashes are not retained between calls.
Filesystem resolution of `alias/../file` is preserved. The merge's only manual
resolution retained both changelog entries.

Fresh isolated review approved this bounded composition. Clean serial checks
passed 429 library, 22 API, nine neutral-contract and two mutation tests, with
explicit ignored oracles retained. Independent probes covered alias/`..`, byte
and link changes, exact revert with restored mtime, new namespace and missing,
unreadable or incomplete observations. Initial parallel inherited-target Cargo
fixture failures were retained; clean serial reruns passed. The review does not
replace platform, backend or complete application acceptance.

| Original external artifact | SHA-256 |
|---|---|
| Fresh cumulative review | `64c273662cd0b0cdfbc06c68b659265fa259571d071cb04547988356c82efb8d` |
| Fully rehashed review index | `c9ca5f0004e7f0a8cc5206b0f1021e0557d3bbb9dd54644f6a9164e4d32b7d48` |
| Independently checked component map | `89ffbd3a9d6c2164c177a032a5b495eab14233349204f2b0cc0bef4818334bc2` |

## Native Linux

The clean cumulative source passed the native x86-64 Ubuntu EC2 workspace
gate, pinned solver suite, complete E2E including local extra, installed-service
controls and fast PR conformance. Each fast PR arm matched 33 units and 311
functions, with five rlimit-drift units and no unstable/different/infra results
or skipped projects. The workspace gate passed 1,044 tests with
44 explicit skips. Fresh real managed HumanEval execution matched all 21
comparisons: cold, 18 selected mutants, ordinary revert and forced revert. Both
reverts used their own cached answers. No unstable/different, false-verified,
infrastructure or catalogue mismatches were reported. Original E2E logs include
actual source-alias invalid-proof rejection, Current withdrawal and exact
restoration. The complete phase log and designated output/loader/flags/extra/
contract originals are retained. Legacy temporary comparison streams were
removed with the container; their successful comparison summaries remain.
HumanEval retains original runner/header/samples and phase log, without claiming
individual raw compiler streams. No performance gain is inferred.

The installed worker exercised safety, refutation, replay, fairness, projection,
design checks, bounded stopping, unsupported input, deadlines, observed-worker
cancellation, shared SMT and changed-worker identity refusal. Source controls
exercised actual proved positive/guarded inputs, unproved edited/compound inputs,
read-after-restart and retained originals after pruning.

| Current-source original log | SHA-256 |
|---|---|
| Workspace gate | `c1b6bfa5ac563b8036f37c0e152b6964ae169f4a49ec7fdc820566ebb437e303` |
| Pinned solver tests | `871611fee267c02cf2357441bbb11829bb50f0be5f0e8884625d727ac198b892` |
| Installed native/source/mutation controls | `f54683f7f505092c361708544360c8cb209d55b272a38646873442c53b7afebc` |
| Complete E2E and local extra | `cc3c0f9bd040e8f71c1d3ce71a217f8ae98eb4a68f675009d3b5c81c42556eae` |
| Fast PR conformance | `da609eb35e4923fd780152742c86e4aacdd6d1835386ab1952bb37e0a8349979` |
| Fresh managed HumanEval probes | `f2c34057d7e80f9f0bb2c52cb338548edbb56bfc5fc5ced9f0bf70bc28289ccc` |
| Complete managed HumanEval export | `fa56e1da6eba1fd0b2905fd72b4a8504967d31f63fcf770e75ec43bbaec42120` |

The complete installed release and original native/source/campaign export has
SHA-256 `6818a7270e12f233ca04a65a7ac103f22c66ab72deb9fb2682cb5d0fa3f56cc6`.
Every file stream was read and hashed after transfer. The Verus runtime manifest
is `35fdb4942460887104955f409a7dd1745207de2abb829d89061b0ab93ce0b2dc`.

The complete current-source core archive has SHA-256
`a5f30dd10cf8e6adb2a04673b2cbedf83c127c19f33478ccf31ff5802d7ffd84`;
managed/repair originals have SHA-256
`fda3f6f3b709e34d04b1bdfff6b93bf18619de4f76cc9cb013afe92ca2b7ae34`.
Every indexed original and archive file stream was independently rehashed.

## Unchanged corpus components

Original full-fast and standard comparisons executed on
`e0518f43c39f2e352d0432a079c3c0defd5b2114`. Every full-fast arm matched
147 units and 3,451 functions; every standard arm matched 159 units and
13,166 functions. Standard retained 17 rlimit-drift units per arm. Neither suite
had unstable/different results, infrastructure errors or skipped projects.
The standard engine arm exercised 158 focused units. Default unit timeouts and
selection were preserved; standard used jobs=6.

The component map and independent review compare actual Git objects for the
corpus, direct Verus/Cargo-Verus harness, prebuilt engine, shim, daemon, cache,
solver and runtime pins. These unchanged paths do not traverse the altered
managed input/receipt runner. This is explicit component qualification, not a
claim that an old binary or run executed on the cumulative revision. Current
source jobs, E2E and real managed HumanEval mutations/reverts run separately.

Independent readback recomputed all arm metrics and unit identities, verified
every indexed original and archive member, and checked both own-answer revert
controls. The earlier standard HumanEval also matched all 21 comparisons; that
managed execution qualifies e051 only.

| Original external artifact | SHA-256 |
|---|---|
| Full-fast log | `476ef9c454227086941adb21c5d2856f4f0b70e0655ce9f76cfb28cba5ad1c5b` |
| Standard log | `b366bbd95ae987b3a3f74f630790335ec6d5f21a507f5efe52f43b3dd669024c` |
| Standard report | `80baedb89c79bc4fdd27495ba7cb16a8fa3a32dba1d96e2b605f586f070cd6d1` |
| Standard original artifact index | `557d61e1e2390cc099fe843fc48256b0de2bd501eb6f5f31a949e917f0d54e2c` |
| Complete standard originals archive | `8d8b1b81e8ee70fca2b6b3fa6f224b169bc4687c1526bc012f5cc5cca33425d7` |
| Standard source/run handoff | `bdc99cee95902a46b4e7ba4d40cce87d9879490afdcbea4aff48f50959658ce3` |

## Native macOS

Actual frozen source `64a49aa5aa43f69b452424dd8c39bc9079d03569`, tree
`7716977b53a74dae3de955964a43d656877e7541`, passed `ci/run.sh host` on
Apple M4/arm64, macOS 27.0 build 26A428, Rust 1.99.0 and nextest 0.9.143.
Strict workspace/all-target Clippy and 1,033 tests passed with 44 explicit skips.
The host gate completed in 144.395 seconds. CLI, shim and native worker binaries
built successfully in 13.650 seconds; locked dependency metadata and clean
before/after source identities passed. Actual Cargo target collision, alias
retarget/currentness, linked-ancestor, guarded source-observation and retained
timing regressions passed.

This qualifies cumulative-source native build, lint and host tests. It does not
claim a new actual Mac backend/source campaign or performance improvement.
Earlier e051 installed Mac backend evidence keeps its original source identity.
Strong neutral mutants remain Unknown and positive native kills are unqualified.

Every archive file stream was read and hashed; all 21 regular members, exact
source/tree, completed stages and their log hashes were independently verified.
Complete archive SHA-256: `d42ba27a6d2fe384e484822c8b64f416ab2f43cd83fad8d1b6617176f856ac0e`.
Host-gate log SHA-256: `167fdfaad12ff58892b4b15148f8f2ccbbb5bcc3525679ffdacc147181c92522`.
Native-build log SHA-256: `9f73751d9fc72a59185eb40eaca6691c877eb2be3abc1804422a18507446b646`.


The earlier e051 Mac acceptance separately exercised actual installed temporal
and source backends, conservative mutation controls and strict Linux readback of
original Mac receipts. Its runtime manifest is
`f1dcf457ad0b4fd66073db3462179828944bbda3176c088348a5851f43fd339b`;
its complete archive is
`58c563dcec405e1cc0b8b3ee6057ccf7e2937afe3da4b2cd04fc736030fb1959`.
Those backend executions keep their original source identity. The cumulative
host gate qualifies changed platform build/lint/test paths and does not relabel
the earlier backend controls as new execution. See the
[earlier Mac record](integration-macos.md) for its original scope.

## Receipt compatibility and consumer boundary

The production Oyster CLI exposed `SAIL3144` against the earlier `0c002dc2`
producer after a successful source proof and independent SMT link. A retained
backend timing decoded differently under default JSON parsing and downstream
`float_roundtrip`, causing unequal authenticated backend hashes. The corrected
workspace enables that feature for the service and API. Exact full backend
hashes remain mandatory; old inconsistent receipts remain refused and untouched.
[ADR 0075](../decisions/0075-retained-json-roundtrip.md) records the boundary.

Fresh review reproduced the original raw-RPC failure, proved matching bytes for
both downstream feature configurations, and refused altered timings, adjacent
value hashes, wrong proof/member hashes, edited source and promoted mutation
verdicts. The regression failed before the fix and passed with only the feature
enabled. Original diagnosis index:
`659c5be40f423ff2dd9527e7a85cca5da7d368fc8171fc2cdae80a31bf4ec025`;
fresh review index:
`2fdc95df92e19f92029ecfdbe481abef3d6294a000d6228c6e4d2e3a12a73447`.

A complete default-feature Oyster CLI rehearsal against the corrected e051
producer passed source proof, independent link/report queries and campaign
import. The campaign remained inconclusive, its passing gate false and CLI exit
1. A body edit preserved the contract link while removing the current proof tier;
negative reproof did not restore it. Its original artifact index is
`cf44965805b3b8c1077a813ac10048995637350cef77a35e371176330ce9f364`.
That rehearsal used a candidate API path. Final immutable SDK selection, consumer
gates and protected consumer delivery are separate Oyster acceptance gates.

## Limits and retained attempts

Actual strong neutral-contract literal mutants return Unknown with incomplete
quantifiers and no SAT rejection. Strict native mutation acceptance fails as
required. Observation mode qualifies lifecycle, survivor, deadline, cancellation,
restart, pruning and tamper controls while retaining
`native_kill_qualified: false`. Neither observation exit zero nor matching
HumanEval mutants establishes a native neutral-source kill or raises a proof tier.
Broad source admission, conclusive kills and implementation trace capture with
provenance remain future work.

| Source/date | Retained result |
|---|---|
| `0c002dc2`, October 23 | Failed production campaign import, authenticated timing mismatch; original bytes retained in the diagnosis above. |
| e051, October 23 | Initial E2E interrupted and existing-output retry refused; complete fresh run passed without source or admission changes. Original core archive: `072b42c6e9ca21e19d249f534ee8e0fc2735e8d8e9a48b9868a597f2195f2dea`. |
| e051, October 23 | Standard transfer framing refused before tests; complete unchanged retry passed, indexed in the standard handoff. |
| `b2fcda95`, October 23 | Review snapshot changed during final binding; review remains incomplete: `fef272db51bf4cfebec921f73979f702de5924fcea51395a901a6719070be52b`. Replaced by isolated frozen reviews. |
| `e6f4f526`, October 23 | Earlier cumulative Linux/Mac components retained under their original source; superseded for changed managed paths. Mac archive: `98d64ec0feca3cf0b7107a3b0f215d0c0dc7e396d548ac916d5b01d5f5cf3e2f`. |
| `64a49aa5`, October 23 | Initial parallel inherited-target review fixtures failed; clean isolated serial checks passed, original failures remain in the cumulative review index. |

The cumulative Mac validation report has SHA-256
`6e026f5329cd890ece8a61d4234618ca64b5c263d0862e4f8f56ef2938bbbce3`; its independently rehashed artifact index is
`fdc2690c9fc923149e70515d8193f9fe43794e2a567daca1fb87cf028641d896`.

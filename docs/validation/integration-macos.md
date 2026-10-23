# macOS integration acceptance

Assessed October 23, 2026. The required macOS host gate and installed native and
neutral source controls passed for `96d80b3a0dbd9e265f55ca7374221b1ca31d2274`,
tree `e503d72442fb9faeafc6b548766af30230267eba`. The source was clean before
and after execution. Linux conformance and protected delivery are separate
gates. Positive native mutation kills remain unestablished.

## Environment and execution

Execution used an approved native EC2 `mac-m4.metal` host in `us-east-1d`,
macOS 27.0 build 26A428, Apple M4 arm64, Rust 1.99.0 and nextest 0.9.143.
The pinned Mac Verus runtime used Rust 1.99.1 and Z3 4.16.0. The installed
worker links D.I.V.E. `1025cd6a0510183bc16abc1e6e3eea4f9305c5a9`.
No model provider was invoked.

| Check | Observed result |
|---|---|
| `ci/run.sh host` | Strict workspace/all-target Clippy and 1,018 nextest tests passed; 44 explicit skips; exit zero in 122.008 seconds. |
| Build and stage installed Sail, shim and worker | Passed; Mach-O arm64 binaries and manifest bytes independently matched. |
| Actual installed native transport matrix | Passed with fresh cache and socket, including holds, refutation, valid/forged traces, stopped/sampled exploration, unsupported input, deadline, cancellation and restored capacity. |
| Serialized process/shared-service controls | All 11 passed. |
| Actual `check_contract_jobs` | Passed positive/guarded Proved and body-edit/unsafe NotProved controls, original evidence, restart and pruning. |
| Strict `check_mutation_jobs` | Exit one: strong mutants remained Inconclusive, UNKNOWN with incomplete quantifiers and SAT zero. |
| Same example, `--observe-inconclusive` | Lifecycle, survivor, deadline, cancellation, restart, pruning and tamper controls passed; `native_kill_qualified:false`. |

Finite native holds and supplied-trace validity establish neither full
realizability nor an implementation evidence tier. Source proof refers to its
exact snapshot; the producer reports live currentness as unknown. It does not
establish independent contract-to-requirement entailment, required-input
feasibility or termination. Mutation observations do not promote a proof tier.

## Original evidence

Complete receipts, original streams, installed binaries, persisted source and
campaign evidence, command argv and logs remain outside the repository. The
merged archive references the separately retained immutable runtime; compiler
build caches are excluded. Complete archive reads, member hashes and referenced
runtime digests were checked independently after collection.

| Artifact | SHA-256 |
|---|---|
| `mac-merged-evidence.tar.gz` | `9337cfe56f5ab48d4df100c0500cceb01108543e8a0bd2676aaaade57e095b3a` |
| External qualification record, `validation.md` | `05f68990008f59a2a76d96ccd75d09ad844d218d32b061a6b6dfc1173fb03801` |
| Original receipt/log index, `artifacts.json` | `db7cdf99b728d8dd2626fcfb456df3913b10ddf14804e67125d61b75f1318798` |
| Installed Sail | `19ff801450eed749bc910bae922f78dc8556442d58207c8241bb768a797503bc` |
| Installed native worker | `9cbf3153f277ed50e881ba17503716036a2622aca3f497c3c9095e932fd73369` |
| Worker manifest | `1c69fa997da72ac915f8fba48bd1e3a682f0ecfdbe9cb52b436036f63dfe0004` |
| Shared Mac runtime manifest | `f1dcf457ad0b4fd66073db3462179828944bbda3176c088348a5851f43fd339b` |
| Separate earlier `14940a5b` archive | `1c180438caf33a1c9d19b2ca65f70f022efeee28f5cef8aa9227c25276bfa0e9` |

## Preserved attempts

| Source and attempt | Result |
|---|---|
| `14940a5b`, initial launcher | Failed before checks started; detached supervisor retry completed. |
| `14940a5b`, supplementary parallel libtest | Cancellation observer could match another parallel worker; required isolated nextest and full serial rerun passed. Original failure retained. |
| `96d80b3a`, initial external transport probe | Cache path already existed; failed before service startup. Fresh-path rerun passed the unchanged matrix. |
| Both sources, strict native mutation acceptance | Failed as required for UNKNOWN/SAT-zero outcomes. Observation runs do not replace positive kill qualification. |

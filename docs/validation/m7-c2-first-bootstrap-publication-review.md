# First c2 repair: independent export refusal is not attributed

This file-only review reads exact `c2a4b23` production source and retained
records from the [completed benchmark failure](m7-benchmark-c2a4b23-warmup-terminal.md).
It starts no native actor and changes no bootstrap or proof predicate.

The first repair journal has six custom-build compiler artifacts, all
`fresh=false`. The next has five `fresh=true` artifacts and a rebuilt vstd
script. First full validation exits 0, has no execution error and has a complete
requested HumanEval target, but remains Unknown. These facts do not establish
a clean independent bootstrap owner.

In the frozen [runner](../../crates/sail-verus/src/managed/runner.rs),
`run_engine` acquires the optional bootstrap before cleaning, independently of
the original `source_coverage` flag. That flag contributes to proof
`before.complete`. `engine::repair` invokes the unfocused full runner;
independent publication does not require the proof to be Current. Missing
initial source observations therefore explain a possible proof refusal, but
do not themselves prevent this export. Generated source remains fully observed.

The captured graph has 20 packages. Six scripts ran, while metadata also names
the unscheduled serde script. All six scheduled packages have only normal/dev
dependency roles; their declared environment keys are the permitted
`RUSTC_BOOTSTRAP` and `RUST_MIN_STACK`. The disabled-package and later retry
journal corrections address separate coverage gaps. They do not yet explain
the first independent export's absence.

The [bootstrap](../../crates/sail-verus/src/managed/build_inputs_bootstrap.rs)
has further necessary refusal points: acquisition/resource limits, runtime
matching, declaration hashes/environment, guarded source and dep-info membership,
executable alias/header checks, final output rereads, complete input rehash and
final namespace/lease cleanliness. Each returns `None` without a retained stage
reason. The public report has only the declaration-transition note, not an
independent-owner result. Neither capture binds the initial owner inventory,
admission result or first failed predicate. Post-run covered records and
`fresh=false` cannot retroactively establish continuous evidence.

If the corrected product still refuses export, a bounded optional stage and
first-refusal ordinal from the same check would distinguish construction,
runtime mismatch, individual script refusal, final hash/environment change and
owner invalidation. That observation must add no barrier I/O or change to
refusal. No instrumentation or additional experiment is implemented here.

| Retained record | Bytes | SHA-256 |
| --- | ---: | --- |
| Contract/journal capture | 41,327 | `b0ca34be06e8b06027b306de0712ef84b897ded3f1e6ef2aa1c5f818725c63a4` |
| Package graph capture | 176,983 | `ae9297e63a7abf0205037f69d37a4b69b61a30cfa0a75bdc7f846674b2aad96a` |
| Archived first repair stdout | 2,379,947 | `14360fdb0b036eaf4ac2614f50a577b312daaeb4b200a4297b4ecd86ece45b2e` |

The benchmark record binds the raw archive and selected product. No historical
730 cause, recovery latency, successful export or accepted Current proof is claimed.

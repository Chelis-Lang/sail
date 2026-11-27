# Symbolic rechecks (unreleased)

These commands are not part of the installed Sail 0.4.0 product and are not
listed in `sail help runtime`. They need a Dive installation that carries the
symbolic proof worker, which D.I.V.E. 0.2.0 does not ship. They become
supported with D.I.V.E. 0.3 (Chelis-Lang/dive#1417, Sail
[#805](https://github.com/Chelis-Lang/sail/issues/805)). Until then
`sail runtime symbolic-check` still runs against a development Dive that
provides the worker, and its tests run against fixture workers.

## Recheck finite safety properties with a portable predicate

Development builds provide `sail runtime symbolic-check` for a trusted Dive
installation that supports `dive check --strategy symbolic` and predicate
transfer. Set `DIVE` to its executable. Give each model a dedicated writable
session directory. Each invocation checks the module and configuration on disk.

```sh
sail runtime symbolic-check --dive "$DIVE" --session .sail/kvs \
  --module MCKVS.tla --config MCKVSSafetyMedium.cfg --time-limit 120
sail runtime symbolic-check --dive "$DIVE" --session .sail/kvs \
  --module MCKVS.tla --config EditedSafety.cfg --time-limit 120
```

The first completed search saves `sail-symbolic-predicate-v1.json` in the
session directory. A colleague can copy this one file into another session.
The receiving Dive binds its own modules and CFG and checks initial inclusion,
preservation by every current action, and implication of each configured
safety obligation. A successful row reports `proof: inductive-safety`. This
proof carries no state counts or diameter. It can still succeed after a
transition edit. Use `--type-invariant NAME` to select the layout invariant
when the source uses a different name.

When a candidate fails an obligation or its format is unsupported, Sail uses
the remaining wall budget for a fresh symbolic search. A completed search
atomically replaces the candidate. A violation carries a trace independently
validated by Dive. The previous candidate survives timeouts, interrupted
searches, refusals and violations. Concurrent successful writers may replace
each other's candidates. Every receiver checks whichever one it reads.
Keep the session directory outside the source tree or ignore it in version
control. Use a directory whose contents are managed by trusted local code.

Select `--strategy occupancy` for an installed Dive that admits anonymous status tables. Supply an orbit budget when a fresh exact search could exceed the default. The strategies keep separate candidate files in the same session. Existing invocations select the BDD strategy.

```sh
sail runtime symbolic-check --dive "$DIVE" --session .sail/commit --strategy occupancy \
  --module Edits.tla --config original.cfg --type-invariant TCTypeOK --max-orbits 34500000 --time-limit 415
sail runtime symbolic-check --dive "$DIVE" --session .sail/commit --strategy occupancy \
  --module Edits.tla --config stronger.cfg --type-invariant TCTypeOK --max-orbits 34500000 --time-limit 415
```

A complete search writes `sail-occupancy-predicate-v1.json`. Dive rebinds each revised source and verifies initial inclusion, preservation for every feasible population represented by the predicate, and every current safety obligation. A proof includes no counts or depth. When verification is inconclusive Sail can use its remaining deadline for a fresh labeled search. A concrete refutation requires native independent replay. The previous file survives unsuccessful searches. The new option uses the existing trusted local process and staging protocol.

The command emits Dive's JSON report with its original integer precision.
Exit 0 means a proof or completed safe search, 1 a validated counterexample,
2 unsupported input or a Sail usage/protocol error, 3 a checker error, and
4 a stop. Cancelling or expiring a check allows up to 400 ms for Dive to
reap its worker group before Sail sends a hard stop. A cgroup or equivalent
job controller is needed for cleanup if Sail itself receives SIGKILL. Sail's ordinary temporal receipts, daemon, and signed receipt
transport continue to use their own execution and evidence policy.

For repeatable comparisons on three- and five-key KVS, individual timings
and a cardinality change that invalidates the old predicate, see
[portable symbolic predicate measurements](../validation/temporal-reuse-s7.md#portable-symbolic-predicates).

### Recheck one named state invariant across edits

A compatible Dive installation can try the existing predicate, search for a
replacement if induction fails, and certify the result in the same process.
The module and configuration are bound once within the original wall budget.
Sail publishes a staged predicate only after admitting a successful named
proof. A malformed hypothesis, changed layout or changed transition can all
take this path. A verified violation preserves the previous session candidate.
Earlier installations use the separate verification and search calls.

Across eight edits on two public protocols, median unsigned end-to-end time
fell from 3.191 to 2.439 s for KVS and from 3.046 to 2.040 s for TCommit.
Publishing an accepted predicate can add work on a successful recheck.
On five-transaction KVS, the session also retained the newly certified
transition images after a guard edit. [All trial data](../validation/data/2026-11-22-named-predicate-repair.json).


Use a Dive binary with named-property support. Select the invariant from the
original configuration and supply the name of the finite layout in the module.

```sh
sail runtime symbolic-check --dive "$DIVE" --session .sail/lock \
  --module lockserv.tla --config lockserv.cfg --type-invariant TypeOK \
  --property-proof Mutex --time-limit 120
```

Run the same command after editing the module. The first successful proof
stores a candidate predicate. Each later invocation binds the current source,
checks initial inclusion, the named property and every current transition.
If the candidate fails an obligation, Sail asks Dive to discover a fresh
predicate within the remaining deadline. A faulty edit can return a concrete
trace independently validated on that revision. A successful row names the
proved property and reports the number of closure sweeps and transition
obligations. Deadlock and temporal checks require their own runs.

On the Recomp-Verify TwoPhaseCounter protocol grown to 64 resource managers,
adding an `init` guard to `SndPrepare` took 1.005 s with a saved candidate.
A fresh proof of that same edit took 1.157 s. Transfer avoided three closure
sweeps and checked all 323 transition obligations in the new process. Both
paths used about 109 MiB peak RSS. On the original 20-client lock server,
a request-guard edit took 0.354 s both ways. These are medians of three
end-to-end runs on a dedicated Linux c7i.4xlarge.
[Inputs, commands and individual results](https://github.com/Chelis-Lang/dive/blob/main/conformance/reports/perf/2026-11-21-named-property-proofs.json).

For expensive named-property transition checks, a compatible Dive installation
can reuse individually authenticated closure obligations in a new Sail process.
Set `DIVE_SYMBOLIC_PROOF_KEY` to a private key file managed separately from the
session. Dive rebinds the edited modules, discharges Init and the property, and
compares the lowered transition functions against the certified predicate.
The ordinary unkeyed mode rechecks each image. Five-transaction KVS saved
150 ms per restriction edit in a fresh process. TCommit's cheap images favored
unkeyed verification. [Setup and trust boundary](https://github.com/Chelis-Lang/dive/blob/main/docs/user/symbolic.md#reuse-certified-action-obligations-in-another-process) · [Measured edit histories](../validation/temporal-reuse-s7.md#cross-process-action-closure-in-multi-module-models).

Use `--strategy finite-induction` to save or recheck a complete finite
certificate from an installed Dive that supports this strategy. Sail keeps
the certificate in `sail-finite-induction-certificate-v1.json`. A successful
proof reports checked obligations and leaves exploration counts absent.
Dive checks the current source and finite configuration, including edited
invariants and any generated successors outside the state constraint.
With a matching local key, Dive can reuse bound Init and action obligations.
An inconclusive certificate falls back to a fresh check within the same
session deadline.

## Recheck source induction across edits

`runtime symbolic-check --strategy source-obligations` runs a trusted source
proof worker in a fresh process. The worker selects its named invariant and
strengthening. Sail checks that its report agrees with the current module
and CFG bytes, preserves the module's import directory and supervises one
process family under the requested wall-time limit.

```sh
sail runtime symbolic-check --strategy source-obligations \
  --dive "$PROOF_WORKER" --session .sail/source-proof \
  --module "$MODEL" --config "$CFG" --time-limit 180
```

The command invokes the worker with `MODULE CFG` and an optional
`--obligation-cache FILE`. The worker checks every current obligation or
accepts an authenticated receipt for the same mathematical obligation.
`--fresh` omits the cache argument. Each CFG has its own receipt file within
the session, so a job can return to a prior bound without rebuilding its
receipt. The worker publishes a proof of its named invariant after checking
current source binding and the assumptions. The result concerns the selected
invariant while the ordinary checker handles the other configured questions.

Dive's checkout provides `scripts/symbolic/source_obligations.py`. Set
`--backend bdd --layout NAME --candidate NAME --property NAME` for a named
finite induction proof. Set `--backend z3` for the documented finite
AsyncRaft log proof. A launcher fixes those flags, its Python interpreter and
the `--exporter` path, then forwards Sail's arguments. Set
`DIVE_SYMBOLIC_PROOF_KEY` in the worker's environment to a local 32-byte key
file with mode `0600` or `0400`. With no usable key, the worker discharges
its obligations from scratch.

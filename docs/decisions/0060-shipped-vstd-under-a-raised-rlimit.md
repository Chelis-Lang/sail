# 0060: Serve the shipped vstd proof under a raised `--rlimit`

- **Status:** accepted (owner decision, 2026-10-20)
- **Date:** 2026-10-20

## Context

`cargo verus verify -- <flags>` forwards its trailing flags to every package
with `package.metadata.verus.verify = true`, vstd included: cargo-verus's
`--fwd-verus-args-to` defaults to `all` for `verify`, `build` and `check`
(`roots` for `focus`), and appends the flags to the package's
` __VERUS_DRIVER_ARGS_FOR_<package>` variable
(`source/cargo-verus/src/subcommands.rs` at 7d82994). In `verify` mode vstd
is itself verified, so stock verifies vstd with the user's flags. The shipped
vstd proof (ADR 0048) binds that variable in its key, so a project that
forwards any flag misses the shipped proof and verifies vstd cold. Vest at its
corpus pin needs `-- --rlimit 100` under 7d82994 (stock too); its cold first
run is then 68.7 s stock and 78.8 s Sail (#252).

`--rlimit` is special. Verus turns it into the Z3 per-query limit
`(set-option :rlimit <rlimit × 3,000,000>)`, set immediately before each
`check-sat` and reset to `0` after it (`air/src/smt_verify.rs`). Z3's
resource counter grows deterministically with the work done for given
input, and the limit only decides where a query is cancelled. A query that
finished below one limit does the same work, with the same answer, under any
higher limit. Verus reads the counter (`:rlimit-count`) right before and right
after every `check-sat`. For a proof in which every query was answered below
its limit, a raised limit therefore cannot change any answer, resource count,
verdict or diagnostic.

That argument depends on two facts that must be checked, not assumed:

1. raising `--rlimit` changes no solver-input byte except the nonzero rlimit
   values; and
2. the solver does the same work under both limits, that is, the limit acts
   only as a cut-off.

`scripts/rlimit-subsumption.py` checks both on given inputs. It runs each
input K times as given and K times with `--rlimit N`, records every solver
process's input and output, and requires:
- inputs equal after replacing nonzero rlimit values;
- equal answer and `:rlimit-count` sequences;
- equal exit codes, stdout (wall-clock numbers blanked) and stderr.

It also reports the largest consumption of any single query against the
smallest limit set.

## Decision

1. **Exact keys stay exact.** A shipped unit is still found by its exact key,
   which binds every forwarded argument. Nothing else in the forwarded
   arguments is ever dropped from the key.
2. **One subsumed flag.** When the exact key misses, Sail computes a second
   key for the vstd invocation with one `--rlimit N` (or `--rlimit=N`) removed
   from vstd's forwarded arguments. It does so only when the arguments contain
   that flag exactly once and N is a finite decimal number. Every other
   argument and variable is keyed as before. A unit found under the second
   key is served only if all of the following hold:
   - the unit carries an rlimit witness (item 3), bound by the release index
     like every other member;
   - no query of the witness's default run was answered `unknown` or
     consumed its own limit;
   - the default and raised witness runs were byte-identical apart from
     rlimit values, with equal answers and resource counts;
   - N is at least the recorded run's rlimit.

   The served invocation's compiler reads and declared variables are checked
   against the recorded value of that one variable. If the restored dep-info
   declares it, its value is rewritten to the invocation's, as a compilation
   with the flag would record it. Otherwise it is a normal miss.
3. **The recorder proves the flag class for its own unit.** After recording,
   the release recorder verifies the same recipe project twice more with the
   staged runtime's own `cargo-verus` and Z3, with a fresh target and a
   recording solver wrapper: once as recorded and once with
   `-- --rlimit <witness>`. It applies the checks of
   `scripts/rlimit-subsumption.py` to every solver process. Only if they
   hold does it store `units/<key>/rlimit.json` with:
   - the recorded and witness rlimits, and the most common solver limits they
     produced;
   - the largest per-query consumption and that query's own limit;
   - the count of queries at or over their own limit (zero);
   - the number of solver processes and queries;
   - the digest of the normalized solver inputs.

   A function's explicit `#[verifier::rlimit]` keeps its own limit under both
   settings. A failed witness stores nothing, and the unit stays exact-key
   only.
4. **The served output is the recorded output.** The replayed stdout and
   stderr are those of the recorded default-rlimit run. That is correct
   because stock's output at the raised rlimit is identical (item 3 for this
   unit, the validation below for the corpus).
5. **Opt-out.** The second lookup is part of default mode, since it never
   changes a verdict or a diagnostic. It records `rlimit-subsumed` in the
   shipped trace.

## Validation

Required for acceptance (all met):
- `scripts/rlimit-subsumption.py --runs 3` with `--rlimit 100` over the whole
  fast tier, and vstd as a Cargo dependency, on native Linux, for stock Verus
  (comparing as multisets where stock is unordered: VI-0001, VI-0007, VI-0008,
  VI-0017, VI-0021) and for Sail's runtime (exactly);
- a macOS repeat;
- the staged release's own witness.

Results are recorded in [the rlimit record](../validation/rlimit-subsumption.md).
Linux so far:
- Sail's runtime: 147 of 147 inputs byte-identical apart from rlimit values.
- Stock: answers and resource counts identical on all 147. The 13 inputs that
  differ do so only in stock's hash-ordered declarations.
- No query reached its own limit.
- A staged release's witness passed, and stock and Sail were byte-identical
  with the bundle served under `-- --rlimit 100`.
- macOS (EC2 mac-m4.metal) repeats it: Sail's runtime is 147 of 147 byte-identical; the staged witness passed; stock and Sail matched byte for byte in all four cases. Stock's one resource-count difference, in HumanEval, is its own run-to-run variation and appears within each setting.

## Consequences

- Projects that forward `--rlimit N` with N ≥ 10 keep the shipped vstd proof
  when nothing else separates them from it. Vest at its corpus pin does not:
  it depends on vstd with `default-features = false` and its lock has
  `equivalent` 1.0.2, `indexmap` 2.14.1 and `unicode-ident` 1.0.24 (the shipped build has 1.0.3, 2.14.2 and 1.0.27), so its vstd closure differs from the
  shipped one and it misses for that reason (#238), as its `note[SAIL3138]`
  says. With those aligned, as the note suggests, Vest is served. Any other forwarded flag still misses it.
- Recording runs vstd twice more, about two minutes on a c7i.8xlarge.
- The argument relies on Z3's resource counting being the deterministic cut-off it is in Z3 4.16.0. A solver
  upgrade or another solver must repeat the validation. The recorder's
  witness repeats the per-unit check for every recording.
- Revisit if Verus starts using `--rlimit` for anything but the per-query
  limit, for example scaling another option. The byte check would then fail
  and the witness would not be stored.

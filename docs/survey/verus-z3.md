# How Verus talks to Z3

This is a source-confirmed account of how the pinned Verus release finds, starts,
drives and stops its SMT solver. It is the evidence base for the integration
decision (a Z3-compatible shim pointed to by `VERUS_Z3_PATH`) and for what the
session model, cache keys and query attribution must handle.

- **Release:** `release/0.2026.10.04.426d8b0`
- **Commit:** `426d8b01e7ffb910a36c15e1f870ff985bb61eef`
- **Pinned Z3:** 4.16.0 (`verus@426d8b0:source/cargo-verus-toolchains/src/external_deps.rs:2`)
- **Pinned cvc5:** 1.1.2 (`verus@426d8b0:source/cargo-verus-toolchains/src/external_deps.rs:3`)

Citations use the form `verus@426d8b0:source/<path>:<line>`. Every claim below
comes from reading the source at that commit. Claims that still need checking
against the running binary release are listed in
[Must be confirmed empirically](#must-be-confirmed-empirically).

## Summary

- Verus starts Z3 as `$VERUS_Z3_PATH -smt2 -in`, falling back to `z3` on `PATH`.
  The `verus` launcher fills `VERUS_Z3_PATH` with `<dir of verus>/z3` when that
  file exists. Z3 gets piped stdin and stdout, and inherits stderr, the
  environment and the working directory.
- Each **bucket** gets one long-lived Z3 process. A bucket is a module, or a
  single function marked `#[verifier::spinoff_prover]`. In addition, each
  nonlinear, bit-vector or profile-rerun query gets a fresh, short-lived Z3
  process, and so does every query under `-V spinoff-all`.
- The process starts lazily, at the first query. With `--num-threads N` (by
  default available parallelism minus one), up to N buckets run at once. Each
  can have its main process and one spinoff process alive together.
- The protocol is batched and synchronous. Verus writes a batch of commands,
  then `(echo "<<DONE>>")`. It reads stdout lines until it sees a line equal to
  `<<DONE>>`. There is no `print-success` and no `check-sat-assuming`.
- Each query runs inside `(push)` … `(pop)` on the process's long-lived
  assertion stack. It follows a fixed sequence:
  1. `get-info :all-statistics` (for `rlimit-count`);
  2. the query's declarations and `(assert (not …))`;
  3. `get-info :all-statistics` again;
  4. `get-info :version`;
  5. `(set-option :rlimit N)` and `(check-sat)`;
  6. `(set-option :rlimit 0)`, then `get-info :reason-unknown` or `get-model`;
  7. after `sat`, up to `--multiple-errors` further `check-sat`s, each after
     asserting the negation of the error label it already found;
  8. a final `get-info :all-statistics`.
- Solver effort is bounded only by Z3's `rlimit`, which is deterministic. By
  default it is 10 × 3,000,000 = 30,000,000 per `check-sat`. There is no
  wall-clock timeout and no kill. After 2 s, a "long running" notice is
  printed, and pending diagnostics are flushed early.
- Verus never sends comments or function names down the pipe. `;;` comments,
  module headers and `Function-Def <name>` markers go **only** to the
  `--log smt` files. `--log smt-transcript` writes the exact pipe bytes and
  responses, per process. Attribution therefore has to come from Verus's log
  files, or from correlating streams with them.
- Enabling `--log smt` **changes the whitespace of the bytes sent to Z3**,
  because pipe indentation only grows when a log file is attached. There are also
  `std::HashMap`/`HashSet` iterations (opaque-type projections, automatic
  trigger selection, pruning order) that can reorder or duplicate emitted
  AIR between runs. Both matter for content-addressed keys.
- Verus has no query cache. Its incremental features are coarse: cargo
  fingerprinting under `cargo verus`, `--verify-module`/`--verify-function`
  filters (which still send the full bucket context), and imported `.vir`
  libraries.

## 1. Locating the solver

### rust_verify (the actual consumer)

`SolverInfo::executable` resolves the solver path in one place
(`verus@426d8b0:source/air/src/smt_process.rs:23-31`):

- If `VERUS_Z3_PATH` is set, it is used **verbatim**
  (`verus@426d8b0:source/air/src/smt_process.rs:24-25`).
  `std::process::Command` then treats a bare name as a `PATH` lookup, and a path
  containing a separator as relative to the inherited working directory.
- Otherwise it uses the bare name `z3` (`z3.exe` on Windows), which is resolved
  through `PATH` (`verus@426d8b0:source/air/src/smt_process.rs:26-30`).
- For cvc5, the variable is `VERUS_CVC5_PATH` and the name is `cvc5`
  (`verus@426d8b0:source/air/src/smt_process.rs:16-19`).
- If spawning fails, Verus prints
  `error: could not execute z3 process (<err>)` and the help line
  `help: z3 needs to be in the PATH, or the environment variable VERUS_Z3_PATH must be set to the path of the z3 executable`,
  then exits with status **128**
  (`verus@426d8b0:source/air/src/smt_process.rs:121-136`).

`rust_verify` also records `VERUS_Z3_PATH` as an environment dependency in
cargo's dep-info (`verus@426d8b0:source/rust_verify/src/cargo_verus_dep_tracker.rs:18`,
installed at `:44-50`). Under `cargo verus`, changing the variable's value
therefore marks the crate dirty and forces re-verification. Note that
`VERUS_CVC5_PATH` is not tracked; `VERUS_SINGULAR_PATH` is.

### The `verus` launcher

The `verus` binary is a thin launcher that `exec`s `rust_verify` (on Unix;
`verus@426d8b0:source/verus/src/main.rs:17-25`). It handles the solver path as
follows:

- It computes `verus_root`, the directory of the current executable, after
  resolving one level of symlink
  (`verus@426d8b0:source/verus/src/main.rs:102-118`).
- If `VERUS_Z3_PATH` is already set, it leaves it alone
  (`verus@426d8b0:source/verus/src/main.rs:210-211`).
- Otherwise, if `<verus_root>/z3` exists, it sets `VERUS_Z3_PATH` to that path
  in `rust_verify`'s environment. The path is made absolute against the working
  directory if needed (`verus@426d8b0:source/verus/src/main.rs:212-223`).
  - The constant is `./z3`, or `.\z3.exe` on Windows
    (`verus@426d8b0:source/verus/src/main.rs:47`).
  - If the file does not exist, the variable stays unset, and `rust_verify`
    falls back to `PATH`.
- It does the same for `VERUS_CVC5_PATH` with `<verus_root>/cvc5`
  (`verus@426d8b0:source/verus/src/main.rs:226-236`).
- Z3 is **not** a required installation component. The completeness check
  lists only the Verus binaries, the libraries and `vstd.vir`
  (`verus@426d8b0:source/cargo-verus-toolchains/src/installed.rs:7-16`).
- With `--record`, the launcher runs `<z3> --version` once
  (`verus@426d8b0:source/verus/src/record.rs:16-17`). It parses the output
  against the pattern `Z3 version (\d+\.\d+\.\d+) - \d+ bit`
  (`verus@426d8b0:source/verus/src/record.rs:71`). A shim must answer
  `--version` like Z3 does.
- On Windows, the launcher puts itself in a kill-on-close job object, so solver
  children die with it (`verus@426d8b0:source/verus/src/main.rs:27-38`).

The other `VERUS_Z3_PATH` defaults in the repository are developer-only:
- `vargo`'s `.cargo/config.toml` sets `VERUS_Z3_PATH = "z3"` relative to the
  repository (`verus@426d8b0:source/.cargo/config.toml:16-17`);
- `rust_verify` panics if it runs under vargo without the variable
  (`verus@426d8b0:source/rust_verify/src/driver.rs:275-277`);
- `vstd_build` defaults the variable to `./z3`
  (`verus@426d8b0:source/vstd_build/src/main.rs:94-96`).

### cargo-verus

`cargo verus` sets `RUSTC_WRAPPER` to the `verus` binary next to `cargo-verus`
(`verus@426d8b0:source/cargo-verus/src/subcommands.rs:583-584`, `:714-722`).
It also sets `__VERUS_DRIVER_VIA_CARGO__=1`,
`CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true` and `RUSTC_BOOTSTRAP=1`
(`verus@426d8b0:source/cargo-verus/src/subcommands.rs:585-589`). It has no
solver logic of its own. Each per-crate `verus` invocation goes through the
launcher logic above with the inherited environment, so `VERUS_Z3_PATH` set
around `cargo verus` reaches every `rust_verify`.

## 2. Spawning the Z3 process

### Command, streams and environment

- **argv:** `<path> -smt2 -in`, exactly
  (`verus@426d8b0:source/air/src/smt_process.rs:105-107`).
  - No options are passed on the command line. Every option arrives as
    `set-option` over stdin.
  - cvc5 instead gets `--no-interactive --produce-models --quant-dsplit=none --no-cbqi --user-pat=strict`
    (`verus@426d8b0:source/air/src/smt_process.rs:108-114`).
- **stdin:** piped. A dedicated writer thread writes each batch with
  `write_all`, then a newline, then `(echo "<<DONE>>")` and a newline, then
  flushes (`verus@426d8b0:source/air/src/smt_process.rs:48-58`). If a write
  fails, for example because the solver died, the writer thread panics
  (`:57`).
- **stdout:** piped. A dedicated reader thread reads it line by line
  (`verus@426d8b0:source/air/src/smt_process.rs:63-100`; see
  [Reading responses](#reading-responses)).
- **stderr:** not configured, so it is **inherited**: solver stderr goes straight
  to the user's terminal (`verus@426d8b0:source/air/src/smt_process.rs:116-118`
  configure only stdin and stdout).
- **Environment and working directory:** inherited, because neither is
  overridden in `Command`. The working directory matters for `--profile`,
  whose trace file path is relative (see [Profiling](#profiling-trace-files)).

### One process per what

An `air::context::Context` owns at most one `SmtProcess`. The process is created
lazily, on the first `send_commands`
(`verus@426d8b0:source/air/src/context.rs:206-213`). Declarations and axioms
only accumulate in the emitter's pipe buffer until a query needs the solver.
Contexts are created in three places:

1. **One main context per bucket.** It is created at the start of
   `verify_bucket` (`verus@426d8b0:source/rust_verify/src/verifier.rs:1347-1357`).
   - It lives for the whole bucket, and runs every query of every function in
     that bucket in sequence.
   - Buckets are defined by `get_buckets`
     (`verus@426d8b0:source/rust_verify/src/buckets.rs:88-118`): one per
     module, plus one per function marked `spinoff_prover`.
   - Buckets are sorted by `BucketId` (`:111-113`).
2. **One spinoff context per spun-off query.** A query is spun off when its
   prover is `Nonlinear` (`by(nonlinear_arith)`) or `BitVector`
   (`by(bit_vector)`), when it is a profile rerun, or under `-V spinoff-all`
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1533-1538`).
   - Each spinoff is a brand-new `Context`, so a new Z3 process. It replays the
     full prelude and the bucket context accumulated so far, then runs that one
     query (`verus@426d8b0:source/rust_verify/src/verifier.rs:1563-1598`,
     `:1278-1315`).
   - The spinoff is dropped at the end of the loop iteration, which ends its
     process.
   - Bit-vector spinoffs are prelude-free, with no recommended options, no
     prelude and no bucket context
     (`verus@426d8b0:source/rust_verify/src/verifier.rs:1226-1243`, `:1309-1312`).
     They are also *single-check* (no `push`/`pop`;
     `verus@426d8b0:source/rust_verify/src/verifier.rs:1588-1591`).
   - Nonlinear spinoffs additionally get `(set-option :smt.arith.solver 6)`
     **after** the bucket context
     (`verus@426d8b0:source/rust_verify/src/verifier.rs:1255-1276`).
3. **No process at all for filtered-out functions.** With
   `--verify-function`, the queries of non-matching functions are skipped
   before anything is sent
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1069-1078`). A bucket
   with no included queries never starts Z3. Buckets with no included function
   are dropped entirely
   (`verus@426d8b0:source/rust_verify/src/user_filter.rs:146-163`).

### Concurrency

- `num_threads` defaults to available parallelism minus one, with a minimum of 1
  (`verus@426d8b0:source/rust_verify/src/config.rs:447-449`).
- It is then capped at the number of buckets
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:2111-2115`).
- With more than one thread, workers pull buckets from a shared `VecDeque`
  under a mutex (`verus@426d8b0:source/rust_verify/src/verifier.rs:2154-2172`).
  Which thread, and so which process start order, handles which bucket depends
  on scheduling. The *content* sent to each bucket's process does not, because
  each bucket has its own context and process.
- With one thread, buckets run in sorted order in the main thread
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:2518-2535`).
- **Upper bound on live Z3 processes:** `2 × num_threads`. Each worker holds its
  bucket's main process while a spinoff process for the current query runs,
  and a spinoff exists only during its own query.

### Reuse, restart and exit

- **No restart.** Processes are never restarted, pooled or reused across
  buckets.
- **Exit:** when a `Context` is dropped, `SmtProcess::drop` closes stdin (EOF)
  and calls `child.wait()`. It neither kills the child nor sends
  `(exit)`, and it ignores the exit status. It panics only if `wait` itself
  fails (`verus@426d8b0:source/air/src/smt_process.rs:235-243`). Z3 exits on
  stdin EOF.
- **Solver death:**
  - If the solver dies mid-session, a later write panics (`:57`).
  - A read at EOF returns empty lines; after more than two consecutive empty
    lines the reader panics with `Got too many empty lines!` (`:71-96`).
  - Either way Verus aborts with an internal error. It does not retry.

## 3. The version check

- **Command:** `(get-info :version)`. It is appended to the pipe buffer at the
  start of **every** `smt_check_assertion`, that is before every `check-sat`
  including the multiple-errors re-checks
  (`verus@426d8b0:source/air/src/smt_verify.rs:178`). It is sent as its own
  batch, together with anything still buffered (`:180-181`). It is not a
  separate startup handshake, and `z3 --version` is never run, except by
  `--record`.
- **Comparison:** the reply is scanned for a line starting with `(:version`.
  - The text after that prefix, minus the final character, is trimmed of
    spaces and `"`, then compared for exact string equality with
    `Z3_VERSION = "4.16.0"`
    (`verus@426d8b0:source/air/src/smt_verify.rs:143`, `:183-200`;
    `verus@426d8b0:source/rust_verify/src/verifier.rs:1360-1369`).
  - So the shim must answer `(:version "4.16.0")` byte-for-byte as Z3 would.
    Z3 replies `(:version "4.16.0")`; confirm this empirically.
- **Scope:** the expected version is set only on each bucket's **main** context
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:1360`). Spinoff contexts
  still send `(get-info :version)`, but never compare the answer.
- **On mismatch:** Verus reports the error diagnostic
  `expected z3 version 4.16.0, found <v>`
  (`verus@426d8b0:source/vir/src/messages.rs:189-203`). It then panics with
  `The verifier expects z3 version "4.16.0", found version "<v>"`
  (`verus@426d8b0:source/air/src/smt_verify.rs:190-198`).
- **Other output lines:** any other non-empty line in that batch's reply is
  `UnexpectedOutput`, which becomes a panic
  (`verus@426d8b0:source/air/src/smt_verify.rs:201-208`;
  `verus@426d8b0:source/rust_verify/src/verifier.rs:984-987`).
  Under `-V ignore-unexpected-smt` such lines become warnings instead.
- **Opting out:** `-V no-solver-version-check` sets
  `solver_version_check = false`
  (`verus@426d8b0:source/rust_verify/src/config.rs:409`, `:422-423`, `:829`),
  so the expected version is never set.

## 4. The interactive protocol

### Batches and the end-of-response marker

All traffic goes through `send_commands`/`send_commands_async`
(`verus@426d8b0:source/air/src/smt_process.rs:162-192`).

- **Batching:** a batch is everything the emitter has buffered since the last
  `take_pipe_data` (`verus@426d8b0:source/air/src/emitter.rs:48-52`).
- **End marker:** each batch is followed by `(echo "<<DONE>>")`. Verus considers
  a response complete when a stdout line, with `\n` and `\r` removed, equals
  `<<DONE>>`; for cvc5 it is the quoted `"<<DONE>>"`
  (`verus@426d8b0:source/air/src/smt_process.rs:43-44`, `:77-92`).
- **No `print-success`:** it is never enabled, so commands that succeed produce
  no output. The response to a batch is just the lines the solver printed
  before the echo.
- **Empty lines** are kept in the response vector. More than two in a row is
  fatal (`:78-97`).
- **Strictly request/response:** there is always at most one batch in flight per
  process. `send_commands_async` takes the reader handle, and `wait` gives it
  back (`:181-191`, `:212-218`).

### Emitted form

Commands are printed by `Emitter::log_node`
(`verus@426d8b0:source/air/src/emitter.rs:80-96`), using `sise` with
an 80-column break heuristic (`verus@426d8b0:source/air/src/printer.rs:603-660`).

- Each top-level command ends with `\n`.
- **Comments are written only to log files, never to the pipe**
  (`verus@426d8b0:source/air/src/emitter.rs:66-78`).
- **Indentation depends on logging.** The pipe text of a multi-line command is
  indented with `current_indent`. `indent`/`unindent` only change
  `current_indent` when a log file is attached
  (`verus@426d8b0:source/air/src/emitter.rs:54-64`, `:114-126`). So **the
  whitespace of the bytes sent to Z3 differs between runs with and without
  `--log smt`/`--log-all`.** The smt log's own copy also prefixes the
  first line with the indent (`:86-94`).

### Prelude: options and declarations

The first batch a process ever receives contains, in order:

1. **Recommended options.** For Z3 this applies to every context except
   prelude-free bit-vector spinoffs
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1228-1231`, expanded at
   `verus@426d8b0:source/air/src/context.rs:337-349`). They are sent in this
   order:
   ```
   (set-option :auto_config false)
   (set-option :smt.mbqi false)
   (set-option :smt.case_split 3)
   (set-option :smt.qi.eager_threshold 100.0)
   (set-option :smt.delay_units true)
   (set-option :smt.arith.solver 2)
   (set-option :smt.arith.nl false)
   (set-option :pi.enabled false)
   (set-option :rewriter.sort_disjunctions false)
   ```
   For cvc5 the equivalent is `(set-logic ALL)` plus
   `(set-option :incremental true)` (`:350-353`).
2. **Rlimit, but not on the pipe.** The default rlimit is applied next, via
   `set_rlimit`
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1232`). For Z3 this
   writes **nothing to the pipe**; the value is held in the context and sent
   per `check-sat` (`verus@426d8b0:source/air/src/context.rs:272-285`). For
   cvc5 it emits one `(set-option :rlimit-per N)` before the solver starts.
3. **User options.** Each `--smt-option K=V` is emitted in command-line order
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1233-1235`;
   `verus@426d8b0:source/rust_verify/src/config.rs:504`, `:725`).
   - The value is classified as bool, `u32`, `f64` (printed with a `.0` if it
     has no decimal point) or ASCII string
     (`verus@426d8b0:source/air/src/context.rs:377-407`).
   - `--smt-option rlimit=N` is special-cased: it replaces the per-check rlimit
     rather than being sent (`:367-369`).
   - **No random seed is ever set by default.** No `smt.random_seed` or
     `sat.random_seed` is emitted unless the user passes it with `--smt-option`.
     A search of `vir`, `air` and `rust_verify` finds no other `SetOption`
     generation.
4. `(set-option :produce-unsat-cores true)`, only with `-V axiom-usage-info`
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1236-1239`;
   `verus@426d8b0:source/air/src/context.rs:302-306`).
5. **Profiling trace options**, only with `--profile`, `--profile-all` or
   `-V capture-profiles`:
   `(set-option :trace true)` and `(set-option :trace_file_name <path>)`
   (`verus@426d8b0:source/air/src/context.rs:430-439`).
6. `(declare-sort %%Function%% 0)` and the Verus prelude declarations
   (`verus@426d8b0:source/air/src/context.rs:440-444`;
   `verus@426d8b0:source/rust_verify/src/verifier.rs:1240-1245`).
7. **The bucket context**, as declarations and labelled axioms (`assert`, with
   `:named` where named). The batches are titled `Fuel`, `Trait-Decls`,
   `Associated-Type-Decls`, `Datatypes`, `Trait-Bounds`,
   `Associated-Type-Impls`, `Opaque-Type-Constructors` and one
   `Function-Decl <fn>` per function
   (`verus@426d8b0:source/rust_verify/src/verifier.rs:1380-1441`).
   - After that, per-function context ops (`Function-Axioms`, `Function-Specs`,
     `Broadcast`, `Trait-Impl-Axiom`) are interleaved with queries in SCC order
     (`verus@426d8b0:source/rust_verify/src/verifier.rs:1487-1492`;
     `verus@426d8b0:source/rust_verify/src/commands.rs:461-491`).

Because the process starts lazily, all of this reaches Z3 as part of the batch
that also carries the first query's `(push)` and `(get-info :all-statistics)`.
That batch can be very large.

### Per-query sequence (default prover, Z3)

`smt_check_query` (`verus@426d8b0:source/air/src/smt_verify.rs:497-575`), then
`smt_check_assertion` (`:145-376`). Each row below is one batch, sent and
answered before the next.

| # | Batch contents (pipe) | Expected reply | Source |
|---|---|---|---|
| A | any buffered context, plus `(push)`, `(get-info :all-statistics)` | statistics s-expression | `smt_verify.rs:504-517`, `:378-421` |
| B | query-local `declare-const`/`declare-fun`, one `(declare-const %%location_label%%N Bool)` per labelled assertion, `(assert (not <labelled goal>))`, `(get-info :all-statistics)` | statistics | `:519-558` |
| C | `(get-info :version)` (plus, on re-checks, label-disabling asserts) | `(:version "4.16.0")` | `:178-213` |
| D | `(set-option :rlimit <N>)`, `(check-sat)` | `sat`, `unsat` or `unknown` | `:215-238` |
| E1 (after `unknown`) | `(set-option :rlimit 0)`, `(get-info :reason-unknown)` | `(:reason-unknown "…")` | `:270-333` |
| E2 (after `sat`, or `unknown` with reason "incomplete") | `(set-option :rlimit 0)`, `(get-model)` | model | `:423-495` |
| E3 (after `unsat` with `-V axiom-usage-info`) | `(set-option :rlimit 0)`, `(get-unsat-core)` | one line | `:340-359` |
| F | on `sat`: `(assert (not %%location_label%%K))` for the label found true; `(get-info :all-statistics)` | statistics | `:458`, `:563-572` |

Details of this sequence:

- **Goal labelling.** Each labelled assertion in the goal becomes
  `%%location_label%%i => goal_i`, with `i` counting from 0 within the query
  (`verus@426d8b0:source/air/src/smt_verify.rs:44-62`). Labelled axioms
  become `%%global_location_label%%n => axiom`, where `n` is a per-context
  counter (`:63-81`; `verus@426d8b0:source/air/src/def.rs:4-5`).
- **The negated goal.** The query is a single `(assert (not …))` of the
  labelled goal (`:547-548`). It is checked with plain `(check-sat)`. Verus
  never uses `check-sat-assuming`.
- **Statistics batches.** The three `get-info :all-statistics` batches (A, B, F)
  exist only to read `rlimit-count`, for the `--time` statistics. Every other
  statistic is ignored, and a missing `rlimit-count` counts as 0
  (`:378-421`). If the reply is not a flat key/value list, the query fails as
  `UnexpectedOutput`.
- **`set-option :rlimit 0`.** It is buffered right after the `check-sat` reply
  is read, and travels with the next batch (`:270-273`).
- **Reading the verdict.** Exactly one line of `sat`, `unsat` or `unknown` is
  expected. Any other line is fatal unless `-V ignore-unexpected-smt` is given
  (`:249-268`).
- **Reading the model.** Verus scans `get-model` for the first
  `%%location_label%%i` defined `true`. That label's error becomes the
  diagnostic. It then attaches extra notes from the first true
  `%%global_location_label%%` axiom, in sorted label order (`:443-478`). A
  reply containing `model is not available` gives an invalid result with no
  location (`:437-441`).
- **Multiple errors.**
  - After `sat`, Verus keeps querying the same assertion stack, up to
    `--multiple-errors` times; the default is 2
    (`verus@426d8b0:source/rust_verify/src/config.rs:726-729`). It does this by
    asserting the negation of each label already found and re-running C to E
    via `check_valid_again`
    (`verus@426d8b0:source/rust_verify/src/verifier.rs:961-981`;
    `verus@426d8b0:source/air/src/smt_verify.rs:145-176`).
  - Once the budget is spent, it switches to `only_check_earlier`. This
    asserts the negation of every label after the first one already found,
    to make sure the earliest error is the one reported (`:153-176`).
  - Re-checks are not followed by batch F.
- **End of query.** `finish_query` buffers `(pop)`, which goes out at the start
  of the next query's batch A
  (`verus@426d8b0:source/air/src/context.rs:549-557`;
  `verus@426d8b0:source/rust_verify/src/verifier.rs:999-1001`).
- **Single-check queries (bit-vector).** They have no `push`/`pop`.
  `sat` gives `Invalid(None, …)` with **no `get-model`**
  (`verus@426d8b0:source/air/src/smt_verify.rs:366-371`), and the context
  then refuses further queries (`verus@426d8b0:source/air/src/context.rs:549-551`).

### Interpreting `unknown`

After `(get-info :reason-unknown)` (`verus@426d8b0:source/air/src/smt_verify.rs:285-332`):

| Reply line | Meaning to Verus |
|---|---|
| `(:reason-unknown "canceled")` or `(:reason-unknown "max. resource limit exceeded")` | **Canceled**: reported as rlimit exceeded (`:122-129`) |
| `(:reason-unknown "unknown")` | **Canceled** (comment: "sometimes happens when rlimit is exceeded"; `:302-305`) |
| starts with `(:reason-unknown "(incomplete` or is `(:reason-unknown "smt tactic failed to show goal to be sat/unsat (incomplete quantifiers)")` | **Incomplete**: treated like `sat`, so `get-model` and error localisation follow (`:306-314`, `:330`) |
| anything else | `UnexpectedOutput`, which panics (unless `-V ignore-unexpected-smt`) |

A canceled query produces the diagnostic `<desc>: Resource limit (rlimit) exceeded`,
with `; consider rerunning with --profile for more details` appended unless
profiling is on. It counts as an error, and marks the function failed
(`verus@426d8b0:source/rust_verify/src/verifier.rs:862-882`). The process is
**kept and reused** for the bucket's next query: `finish_query` pops and returns
to `ReadyForQuery`.

### Retries and follow-up queries driven by results

These are new queries in the same main context, or new spinoff processes. None
of them is a resend of the same query.

- **Recommends checks.** When a `Function-Def` or `Spec-Termination` query fails
  and did not time out, Verus queues a `Function-Recommends` query, unless
  `--no-auto-recommends-check` is given. The same happens for functions marked
  `check_recommends`
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:1787-1795`, `:1822-1837`).
- **Expand errors.** With `--expand-errors`, a failure starts
  `Function-Expand-Errors` queries that focus on the failing assertion
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:1797-1804`;
  `verus@426d8b0:source/rust_verify/src/commands.rs:346-394`).
- **Profile rerun.** With `--profile`, a canceled query is re-run in a spinoff
  with tracing enabled
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:1756-1766`;
  `verus@426d8b0:source/rust_verify/src/commands.rs:404-422`).
- **Debugger.** `-V debug` can send `(eval …)` from the interactive debugger
  (`verus@426d8b0:source/air/src/context.rs:559-567`).

### Exit

Verus never sends `(exit)`. It closes stdin and waits; see
[Reuse, restart and exit](#reuse-restart-and-exit).

## 5. Resource limits and time

- **Mapping.** `--rlimit R` is a float, roughly seconds, with a default of
  `10` (`verus@426d8b0:source/rust_verify/src/config.rs:6`, `:704-723`). It
  becomes the Z3 rlimit `min(R × 3_000_000, u32::MAX)`
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:50`, `:1131-1141`), so
  `30000000` by default.
  - `--rlimit infinity` maps to `0`, which means unlimited (`:1136-1137`).
  - `--rlimit 0` is rejected (`config.rs:719-721`).
  - The cvc5 factor is 333,333 (`verifier.rs:51`).
- **Per-function override.** `#[verifier::rlimit(R)]` overrides the limit for
  that function's queries, then the default is restored
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:1605-1617`, `:1663-1665`).
- **How Z3 receives it.** The limit is sent as `(set-option :rlimit N)`
  immediately before each `check-sat`, and reset with
  `(set-option :rlimit 0)` immediately after
  (`verus@426d8b0:source/air/src/smt_verify.rs:215-218`, `:270-273`). So the
  limit applies per `check-sat`, never to declarations or follow-up commands.
- **Wall clock: none.** No `timeout` option is set, nothing is killed, and the
  reader blocks indefinitely
  (`verus@426d8b0:source/air/src/smt_process.rs:212-218`).
- **Long-running notices.** The only wall-clock mechanism is the long-running
  reporter: `wait_timeout` with a threshold of **2 seconds**
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:762-804`;
  `verus@426d8b0:source/air/src/smt_verify.rs:226-235`).
  - When the threshold passes, Verus flushes any deferred diagnostics for the
    function. These are then "reported as they are discovered (they may not
    be in source order)".
  - It also prints a `… has been running for N seconds` note, a
    `… finished in N seconds` note, or progress bars.
  - `--no-report-long-running` turns this off
    (`verus@426d8b0:source/rust_verify/src/config.rs:404`, `:836`).
  - So **the text and order of Verus diagnostics depend on wall-clock solver
    time**, even though verdicts do not.
- **Canceled verdicts.** `unknown` with `canceled` or
  `max. resource limit exceeded` (or `unknown`) is reported as an rlimit
  failure (§4). Z3 4.16.0's exact reason string at rlimit exhaustion must be
  confirmed empirically.

## 6. Query labelling and machine-readable output

### Log files

- **Flags.** `--log smt`, `--log smt-transcript`, `--log air`,
  `--log air-final` or `--log-all` enable logging; `--log-dir DIR` changes the
  directory (`verus@426d8b0:source/rust_verify/src/config.rs:332-366`,
  `:731-764`).
- **Directory.** The default is `.verus-log`
  (`verus@426d8b0:source/rust_verify/src/config.rs:23`). On first use in a run,
  every regular file already in it is **deleted**
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:627-650`, `:2991-3003`).
  Captures must be copied out after each run.
- **File names.** Logs are named `<prefix><suffix>`
  (`verus@426d8b0:source/rust_verify/src/verifier.rs:660-671`, `:1115-1129`).
  - The **prefix** is `BucketId::to_log_string()`: module segments joined with
    `__`, or `root` for the crate root. A `spinoff_prover` bucket uses the
    function path segments joined with `__`
    (`verus@426d8b0:source/rust_verify/src/buckets.rs:31-44`).
  - The **suffix** is `[_rerun][<fn-path>_<NN>][_expand]<ext>`, where:
    - `_rerun` marks a recommends query in a spinoff;
    - `<fn-path>_<NN>` appears only for spinoffs. `<fn-path>` comes from
      `path_to_string`: segments joined with `.` plus a trailing `.`, with a
      `krate!` prefix for non-local crates
      (`verus@426d8b0:source/vir/src/def.rs:394-400`). `NN` is a two-digit
      counter that restarts at 01 for each query op
      (`verifier.rs:1512`, `:1597`);
    - `_expand` marks expand-errors queries;
    - `<ext>` is `.smt2` (SMT), `.smt_transcript` (transcript), `.air` or
      `-final.air` (`verus@426d8b0:source/rust_verify/src/config.rs:31-34`).
  - **Collisions.** `File::create` truncates, and the counter restarts per op.
    So two ops of the same function that both spin off write the same name,
    and the second overwrites the first. This happens with, for example, a
    profile rerun of a nonlinear query, or a `Spec-Termination` and a
    `Function-Def` that both spin off. Spinoff logs are therefore not a
    lossless record.

### Markers in `.smt2` logs

These are written as `;;` comments, in the log only.

| Marker | Emitted at |
|---|---|
| `;; AIR prelude` | `verus@426d8b0:source/air/src/context.rs:441` |
| `;; Prelude`, `;; Fuel`, `;; Datatypes`, `;; Function-Decl <fn>`, … (one per command batch title) | `verus@426d8b0:source/rust_verify/src/verifier.rs:1019-1021`, `:1380-1438` |
| `;; MODULE '<friendly bucket name>'` (e.g. `module foo::bar`, `root module`, `module m, function f`) | `verifier.rs:1248-1249`; `buckets.rs:46-60` |
| `;; Function-Axioms <fn>`, `;; Function-Specs <fn>`, `;; Broadcast <fn>`, `;; Trait-Impl-Axiom` | `commands.rs:461-491`, `verifier.rs:1487-1490` |
| **Before each query:** `;; <OpKind> <fn>`, where the kind is `Function-Def`, `Spec-Termination`, `Function-Recommends`, `Function-Expand-Errors` or `Check-Api-Safety` (with a `-Profile-Rerun` suffix when applicable), followed by `;; <span as string>` (file:line:col of the obligation) | `verifier.rs:1089-1093`, `:1624-1637` (the op comment is passed in as `&op.to_air_comment()`); `commands.rs:461-491` |
| `;; <error note>` for each labelled assertion (e.g. `postcondition not satisfied`), before its `%%location_label%%` declaration | `verus@426d8b0:source/air/src/smt_verify.rs:538-539` |
| In spinoff logs: `;; <span>` and `;; query spun off because: nonlinear\|bitvector\|profile_rerun\|spinoff_all` | `verifier.rs:1304-1307` |

The function names are `fun_as_friendly_rust_name`, for example
`crate::foo::bar` or `<T as Tr>::m`. The span string is rustc's
`span_to_diagnostic_string` form, so it includes the file path. The ordinal of a
`Function-Def` comment among the `(check-sat)` lines in the same file identifies
the query. One function can own several queries, from recommends, multiple
errors and expand-errors.

### Transcript logs

`--log smt-transcript` records exactly what crosses the pipe, per process
(`verus@426d8b0:source/air/src/smt_process.rs:169-174`, `:201-209`). Each
batch is written as

```
;;;>>> QUERY
<batch bytes>;;;<<<
;;;>>> RESPONSE
<reply lines>
;;;<<<
```

The batch omits the trailing newline and the `(echo "<<DONE>>")` that the
writer thread appends, and the reply omits the `<<DONE>>` line. Transcripts
carry no `;;` attribution comments. Their file name, though, identifies the
bucket and spinoff, as above. Because `--log smt-transcript` alone does not
attach an smt log, it does not change the pipe indentation. Confirmed
empirically (ADR 0014): `--log smt-transcript`, `--log air`, `--output-json`
and `--time` leave every byte sent to Z3 unchanged; only `--log smt` changes
them.

### Machine-readable verdicts and timings

- **`--output-json`** prints one JSON object on stdout at the end
  (`verus@426d8b0:source/rust_verify/src/main.rs:519-555`). It contains:
  - `verification-results`, holding `encountered-error`,
    `encountered-vir-error`, `success` (only for whole-crate runs),
    `verified`, `errors` and `is-verifying-entire-crate`;
  - `func-details`, which maps each function name to its
    `obligation_proof_notes` and `failed_proof_notes`
    (`verus@426d8b0:source/rust_verify/src/verifier.rs:356-373`);
  - build info.
  - It has **no** per-obligation verdict list.
- **`--time` / `--time-expanded`** add `times-ms` to the JSON, or print a text
  table without `--output-json`
  (`verus@426d8b0:source/rust_verify/src/main.rs:153-517`). `times-ms.smt`
  holds:
  - totals for `smt-init`/`smt-run` and `rlimit-init`/`rlimit-run`;
  - per-module times;
  - under `smt-run-module-times[*].function-breakdown`, one entry **per
    function** with `function`, `mode:`, `time`, `time-micros`, `rlimit` and
    `success` (`main.rs:394-411`). The `mode:` key really is spelled with a
    trailing colon.
- **Where the numbers come from.**
  - Per-function SMT time is the wall-clock time spent in `check-sat` batches
    (D) only (`verus@426d8b0:source/air/src/smt_verify.rs:223-239`), summed
    over the function's queries (`verifier.rs:1601-1650`, `:1772-1786`).
  - `smt-init` is the time of batch C, which includes Z3 parsing whatever was
    buffered (`smt_verify.rs:179-182`).
  - The `rlimit` figures are differences of `rlimit-count` between batches A,
    B and F.
- **`success` is incomplete.** It means "not in `func_fails`". `func_fails`
  is filled on rlimit cancellation and on located failures
  (`verifier.rs:862-866`, `:903-913`), but **not** on the `Invalid(None, …)`
  path used by bit-vector queries (`:883-899`). So a function whose only
  failure is a bit-vector query can show `success: true` there.
- **`--record` and history recording.** The launcher forces
  `--output-json --time --time-expanded` when recording
  (`verus@426d8b0:source/verus/src/main.rs:290-310`).

## 7. Existing caching and incremental features

Verus has **no SMT query or result cache**. A search of `air` and
`rust_verify` finds none. What exists:

- **Selective verification.** `--verify-root`, `--verify-module`,
  `--verify-only-module` and `--verify-function` (substring or `*` wildcards;
  `verus@426d8b0:source/rust_verify/src/config.rs:459-480`, `:666-692`).
  - Non-matching buckets are dropped, and non-matching queries inside a kept
    bucket are skipped.
  - The **whole bucket context is still sent**, so the queries for the selected
    function are byte-identical to a full run
    (`verus@426d8b0:source/rust_verify/src/verifier.rs:2068-2090`, the design
    comment). This is the closest existing analogue to "re-run only what
    changed".
- **`#[verifier::spinoff_prover]`.** Puts a function in its own bucket, and so
  its own process with pruned context
  (`verus@426d8b0:source/rust_verify/src/buckets.rs:85-109`).
  `-V spinoff-all` runs every query in a fresh process
  (`verus@426d8b0:source/rust_verify/src/config.rs:410`, `:425`).
- **Cargo-level incrementality** under `cargo verus`.
  - Crates are fingerprinted by cargo, with checksum freshness enabled, and
    Verus adds env and file dep-info, including `VERUS_Z3_PATH`
    (`verus@426d8b0:source/cargo-verus/src/subcommands.rs:585-589`;
    `verus@426d8b0:source/rust_verify/src/cargo_verus_dep_tracker.rs:13-57`).
  - An unchanged crate is not re-verified at all. A changed crate is
    re-verified in full.
- **Library import/export.** `--export`/`--import` `.vir` metadata, as with
  `vstd.vir`, means dependencies are not re-verified
  (`verus@426d8b0:source/rust_verify/src/config.rs:457-458`).

## 8. Nondeterminism in what Verus sends

The cache key is the exact byte stream. These are the sources of variation found
in the source.

1. **`std::collections::HashMap`/`HashSet` iteration order.** These use
   `RandomState`, which is seeded per process. Iterating them can change what is
   emitted between two runs on identical input. This was found by reading the
   code and has not yet been seen to happen.
   - **Confirmed: opaque return-type projections.** `ret_exp_projection_map.keys()`
     is a `HashMap`. Iterating it decides the order of the extra
     `opaque_ty_additional_stmts` assumptions for opaque return types with two or
     more type-equality bounds
     (`verus@426d8b0:source/vir/src/sst_to_air.rs:3423-3446`, with
     `HashMap` imported from `std` at `:42`).
   - **Likely: automatic trigger selection.** The selection starts from
     `state.remaining_vars.iter().next()`, a `HashSet`
     (`verus@426d8b0:source/vir/src/triggers_auto.rs:621`, `:718-719`). The set
     of distinct triggers it finds is sorted afterwards (`:802-805`), but no
     step removes duplicates. So the *number of identical `:pattern`
     annotations* can depend on which variable is picked first. The
     10,000-step trigger-search budget (`check_timeout`, `:716`) also depends on
     how much is pruned, so near that limit a function could fail trigger
     selection in one run and not another.
   - **Likely: pruning reachability.** `reached_types`, `reached_functions` and
     `reached_assoc_type_decls` are `HashSet`s
     (`verus@426d8b0:source/vir/src/prune.rs:92`, iterated around `:603`,
     `:659-662`, `:697`). Their iteration order sets the order in which function
     bodies are visited. That order in turn sets the order of `HasResolved`
     types collected for the resolve axioms (`prune.rs:533`), and those axioms
     are emitted in that order
     (`verus@426d8b0:source/vir/src/resolve_axioms.rs:118-122`, `:331`).
   - **Checked and order-independent:** call-graph SCC order, buckets
     (sorted), datatypes, fuel, trait axioms, the closure, lambda and `:qid`
     counters, `var_to_const`, which uses `IndexMap`, and model and axiom-info
     handling (`air/src/smt_verify.rs:466` sorts). No timestamps, process ids,
     random numbers or source paths appear in emitted SMT. String literals are
     encoded as SHA-512 hashes, which are deterministic
     (`verus@426d8b0:source/vir/src/sst_to_air.rs:728-758`).
2. **Logging changes whitespace.** The pipe bytes depend on whether `--log smt`
   or `--log-all` is on (§4, emitted form). This is deterministic for a given
   flag set, but a corpus captured *with* `--log smt` has different bytes from
   a production run *without* it.
3. **Profiling paths.** `--profile*` injects `(set-option :trace true)` and
   `(set-option :trace_file_name ".verus-solver-log/<bucket>[...].profile")`
   (`verus@426d8b0:source/air/src/context.rs:430-439`;
   `verus@426d8b0:source/rust_verify/src/verifier.rs:1330-1346`, `:1540-1561`). This is
   deterministic but path-bearing.
4. **Scheduling.** Thread scheduling decides which bucket starts when, and so the
   order and timing of process spawns. It does not decide the content of any one
   process's stream, because each bucket has its own context, `Ctx` and name
   context.
   - Per-bucket `GlobalCtx` copies reset `qid_map` and `chosen_triggers`
     (`verus@426d8b0:source/vir/src/context.rs:746-776`).
   - The remaining shared mutable state is diagnostic only.
   - Single-threaded runs pass the `GlobalCtx` through buckets sequentially
     (`verifier.rs:2527-2533`).
5. **Response bytes that are not deterministic.** Z3's `get-info :all-statistics`
   output includes time and memory figures. Verus reads only `rlimit-count`, but
   a cache that stores reply bytes verbatim (charter rule 3) stores those too.
   - `rlimit-count` is cumulative per process, so its absolute value depends
     on the whole history of that process. Verus only uses differences.
6. **Order-dependent output, not content.** Diagnostic ordering depends on wall
   clock (the 2 s long-running flush, §5) and, with several threads, on which
   bucket reports first (`verifier.rs:2223-2300`). Verdicts do not. The
   conformance harness must compare diagnostics as a multiset per function, or
   pass `--no-report-long-running` and `--num-threads 1`, rather than as an
   ordered byte stream.

## 9. What a Z3-compatible shim must do

Pointed to by `VERUS_Z3_PATH`, the shim replaces one Z3 process. From the above,
it must:

- **Accept argv `-smt2 -in`,** and read SMT-LIB from stdin until EOF. Also
  answer `--version` exactly like Z3 (`Z3 version 4.16.0 - 64 bit`), because
  `--record` runs it. Pass through any other argv unchanged to the real Z3 on
  fallback.
- **Honour the batch protocol.**
  - Process commands in order, and print exactly what Z3 prints for each,
    including `<<DONE>>` for `(echo "<<DONE>>")`.
  - Flush stdout at least at every echo, or Verus blocks forever.
  - Print nothing extra: no `success` lines, banners or blank lines. Unexpected
    lines panic Verus, and more than two blank lines in a row panic it too.
  - Batches arrive delimited by `(echo "<<DONE>>")`, which a shim can use as a
    natural point to cut a session. It must not *require* the delimiter,
    because other clients or `-V debug` may send other shapes.
- **Reproduce reply bytes exactly** for:
  - `check-sat` (`sat`/`unsat`/`unknown`);
  - `get-info :version` (exactly `(:version "4.16.0")`);
  - `get-info :reason-unknown`;
  - `get-model`: label values decide *which* error is reported, and
    `%%global_location_label%%` values decide the extra notes, so a different
    but valid model can change the diagnostics;
  - `get-unsat-core`;
  - `eval`.
- **Keep `get-info :all-statistics` well formed.** It must be a flat
  `(:key value …)` list. Its `rlimit-count` is cumulative per process, and
  Verus subtracts successive values (`smt_verify.rs:509-572`). Replayed or
  pooled answers must keep `rlimit-count` monotonic and consistent with the
  rlimit consumed, or `--time` rlimit figures change. Verdicts do not depend on
  it. Values that are not atoms are fatal.
- **Model history, not just the assertion stack.**
  - Queries in one process run after earlier pushes, pops and check-sats.
  - Z3's answer under a resource limit, and the model it returns, may depend
    on that history: on internal state such as term ids, learned lemmas
    surviving pops, and random-generator state. A fresh-process replay of only
    the current stack may differ from it.
  - Until this is measured, a default-mode key must cover the *entire command
    prefix of the process*, not only the logical stack at `check-sat`. This
    sharpens charter rule 2.
- **Treat `set-option :rlimit` as per-check state.** It is set just before each
  `check-sat` and reset to `0` just after. The key must include the value in
  force at `check-sat`. A pool that answers from a different process must
  re-apply it.
- **Handle `trace`/`trace_file_name` locally.** Under `--profile*`, Z3 must
  write the trace file at that path, relative to the shim's inherited working
  directory. Verus asserts the file exists and parses it
  (`verifier.rs:1706`). A shim should forward such sessions directly to
  the pinned Z3 in place, rather than to a remote daemon with a different
  working directory.
- **stderr.** It is inherited by both Z3 and the shim, and goes to the user's
  terminal. The shim must not write to stderr in default mode, or Verus's
  observable output changes.
- **Exit codes.** Verus ignores the solver's exit status. It only `wait`s
  after closing stdin. The shim should still exit 0 on EOF, as Z3 does.
- **Signals and cancellation.** Verus never kills the solver. On Ctrl-C, the
  terminal delivers SIGINT to the whole foreground process group, so Z3 or the
  shim gets it too. If Verus itself dies, the shim sees EOF on stdin, or EPIPE
  on stdout.
  - A shim must handle both by aborting any in-flight work and exiting
    promptly. A pool-backed shim must also cancel its daemon-side query, so a
    long `check-sat` does not keep burning CPU.
  - On Windows, the job object kills children with the launcher.
- **Concurrent processes.** Expect up to `2 × num_threads` shim processes at
  once, each an independent session. The very first batch of each can be
  megabytes long: the prelude plus the bucket context.
- **Fallback.** This follows from rule 10 and the behaviour above. If the
  daemon is unavailable at any point, the shim must be able to start the real
  Z3 and **replay the session prefix it has already accepted**, without having
  emitted any reply it cannot stand behind. Alternatively, it must decide on
  fallback before acknowledging anything.

## Implications for sail

- **The integration point is confirmed.** `VERUS_Z3_PATH` is honoured verbatim
  by `rust_verify`, and is preserved by both the `verus` launcher and
  `cargo verus`. Setting it around `cargo verus` reaches every crate, and
  changing it invalidates cargo's verification fingerprint, so switching to
  sail and back cleanly forces re-verification. No change to Verus is needed.
- **Session granularity.** One shim process means one bucket, or one spun-off
  query. Within a bucket, queries are strictly sequential and share a growing
  prefix: prelude, bucket context, then earlier context ops. Spinoffs replay
  that same prefix in a fresh process, so prefix sharing between processes is
  common and cheap to detect.
- **Keys must be byte-exact, and logging-aware.** Run the capture and production
  configurations with the same `--log` flags, or record the flag set in the
  key. Normalizing whitespace would remove the `--log smt` difference, but that
  is normalization: it needs an ADR and oracle validation (charter rule 5).
- **Expect some run-to-run misses.** Because of the `HashMap`/`HashSet` orderings in §8, two runs on the same input can send different bytes. Content addressing keeps that safe: it costs hit rate, never correctness. Measure how often it happens before considering any normalization.
- **Key the full process history,** until experiments show that Z3 4.16.0's
  answers after `pop` depend only on the current stack.
- **Store every follow-up reply** against the state it was asked in, as charter
  rule 3 requires: `get-model`, `reason-unknown`, `version`, unsat cores and the
  re-checks after label-disabling asserts. Re-synthesize `all-statistics`, or
  pass it through, rather than replaying stale time and memory figures. Decide
  this explicitly in an ADR.
- **Attribution needs Verus's own logs.** The pipe carries no function names.
  `sail-verus` should run Verus with `--log smt-transcript`, which leaves the
  pipe bytes unchanged, and use the transcript file names to map processes to
  buckets. Mapping a check-sat to a function needs the `;; Function-Def <fn>`
  markers, which exist only in `--log smt` (which changes pipe whitespace), or
  an equivalent derived by sail from the bucket structure. Expect lossy spinoff
  logs (file name collisions).
- **Conformance must neutralise timing-dependent output.** Use
  `--no-report-long-running`, or compare diagnostics per function as sets,
  because the 2 s flush reorders diagnostics and adds timing notes.
- **No wall-clock timeouts in Verus.** Any timeout sail adds is sail's own, and
  must never be cached as a solver answer (charter rule 4).

## Must be confirmed empirically

1. **Exact first-batch bytes and per-query batch shapes** on the binary release,
   with a recording shim or `--log smt-transcript`. In particular: that the
   order is A to F, that `(pop)` is coalesced into the next batch, and that
   `set-option :rlimit 0` rides with the follow-up.
2. **Whitespace difference.** That pipe bytes with `--log smt` differ only in
   whitespace from bytes without it.
3. **Version reply.** That Z3 4.16.0 replies `(:version "4.16.0")` to
   `get-info :version`, and prints `Z3 version 4.16.0 - 64 bit` for `--version`.
4. **Rlimit exhaustion string.** Z3 4.16.0's `reason-unknown` at rlimit
   exhaustion: `canceled`, `max. resource limit exceeded` or `unknown`?
5. **Rlimit semantics.** Whether `(set-option :rlimit N)` is relative to the
   current `rlimit-count`, as Verus's per-query accounting assumes, or absolute.
6. **History dependence.** Whether Z3 4.16.0 gives identical verdicts, models
   and `rlimit-count` deltas for a query run in a fresh process with only its
   current stack, versus at its position in a bucket session. This is critical
   for key design.
7. **Model stability.** Whether `get-model` output is byte-stable across runs of
   the same session.
8. **Process counts.** The observed number of concurrent Z3 processes for a
   corpus crate at the default `--num-threads`, and the spinoff process count.
9. **Lazy start.** That `--verify-function` on a bucket with no included
   function starts no Z3 process.
10. **Log naming and collisions.** The spinoff log file names
    (`<bucket><fnpath>_NN.smt2`) and the overwrite collisions.
11. **Cargo freshness.** That `cargo verus` re-verifies when `VERUS_Z3_PATH`
    changes, and whether the launcher-injected default value causes spurious
    rebuilds. The dep-info records the launcher-set path, while cargo's own
    environment may not have the variable.
12. **Statistics.** That `get-info :all-statistics` always contains
    `rlimit-count` after the prelude, and is printed as one flat list spanning
    several lines.
13. **HashMap ordering.** Run-to-run byte equality of transcripts for the same
    input. Run the corpus twice and diff the transcripts, to catch the
    `HashMap`-order sources listed in §8.

Items 4 to 7 have since been measured on the pinned release, on the
captured examples and vstd corpus (`solver-behavior.md`):
- item 4: rlimit exhaustion reads `canceled`;
- item 5: `:rlimit` is relative;
- item 6: answers are stack-determined, but models and `rlimit` deltas
  depend on history;
- item 7: models are byte-stable across replays of the same session.

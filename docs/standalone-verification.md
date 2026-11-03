# Verify an existing Verus file

Standalone verification is available in the published registry prerelease
`0.2.0-rc.1` and the stable `0.2.0` candidate, with native Linux/macOS
qualification. It is not in published 0.1.0.
See the [qualification record](validation/standalone-verification.md).

Use your existing Verus source without creating a Cargo package:

```sh
sail verify proof.rs
sail verify proof.rs --output json
sail verify proof.rs -- --crate-type lib --rlimit 30
sail watch proof.rs
```

The first positional `.rs` file selects standalone verification, including
inside a Cargo workspace. Other `sail verify` invocations retain Cargo mode.
Arguments after `--` go to Sail's pinned Verus. Sources and their relative
modules/includes retain their original paths and working directory. The
runtime supplies its own verified vstd. No Cargo manifest is generated.

A complete successful verify-only call can retain a proof receipt. An
unchanged call or return to a retained successful revision rechecks its
inputs and reuses that proof. JSON records the original `establishing_run`,
`reused` and actual current invocation counters. History retains up to 16
revisions within 1 GiB, keeping the first successful revision pinned.
Changed files, includes, namespaces, environment or runtime invalidate reuse.
The candidate's corrected runtime also covers non-UTF-8 `include_bytes!` inputs:
unchanged proofs can be Current and reused, while a same-size edit with restored
mtime still requires verification. Emptying a binary used by the proof preserves
the fresh compiler's failure. See [qualification](validation/binary-compiler-inputs.md).
External source reads also bind every intervening discovery directory;
unrelated changes in those ancestor directories can conservatively prevent reuse.
Missing compiler observations cause fresh verification. `SAIL_MANAGED_FORCE=1`
forces execution. Fresh failing proofs never establish a complete receipt.

Watch verifies on startup and after saves, cancelling superseded jobs.
The source and discovered modules/includes must be beneath the working
directory. Run watch from their common source directory. An external source or
an expansion outside that boundary is refused; CLI and MCP verification keep
their broader input coverage.
Standalone watch is verify-only: compilation and explicit output-writing flags
are refused to avoid reruns caused by its own outputs; use `sail verify`
for those requests. It
uses the same durable job service as background verification:

```sh
sail verify proof.rs --background --output json
sail jobs list
```

An MCP client calls `verify` or `start_verification` with:

```json
{"project_dir":"/path/to/proofs","source_file":"proof.rs"}
```

Existing `module`, `function` and `verus_args` selectors apply. Partial
checks stay partial and run freshly. File verification requests reject Cargo
selectors and solver experiments. Standalone verification currently
uses pinned Z3; an explicit cvc5 request is refused rather than attributed
to the wrong solver. Compilation, custom extern/search
paths, explicit output side effects and unsupported reuse configurations
run fresh Verus with the solver cache. Such a successful execution is not
promoted to reusable evidence unless its complete inputs are established.

Validate a repair to one function, then the complete file:

```sh
sail repair-check proof.rs --function proof::lemma --engine interactive --output json
```

MCP's `start_repair_validation` accepts `source_file`, `function` and
`engine: "interactive"` instead of Cargo package/kind/target selectors. It
returns a durable job. The selected function's focused result remains
provisional; acceptance requires independent successful Current verification
of the complete file and the engine's normal late compiler seal. A failed
full phase, missing selector, engine fault or stale source never accepts a
repair. No Cargo project is generated. See [repair](proof-repair.md).

`SAIL3150` identifies standalone setup/protocol errors. A `SAIL3151` JSON
note means input coverage could not establish a reusable Current proof;
the fresh verifier's verdict and diagnostics remain available.

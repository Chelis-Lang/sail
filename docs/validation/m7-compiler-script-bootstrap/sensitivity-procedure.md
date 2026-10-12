# Ordinary-script bootstrap sensitivity procedure

The [thin native caller](sensitivity-run.py) implements this procedure without
another process-ownership implementation. Nine [pure controls](test-sensitivity-run.py)
passed on the host; no native compilation or sensitivity phase has run. The root
coordinator owns every native command. Run it after the corrected source passes its complete affected checks;
the existing failed source scopes retain their FAIL status. The planned outcome
is a fresh positive/negative/restored-positive component scope, not a product or
benchmark qualification.

Use the existing [`OwnedPhaseRunner`](../../../../scripts/local-combined-gates.py)
with its `Closure`, `Groups`, `group_closure` and `closed_groups` rules. Its pinned
helper SHA is `f5dc0e498629e1945246c4ae7919d8b81f497783705a69139021c4575f08a5bc`.
A thin caller supplies the source/protected/container-birth check and retains
its results; it does not replace that lifecycle. Register the existing interrupt
handler for SIGINT, SIGTERM and SIGHUP. A normal exit, including expected 101,
requires a complete known-group audit before the next phase. Timeout,
interruption, unknown birth, partial census or live sibling makes uncertainty
sticky. A leader reap alone never authorizes a source restore or another actor.

Freeze the actual integrated corrected Git revision, its Git archive/inventory,
Cargo.lock, five candidate source hashes and the existing protected engine,
product and tested-image inventories. Extract a NEW private source from that
archive; do not alter the preceding failed source. Preserve ordinary Git modes,
links and all member hashes. The target is the root-owned common target, with no
other writer. Record the current container ID/image/birth and PID1 birth before
execution; repeat source/protected/birth checks around each phase. Set
`CARGO_INCREMENTAL=0`, retain pinned Rust 1.99.0 and use the helper's two build
jobs. Remove unexpected engine/trace/test selectors before freezing the exact
private environment; do not publish environment values.

The commands below are argv for the lifecycle helper, not unsupervised shell
commands. Each compile has an outer 600-second ownership cap; each libtest run
has an outer 180-second ownership cap. The real-Cargo fixture's GNU timeout
60-second TERM/two-second KILL policy remains unchanged.

```sh
cargo test --locked -p sail-verus --lib --no-run --message-format=json
```

Bound the JSON build log at 32 MiB. Select exactly one emitted
`compiler-artifact` with `profile.test=true`, target name `sail_verus` and its
explicit executable path. Refuse absent/ambiguous output. Copy and hash that
ELF before any later compile using `retained_images` under a phase-specific
record directory; check its errors list is empty. Record the source/lock,
compiler command/log seal, ELF bytes/mode/SHA and the actual copied executable
argv. Execute that immutable copy:

```sh
"$SAIL_BOOTSTRAP_TEST_ELF" managed::build_inputs::family::bootstrap::tests::bootstrap_actual_cargo_can_publish_ordinary_contract_without_current_proof --exact --test-threads=1
```

1. Positive source: require exit 0, that exact test case, and exactly one passed,
   zero failed. Record complete owned closure before mutation.
2. After `require_idle()` and independent source/protected/birth checks, apply
   the immutable [disable-only-emission patch](disable-publication.patch).
   The caller recomputes its complete unified diff from exactly one emission
   replacement and requires byte equality with that frozen patch before writing
   the one changed file. Its SHA is bound in
   [source.json](source.json). Verify the single changed file byte-for-byte
   against the patched frozen original; every other source member is unchanged.
   Compile, retain the new actual ELF under a different phase directory and run
   the SAME exact positive test with unchanged assertions. Require exit 101,
   exactly zero passed/one failed, that exact failed case and the unchanged
   message `independent ordinary-script contract must escape absent-generation bootstrap`.
   A compile failure, timeout, other panic or closure uncertainty is not the
   required negative outcome. Do not require a generic `assertion failed`
   string: libtest preserves this assertion's explicit message.
3. Only after the negative actor is closed and `require_idle()` succeeds,
   restore the original single file from the frozen Git archive. Reverify the
   entire source inventory, lock, modes and protected/container bindings.
   Compile, retain a third actual ELF and execute the SAME positive once;
   require exit 0 and exactly one passed/zero failed.

Do not restore source in an unconditional `finally` after interrupted or
uncertain ownership. Retain the primary failure and stop until the coordinator
has an independent holder audit. Retain separate logs, tested images and closure
records for all three phases, and an aggregate terminal that cannot splice old
outcomes into the new scope. Full affected tests, all-target strict checks and
fresh combined gate/solver remain separate mandatory checks. The unchanged
original repair fixture and original two-warmup real benchmark must measure the
actual integrated product before any correctness or speed claim.

The root stages a NEW outside-target scope with `source.tar`,
`source-inventory.json`, `protected.json`, `disable-publication.patch`, the exact
pinned `local-combined-gates.py`, `config.json` and `grant.json`. Config and grant
bind the full revision, NEW absent source path, existing target path, PID1 birth,
externally inspected container ID/image/birth, and SHA-256 of the archive,
inventory, protected inventory, patch and this controller. The grant has
`status: GRANTED`, `scope: ordinary-script-bootstrap-sensitivity` and the actual
root-owned preflight `holder_audit` with empty `survivors` and `unavailable`.
Protected files include the current engine, products and prior tested images.
The caller verifies/extracts the entire source, creates only its external target
symlink and checks all bindings around every compiler/test phase. Host transport
must retain its own before/after Docker identity checks; native PID1 checks do
not independently obtain Docker IDs. Source-derived helper code is imported
only after exact archive extraction verification.

```sh
python3 sensitivity-run.py --root "$SAIL_BOOTSTRAP_SCOPE"
```

No source mutation occurs in exceptional finalization. A failed negative scope
may deliberately leave the exact negative source in place, marked unrestored;
an independent owner audit is required before its disposal/restoration. The
existing `cancel_native` route can use this caller's saved birth-bound
`owner.json` for cooperative interruption. `primary.json` precedes final
source/protected checks; `result.json` records any closure failure. The three
retained images have a combined 512 MiB cap, and capture reads have a 32 MiB cap.
The optional certificate's production source remains unchanged by this caller
commit.

# Original Kani driver development qualification

Status: bounded native Linux tool/stage/runner qualification and independent
reviews pass; unselected. Tracks #712 G, ADR 0152/0153/0154.

Environment: native x86-64 c7i.4xlarge, us-east-2, Ubuntu 24.04, Rust 1.99.0
for Sail. Original Kani 0.68.0 / CBMC 6.11.0 and six pinned Rust component
archives supply the actual tools. Rustup bookkeeping grants no tool identity.

| Source / gate | Actual result |
|---|---|
| Tools6e85f920 | Full workspace 1,445/59 declared skips; 15 actual original-tool controls pass |
| Stage88e8d9e6 | Full workspace 1,447/59 skips; eight library and 19 stage controls pass |
| Fresh Tools/Stage review | Original archive reconstruction and 86 process controls pass, including native-byte routes, file types, permissions, limits and same-handle rechecks |
| Runner9f379750 | All-target Clippy, all 13 library tests and release inspectors pass |
| Runner full workspace | 1,452 passed/59 skips; fmt/Clippy/doctests/rustdoc/layering/deny/generated page pass |
| Actual original driver and Rust runner | Three pinned arrayvec harnesses yield bounded success, counterexample and insufficient bound; sources unchanged |
| Runner refusals | Zero unwind/time, excessive time, existing destination and absent harness refuse without observations |
| Fresh runner review | Original 13 tests, four instrumented process tests, actual harnesses, explicit unwind override, started-driver cancellation, 1 ms timeout and changed-route refusals pass |

The explicit override changes the original insufficient-bound harness from
unwind 2 to bounded success at unwind 6. Original streams remain retained.
Process controls cover exact binary streams/exits, signals, size/type failures,
live cancellation and inherited children/grandchildren after timeout/leader exit.
Instrumentation is confined to a separate reviewer copy, with its exact delta
and binary retained; it is not labelled untouched-source execution.

Every indexed payload and link record is independently lead-rehashed before
external retention:

| Complete evidence | Payloads | Archive SHA-256 |
|---|---:|---|
| Original tools/setup/failed attempts | 7,238 files | b4650ca77ab5891e98fc450385f9eceaa88afc326b3135b07dfacb796dbc0b5a |
| Stage originals | 357 files, 3 links | f89c56b10c1f4a3cbbd1dd1c6f37bd472f230bc7754c07c42dfbc255546e753c |
| Fresh Tools/Stage review | 9,144 files, 3 links | 3d529b5cac9ceb04e25d6bdb204b91d8cc9a15d517fbef1a7db3b5857c406bba |
| Driver/runner originals | 472 files, 1 link | 127a9371ce89d50ce93b6362f6d8894b725ffefc249a61504bbed5878d45deb0 |
| Fresh runner review | 548 files, 198 directories, 1 link | 73d76a397be6a108fa49728acca544c9c3e17486548c38255c4d55a073027aea |

Runner index: a46619a5afb7c405c37d579ca844dbdc268460a4dfb0cb1bf07c569822eb8757.
Fresh review index: d8664db777111435c1d458a0d1b054444de5a704cea8586eec06bcd4ca401854.
Its report: b2531bd63a031e1e7de02af788dbd2753d4dbe87e77a14ce515a2ea024178f42.
The audit independently rehashes all 7,137 live original Kani/Rust files against
the sealed originals and reads back five executed compiler/driver payloads.
Original download provenance is explicitly reused, not manufactured anew.

Unsuccessful attempts stay indexed in their original archives: initial tool
mount/mkfifo setup, Stage lint failure, olde04 route/argument panics, reviewer
filename/debug-helper mistakes and the first dangling-route finalizer.
No failed attempt is relabelled. The duplicate inspector-code finding at88e
is fixed in9f; main's export-only correction separately merged in #757.

Source/configuration/dependency/build-script/environment closure authority,
receipts, currentness, cache admission and selected CLI/MCP/backend remain open.
The child budget excludes filesystem authentication and all generated target
disk use. Inherited-group cleanup does not contain escaped process groups.
Caller ownership/immutability and private parents remain preconditions.
Full arrayvec/direct comparison, Mac, performance and release acceptance are
separate. No standard/deep conformance or theorem about this runner is claimed.

Documentation: crate README, ADR statuses, this record, error reference, roadmap
and docs index. The consumer skill is unchanged because no selected consumer
command or released backend changes. The final documentation amendment changes
no qualified executable source.

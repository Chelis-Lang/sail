# Tspec: retained wait evidence and a useful next diagnostic

This read-only audit uses the complete failed
[e0ca standard scope](m7-default-standard-e0ca-terminal.md), its
[sleeping observation](m7-standard-tspec-sleeping-observation-e0.json), and
immutable pinned Verus source objects. It launches no actor and authorizes no
change to the original flags, classifier, thread policy or 3,600-second bound.
[VI-0013](../verus-issues/VI-0013-verismo-tspec-stock-wait.md) remains unresolved.

## What the existing reproduction owns

The corpus selects `cargo verus focus` for the pinned
`verismo/source/verismo_tspec/src` package. The exact e0ca runner constructs
both arms with `--output-json --time --no-report-long-running` and
`-Z write-long-types-to-disk=no`. It sets the pinned Rust toolchain and quiet
Cargo settings, removes the listed Cargo/rustc wrapper and flag variables,
and empties the prepared target before each arm. The plain arm removes the
shim path, trace and daemon socket overrides and uses the bundled stock Z3.
These source facts are not a retained full environment dump.

At the sampled instant, the timing wrapper and Cargo were still waiting for
their descendants; the stock verifier and three actual Z3 children had
started. The runner owned the group, drained two output pipes independently,
and retained the original deadline. Its timeout classification is
infrastructure failure, not a completed verification outcome. No evidence
establishes a Cargo download stall, stream-retention deadlock or bad result.

## Futex metadata cannot currently identify a guest lock

The exact retained `proc-syscalls.json` has SHA-256
`91f3e07d302d69089a16ab4b0414121bdfaa8fa48e7a31591ae64e18187762d9`.
Comparing each syscall's first address with every interval in its complete,
untruncated maps shows that **all 16 futex addresses are absent**, in addition
to the previously documented absent program counter. All 15 reported private
PI operations have address minus reported stack pointer exactly `0x168c`;
the remaining private wait has a different offset. This arithmetic does not
identify an AIR channel, guest mutex, owner or cause. It prevents interpreting
those addresses as mapped Rust locks without a separately demonstrated
translation/ownership route. Blind address reads would not fix that gap.

The [pinned channel audit](m7-standard-tspec-channel-ownership-audit.md) still
finds no ordinary path that omits a command enqueue, reader enqueue, completion
marker or completion send. `SmtProcess` has one outstanding response owner;
normal send/read/disconnection failures panic rather than silently dropping
the response. Its destructor closes the sole writer sender before child wait.
Those static constraints do not establish which path the sleeping actor ran.
The [debugger preflight](m7-tspec-stack-preflight.md) remains failed on child
lifecycle controls and cannot be substituted for a functioning stack route.

## Concrete diagnostic option, pending scoped review

Stock Verus already supports `--log smt-transcript --log-dir`. At pinned
`rust_verify/src/verifier.rs:1228`, it chooses a per-bucket/per-query log file;
`air/src/context.rs:206` transfers that writer into the same solver process.
`air/src/smt_process.rs:167` writes and flushes QUERY before enqueue, while
`CommandsHandle::wait:214` writes RESPONSE only after receiving the reader's
completed delimiter. This offers a specific last batch and file/function
identity for a targeted replay or channel diagnosis. The historical tspec
record has no such transcript, so it presently supports no exact SMT replay.

One separately labelled stock unit diagnostic with only these additional log
flags could use the original solver, thread policy and 3,600-second bound.
It would change diagnostic I/O and therefore could not count as the unchanged
conformance attempt or explain the old failure merely by succeeding. QUERY
would remain intent, not proof of a pipe write; an unmatched QUERY would not
alone distinguish writer, reader, solver or logging failure. Observer reads
and copied evidence need explicit byte/file bounds; stock transcript growth
itself is not bounded by that observer. A concrete controller and storage plan
must be reviewed before execution. No such run is launched or granted here.

If even these diagnostic argument changes are excluded, the remaining useful
choice is a separately built diagnostic-only AIR phase observer or a proven
guest-stack route. Repeating the same empty streams and sleeping metadata
cannot reveal the private request/response channels. Neither route permits
changing acceptance flags or stitching isolated outcomes into the failed
159-unit standard scope or unattempted cache fill/warm rounds.

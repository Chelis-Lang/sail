# Anonymous impl numbering (VI-0030)

This reduced candidate has not been compiled or executed. The actual cause is
established separately by the accepted-e536 Vest witness in
[VI-0030](../../../docs/verus-issues/VI-0030-anonymous-impl-numbering.md).

The 28-line baseline consists of `main.rs`, `defs-before.rs` staged as
`defs.rs`, and the shared `client.rs`. The edited version changes only `defs.rs`,
inserting `impl Unrelated {}` immediately before the unchanged `impl Kept`.
The later impl contains an unchanged spec method and an executable closure.
Both methods and all proof bodies remain byte-identical. `client.rs` is an
untouched separate source file, avoiding shifted client source positions.

For a future focused compiler gate, stage the three files in one fresh directory
and run the accepted runtime with `--output-json --time --log smt-transcript
--log-dir <fresh-log-dir>`. Retain full stdout, stderr and every raw transcript,
including empty files and duplicate bodies. Replace only `defs.rs` by
`defs-after.rs`, use a second fresh output directory, and repeat. These are
prospective commands, not a completed result. The existing exact transcript
envelope adapter identifies literal solver inputs; it must not remove comments
or rename identifiers. Compare full method paths, closure symbols and untouched
client input bytes. Preserve either failure as a failure.

The expected hypothesis is that rustc's sibling impl disambiguator changes the
later method names and the closure's parent definition hash. Only the method
case is already established by the real Vest witness; the reduced closure
case remains unrun. No runtime or default selection follows from this fixture.

Prototype02 intentionally retains inherent impl identity, so this original
inherent-method/closure reduction remains a cause control, not a promise that
prototype02 fixes it. Separate source-only compiler controls exercise concrete
trait methods, closures and producer/native/consumer erasure parity. None has run.

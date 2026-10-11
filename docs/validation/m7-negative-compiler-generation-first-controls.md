# Current negative generation: first native controls

The six-command component scope on snapshot `a9638b4` ended **FAIL** after
361.073 seconds. All 15 affected tests passed; the final all-target strict Clippy
command exited 101 on `semicolon_if_nothing_returned` in a new test closure.
Correction `7d78ae2` adds only that assignment's final semicolon. Its corrected
native strict check remains pending. The original failed scope is retained.

The tests passed the negative-outcome refusal matrix (1), real Cargo dependency
generation composite (1), original family controls (9), original receipt guard
(1), and original driver-terminal controls (3). The real Cargo control admitted
the captured complete negative terminal through production publication, then
required an actual fresh ordinary dependency artifact and rebuilt selected
crate in the next context. It retained incomplete/nonreused negative targets
and no successful proof receipt. This is component evidence, with no installed
repair, new Verus proof or measured performance acceptance.

The run used one Cargo job, nice 0, serial test cases and a 300-second per-process
cleanup deadline. The first test included observed cold compilation and closed
in 263.886 seconds; the Cargo composite closed in 6.305 seconds. These component
limits do not change the original repair fixture's 120/30/5 limits.

Before/after hashes of the source, lock and toolchain match. The actual tested
ELF is retained (34,816,928 bytes, SHA `971a04cbeb74340fa849d1afb8c02f923594c5da6af34cf3830cf26ada0ae15d`).
Raw logs, commands, timings and the independently verified host copy of that
image are listed in the [machine-readable record](m7-negative-compiler-generation-first-controls.json).
Both fresh container cmd/cwd/FD/maps scans found zero holders and zero
unreadable live processes for the owned source, target and output paths. The
private compilation slot was released before the lead's complete workspace gate.

The recovered original fixture failure remains
[separate](m7-repair-a960-recovered-setup.md). Whole-product and benchmark
qualification remain pending; no verdict, proof receipt or Currentness predicate
was changed by these controls.

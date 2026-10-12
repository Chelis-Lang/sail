# Tspec transcript diagnostic interrupted by environment failure

The single authorized [logging diagnostic](m7-tspec-transcript-diagnostic-plan.md)
started the original plain stock unit with its unchanged 3,600-second limit,
solver, thread policy and harness draining. It did not reach a unit result.
An owned scratch write failed with `EROFS`; Docker then became unavailable and
both containers exited 255 before controlled interruption or cleanup could run.
The restart initiator was not observed. No diagnostic retry was launched.
The [machine record](m7-tspec-transcript-interrupted.json) binds the actual bytes.

The last retained observation is at 1,235.749 seconds. Its birth-bound verifier
has six sleeping threads and unchanged CPU counters; one remaining Z3 is
waiting for input. These kernel observations do not identify a userspace lock
or establish an emulation, solver or source-code cause.

Read-only recovery retains all 542 transcripts: 161,009,268 bytes, byte-for-byte
equal to the earlier complete framing inventory. It records 3,880 complete
QUERY sections and 3,880 complete RESPONSE sections, with no unmatched query
or open section. Stock AIR writes QUERY before enqueue and RESPONSE after the
reader returns; logging intent does not certify accepted solver wire. The
newest open `proof_bit64_and_rel_mod._47` transcript is empty. The main
`math__bits_p` transcript has 35 completed pairs. Pinned Verus creates the new
spun-off context's log before lazy solver launch or the first send. The context
is a fresh query-iteration local (`verifier.rs:1533`), so its initialization
does not replace a prior iteration's context. The earlier replacement wording
was incorrect. Setup or disposal in another active context remains a possible boundary;
no actual stack demonstrates the blocked operation.

The retained archive contains 545 regular files, including empty plain stdout,
stderr and timing captures. Separate controller, freeze, actor and observer
files are hashed; the private environment was excluded. The original primary
and result JSON files were never written. After external container recovery,
all six last-observed process births are absent. This point-in-time check is
not a controlled cleanup or exhaustive orphan-absence certificate. Old PID
grants were not reused.

The [complete 159-unit failure](m7-default-standard-e0ca-terminal.md) remains
158 Same plus one original infrastructure timeout. This interrupted diagnostic
neither explains that historical timeout nor qualifies standard validation,
cache fill/warm, source reuse, readers or the rebuilt product.

# HTTP known-body controller independent review

Read-only review of `db31164b53eff04af87ed9ea2537748013d7a344` found no
remaining blocker to its root-granted private validation scope. The ten pure
controls passed, and all ten frozen payload hashes matched. No native compiler,
test executable, verifier, daemon or Docker lifecycle actor was started.

The correction preserves the original HTTP candidate, fixture assertions and
phase deadlines. Timeout, interrupted wait, required group termination, and a
signal during an active command or spawn make closure uncertainty sticky.
Reaping the leader does not clear it. Further command execution, patching,
restoration and successful final closure refuse while uncertainty remains.
Interrupted spawn or progress recording can still require external cleanup;
the controller does not certify descendant retirement from its leader state.
The lead's independent holder audit remains required after controller exit.

Raw CPU interpretation now reads at most 32 MiB plus one byte before refusing
oversize logs. Both observation overlays mark unavailable thread names and a
final traversal duration of at least 10 ms as incomplete. An incomplete sample
cannot supply CPU or runqueue attribution. These observations add no original
fixture acceptance, timing distribution or historical cause credit.

The controls cover normal zero and nonzero exits, timeout followed by leader
reap, a timeout racing leader exit, TERM/KILL grace preservation, failed
termination, sticky uncertainty after later completion, bounded log reading,
small-log interpretation and incomplete sampler refusal. The overlay text
control is static; actual Rust sampling and affected tests remain pending.

Exact reviewed SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `docs/validation/m7-http-small-known-body/run.py` | `f025aef616e57b301be5813ac1efa5804eb877dcdc2b15d96bd27bf761a1250b` |
| `docs/validation/m7-http-small-known-body/inputs.json` | `e10258a9747c39ccaee99595265df64f3f493b1966cf60012c374a74e5b3309a` |
| `docs/validation/m7-http-small-known-body/from-2264076.patch` | `432d4f63bff1c39972c476fe21bd20197141c96309eef923dc3c29a192e6032e` |

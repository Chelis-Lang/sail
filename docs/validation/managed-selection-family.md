# Managed Cargo selection and dependency artifacts (#162)

Status: implemented candidate; shared-Mac all-workspace/all-targets Clippy
passes. Native correctness and performance qualification remain pending. No
performance gain is accepted. Changing package or target selection shares ordinary Cargo artifacts;
proof contexts and requested-target receipts remain separate. See
[the freshness guide](../freshness.md#cargo-selection-and-compiled-artifacts).

The retained 2026-10-17 diagnosis used Sail `d422adf0` and the genuine normal
`cc7bbce4` runtime. An interactive package verification and the next repair with
explicit `--lib` used different compiler artifact directories. Both compiled
`vstd` and `vest_lib`; the requested package was Current and complete. The raw
Verus totals for `vstd` were 26.945 seconds in the interactive verification and
26.995 seconds in the first repair. Those are observations of repeated work,
not a measured saving or a steady-state repair floor. Donor verification used a
separate ordinary engine request; that boundary remains separate.

The source correction changes only the compiler-family selection projection,
using the existing package/target selection filter. Exact arguments still bind
proof contexts. Features, profile, target triple, compiler configuration, runtime
and environment still bind the compiler family. Cargo freshness and complete
dependency receipt checks still apply; selected targets cannot borrow a sibling
proof receipt. The quarantined v1 dependency store remains disabled.

Pending validation: focused hash and real-Cargo tests, a matched native
interactive-verification-to-repair comparison with the same runtime, feature,
configuration and dependency mutation negatives, full gates and FAST/STANDARD.
Each measured attempt must retain actual target invocation records, source and
runtime identities, verdicts, diagnostics and query multiplicities.

Indexed diagnostic attempts (all remain failures):

- `a17941f3`, 2026-10-17: source-cache/receipt-cache overlap correctly refused;
  the later aggregate projection and source restoration also failed.
- `a17941f3`, 2026-10-17: selected raw capture exceeded its former 1 GiB bound.
- `a17941f3`, 2026-10-17: baseline repair passed, but the diagnostic assumed one
  total compiler invocation and saw dependency plus requested target; later
  archive relocation crossed container mounts. Candidate profiling was unrun.
  Context archive SHA256
  `f7435bc991ca0e2478f66ca2cb6337cfeea3b90d1179e7467ad44f8f23e05af5`;
  final archive SHA256
  `aecc4c46b32e853456e147d3a44b4fabee04b9d29c4114773401bd8b17648ab1`.

- `8dc02803`, 2026-10-17: first screen stopped before builds because the
  external preparer required an optional admission field. Corrected without
  changing runtime admission.
- `8dc02803`, 2026-10-17: native workspace gate rejected similar test variable
  names under Clippy; no timing ran. Final archive SHA256
  `bb37b2a4ed3711ae00958580c0c53c707503fdf74de102a750793b22927950ea`.
  The variable-only correction passes shared-Mac all-target Clippy (log SHA256
  `36cb5647201a3815da21aff52e2c7b92a83cea9f6492b2686a0be64278736693`);
  native gates and the complete matched screen remain required.

# Neutral contract service qualification

## Scope and result

The installed-service acceptance on source `96af775e` passed on October 21,
2026, native Linux x86-64 on the owned c7i.4xlarge qualification host in
us-east-1. It used the pinned fork `f8589ed4345f147d8fb43d8cb7992921944b91f0`,
Z3 4.16.0, Sail Rust 1.99.0 and backend Rust 1.99.1 in the existing CI image.
The [service specification](../spec/neutral-contract-jobs.md) defines its bounds.
Full native gate, solver, complete e2e and fast/standard conformance on the final
qualifying source remain separate required gates; their exact source and log
hashes are recorded in the qualifying pull request.

The release acceptance binary started the actual installed Sail service and
submitted only exact source, its digest and a compiler crate name through the
capability-negotiated API. It did not invoke the helper or verifier directly.
The positive source was `Proved` with 2,061 unsat checks and no unknowns. The
body-only edit was `NotProved` with 2,062 unsat checks and one unknown, retaining
`(:reason-unknown "(incomplete quantifiers)")`. That unknown is not a concrete
SMT refutation or mutation kill, regardless of the unchanged legacy function
label. The separate compiled edited-source sample at input 1 satisfied the
precondition and failed exactly the ADD-ONE postcondition with `SAIL0702`.

For both sources, public job currentness remained `Unknown`, the original
source file stayed unchanged, and the typed result matched the exact complete
worker envelope. Restart preserved results. Independently retained baselines
survived deletion of both original job directories. Baseline currentness was
historical, with identical source-bound contract metadata.

| Original evidence | SHA-256 |
|---|---|
| Installed-service log | `ac105be277a469c0f34b81878fbefcad7169850ab1dab8a4455a10609ca94cbf` |
| Positive exact worker stdout | `32bb4cd50717eaa25e650364f1fd244f0389f18acca4be42c641830472cdabeb` |
| Body-edit exact worker stdout | `e2bc89e5d80f5138231dca66b5273f978fbe8c487cbba8f9f37cdf054e65dacb` |
| Separate compiled runtime witness | `e3f6da4ac1104880e4f44356ff37b657f2159326eb84a7adfc5c1ee3b46edfa7` |

The same source passed the macOS host gate, including all 916 selected tests.
Fast injected tests separately cover strict whole-request decoding, old-service
refusal before submission, exact ordinary wire compatibility, source and output
tampering, forged proof/manifest metadata, restart, baseline pruning and actual
process-group cancellation. A real shell control with a reaped leader and a
remaining descendant proves both deadline and cancellation release inherited
output pipes. These injected checks do not establish a backend proof.

## Final-source hardening

The subsequent source seals neutral job stderr at worker completion and retains
its exact bytes and hash with each independent baseline. Tampering before save
or after original-job pruning is an explicit integrity error. The compiled
service acceptance checks exact diagnostic bytes after pruning and verifies
that changing them refuses readback. Diagnostic text establishes no proof.

| Source | Gate scope | Result |
|---|---|---|
| `72e433c2` | Full native gate, pinned solver, macOS host | Passed: 927 native tests, 39 solver tests and 916 Mac tests |
| `72e433c2` | Complete e2e sequence | Continued unchanged to its boundary before the stderr hardening; no final-source claim |

## Limits

The proof is conditional on the declared preconditions and the qualified scalar
source/translation subset. Requirement entailment, feasibility, stable
requirement tiers and mutation establishment are not established by this gate.
The client still must hash its actual current source. Source-bound semantic
preconditions, guarantees and definedness for requirement entailment remain a
separate interface, beyond this source-mapping service.

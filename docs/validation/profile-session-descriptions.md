# Diagnostic check/session descriptions

Status: bounded native Linux component qualification and fresh independent
review pass at `f91ab7dd0d02a992a20e2df101d4d23310c3a354`. This pure library
collector remains unselected by verification. It authenticates no session,
source, solver, currentness or proof, and exposes no profile command.

The collector reuses the named AIR/timing identity join, counts original
process checks across obligation boundaries and preserves ordinals/spinoffs.
Missing, ambiguous, duplicate, malformed and over-budget descriptions refuse
the entire collection. Empty/unselected spinoff logs can conservatively make
profiling unavailable. It reads no files and invokes no solver.

Native c7i.4xlarge/us-east-2 qualification uses Rust 1.99.0 and image
`6c19f5c8d397b61c-core`:

| Scope | Actual result |
|---|---|
| Affected Clippy and focused attribution suite | Pass; 25 tests including seven collector tests |
| Complete workspace gate | Pass; 1,438 workspace tests / 58 configured skips, fmt, Clippy, examples, doctests, rustdoc, layering, deny and benchmark freshness |
| Independent public API controls | 49/49 pass; exact expanded-text/row boundaries, identity/collision/version controls, positions versus reply indices, spinoffs, late all-or-nothing refusal and malformed/budget cases |
| Independent original AIR/SMT oracle | Six accepted capture/reorder cases, 19 exact bindings; original pipe digests, ownership and all process/obligation indices agree |
| Fresh pinned-Z3 reproduction | Five reconstructed original streams reproduce all 14 recorded answers |
| Causal allocation controls | Predecessor long-module input is OOM-killed under 256 MiB; successor returns SAIL5104 at 17,972 KiB maximum RSS, while both short-module controls succeed |

Named producer `a3109bed` conclusions use independently rehashed original
compiler captures, fresh collector execution and fresh pinned-Z3 replay. The
named compiler itself is unavailable here and is not called freshly qualified.
The causal memory controls are not a product performance benchmark. Retained-
input/invocation authentication, profile/worker/CLI/MCP integration, corpus and
Mac/product acceptance remain separate. Under ADR0087 this unselected diagnostic
component changes no selected runtime behavior; no corpus entries are affected.
No full conformance, E2E, solver tier or model/refinement run is inferred from
the component checks. Hosted required gates remain separate.

All 598 predecessor and 471 successor review payloads are independently
lead-rehashed and externally retained. Predecessor archive SHA256:
`c67684c0f756415307ac0a19b2f4fd467b11473c987d5e21086c43579490ae6d`.
Successor archive SHA256:
`c07c4919723235679cd8876b80117b24d5f252a52da80ca59509713c5aba496d`;
index `f7d7f27256cd84cdc4bab7db71d52a7556fcc44b740cb5de50713cd84b0e370e`.
All 57 selected original compiler files also match their original archive/index.

Documentation: ADR0146, this record, crate README, errors and changelog. Book
source/link checks, mdBook 0.5.2 build and generated links pass; the independent
core-image run lacks Node and does not execute its optional highlighting probe.
No CLI/MCP commands or release availability change, so consumer skill commands
are unchanged.

## Retained attempts

| Source / attempt | Actual result |
|---|---|
| dd20a84b / native1 | Affected all-target Clippy refuses redundant clone-to-slice expressions in the new tests. Unit tests are not reached. Successor uses borrowed single-element slices; all original assertions and inputs remain. |
| a91db645 / native2 and independent review | Clippy and six tests pass, but independent review rejects memory amplification: a <0.5 MB timing input with 20,000 rows and one 16 KiB module consumes 341,572 KiB RSS versus 23,188 KiB for a short module; under 256 MiB it is killed. The legacy helper clones the module per row. Successor preflights projected row count and expanded row text before calling either helper. |
| f91ab7dd / independent gate1 | Reviewer-forced absolute Cargo build-directory overrides leak into nested fixtures: 1,425 pass / 13 fail / 58 skip. Unmodified successor gate with normal private directory layout passes; all original failures remain. |
| f91ab7dd / named compiler setup | Named runtime path is absent; no fresh named compiler executes. Original captures and fresh Z3 reproduction are distinct scopes. |

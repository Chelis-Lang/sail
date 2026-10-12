# Independent ordinary-script bootstrap review

This file-only review gives a static authority ACK to bootstrap commit
`f697201db80717ec4d2d94c53855d7cbb22bf59f` against the retained product-730
terminal records. The [source record](m7-ordinary-script-bootstrap-review-source.json)
distinguishes the initial draft from the final five independently verified source
hashes and Cargo.lock. No Docker command, target read, Cargo invocation, compiler,
proof or benchmark ran. Seven native controls, sensitivity, strict checks and
unchanged whole-product validation remain pending. Static review is not
production qualification or a measured benchmark result.

The [original benchmark failure](m7-benchmark-namespace-730-review.md) remains
failed with no measured samples. Terminal inventory identifies one generated
serde source per context, but lacks the old observer binding. A plausible
clean/regeneration cycle is not identification of the historical failed root.

## Supported transaction

`build_inputs_family::begin` consumes the export before target mutation. Missing
or invalid records preserve old declarations only as uncovered watch paths.
The draft now reports availability only for a nonempty, wholly eligible export;
an empty or partly refused export can therefore acquire the optional bootstrap.

`runner::run_engine` acquires this independent owner before stale-script cleaning,
over package sources, prior script declarations, Cargo configuration and actual
runtime inputs. It does not add generated semantic outputs to this script-input
transaction. The original complete proof snapshot still includes those outputs;
its monitored snapshot, native/kernel guards, currentness and receipt rules are
unchanged. An independent ordinary-output export cannot promote an Unknown proof.

The final bootstrap admission requires a complete successful unfocused batch,
the same family lock, matching effective declarations, output seals and compiler
roles, initial/final independent content equality, and clean original native and
kernel owners. Directory declarations, build dependencies, multiple output units,
unsupported environment and unavailable coverage refuse the optional export.
Refusal retains original fresh-script cleaning and proof diagnostics.

## Actual compiler roles and fallback

All three retained final journals contain 23 compiler-artifact events and six
build-script events. All six custom-build executables in each journal report
`fresh=false`. All six corresponding metadata packages have no build dependency.
The independently verified supplemental dep-info contains 12 files, each naming
only its `build.rs` in the first input rule, with no compilation-time `env-dep`
line. These observations support the conservative draft role for these records;
they do not certify acquisition coverage or that a new bootstrap would publish.

The runner truncates the journal before its fallback invocation. The draft must
validate the final journal's actual role, rather than combine an earlier compile
with a later replay without a generation binding. Refusing final `fresh=true` is
conservative. A separate control should cover fresh first-pass compilation followed
by reused final artifacts and demonstrate refusal or a separately justified role.
The retained journals have no `build-finished` event, so admission cannot require
one without changing artifact capture. A `build-script-executed` event alone
does not establish that the script executed freshly.

The repair function selector is not a partial Cargo passthrough scope.
`engine::repair` rejects those partial arguments, sets `force=true`, obtains
the full result from `runner::run_engine`, and only then assembles the separate
focus projection. Bootstrap publication inside `run_engine` therefore examines
the full batch's requested targets, not the later focus graph. The function
request remains bound in the proof context; clearing it for the ordinary compiler
family cannot transfer proof authority.

## Closed review findings and remaining limits

The initial `Bootstrap::covered` resolves a declaration or dep-info input and
tests canonical inventory membership. For a newly learned raw symlink outside
all original roots, that is insufficient: the alias can resolve elsewhere during
script use and return to the guarded file before final validation. Both guarded
files can retain unchanged bytes while the unobserved alias changes. Require
same-owner raw alias coverage or conservatively refuse newly learned noncanonical
paths. An external alias switch/restore control must exercise this exact path.
This issue was sent to the implementer and lead. The final commit conservatively
requires raw path equality with its canonical resolution and adds an external
switch/restore control. That closes the static alias seam. The control remains
unexecuted, with no claim that the historical failing path was an alias.

The dep-info is a trusted Cargo/rustc-generated private artifact, not authenticated
compiler evidence. Required headers, actual target source membership, fresh
compilation, output seals and input coverage detect unsupported or malformed
roles. They cannot prove that a malicious writer omitted an unknown dependency
from both private metadata and Cargo output. Keep the explicit
[ADR 0036](../decisions/0036-managed-compiler-artifact-families.md) trust boundary; do not
describe `fresh=false` or a checksum as authentication.

The external dep-info control initially replaced its required header, allowing
refusal before testing outside-input coverage. The final control preserves the
actual header and adds the external prerequisite. Tiny Cargo producers use GNU
timeout, file-backed captures, bounded capture reads and an owned cleanup guard
with bounded reap on error. Exceptional supervisor teardown explicitly does not
certify descendant closure. These are control and lifecycle corrections, not
changes to production or benchmark deadlines.

The draft accounts retained owner fields and admits at most 40 MiB, with a
separate 32 MiB observer-plus-lease sublimit. Its hashing has per-file and aggregate
byte limits and cooperative cancellation/deadline checks. These do not bound
blocked filesystem calls, inherited construction/parser transients or OS RSS.
The final revision uses exhaustive field destructuring in the retained ledger so
additions cannot silently escape. The initial per-file check bounded only
metadata length; the final stream decrements both per-file and aggregate byte
allowances, including growth. A refused read can consume one further fixed
64 KiB buffer before detection. The 4 GiB aggregate and 30-second cooperative
hash phases have no complete-runtime measurement yet. Retained admission is
distinct from allocation before growth.

No static authority or generation blocker remains within the explicit private
Cargo-artifact trust boundary. Native controls and the unchanged original
two-warmup benchmark remain necessary to establish actual bootstrap viability.
No measured bootstrap gain, Current authority transfer, reader regression fix or
benchmark acceptance follows from this review. The complete original reader pass
on product 730 remains a separate scope.

## Environment and compilation follow-ups

Static review also ACKs environment-policy commit `91a2cb8` and its narrow
correction `b4a8f0dd5c4c7199b7616b5010d24194016ab536`. Both immutable source
manifests and all five source files match their recorded hashes. The original
`f697201` review remains retained; these follow-ups draft eight native controls,
with no native execution in this review.

The shared family/bootstrap policy refuses unaudited child environment in both
execution declarations and compilation dep-info. Cargo documents dynamic-library
search-path modification, resolved `RUSTDOC`/`RUSTC_LINKER`, `DEP_` metadata and
removal of `RUSTFLAGS`; parent values cannot certify those child values.
The pinned `RUSTC_BOOTSTRAP=1` exception remains explicit.
See the [official Cargo environment reference](https://doc.rust-lang.org/cargo/reference/environment-variables.html).
The additional conservative refusals grant no new output or proof authority.

The eighth actual-Cargo control originally required no file after both publication
routes refused an injected `LD_LIBRARY_PATH` declaration. Existing ordinary
publication deliberately writes a valid empty envelope after filtering all
records. The corrected control preserves that representation and requires exact
empty records, `begin=false`, and forced cleaning of the fixture package. The
independent bootstrap route still requires absent export. An empty envelope
cannot authorize reused script outputs, and the original empty-generation
recovery control remains unchanged.

The first native attempt failed strict compilation before Rust tests: the runner
used similar `owner`/`owned` names and borrowed a moved hash. The correction only
renames locals and borrows the unchanged owning report's `context_sha256`.
The next strict attempt failed import placement and stack-array lints, corrected
in a later source revision. The [separate gate record](m7-core-244d51d-validation.md)
retains both failures beside the earlier 244 workspace/solver PASS. None qualifies
the bootstrap candidate or identifies the historical first changed binding.

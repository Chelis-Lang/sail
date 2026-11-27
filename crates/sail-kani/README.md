# sail-kani

Development groundwork for the experimental Kani backend selected by ADR 0109.
This first library slice decodes bounded observations from pinned Kani 0.68.0
and CBMC 6.11.0. It neither invokes Kani nor creates proof receipts, currentness,
cache authority or an ordinary verified result. Bounded native Linux decoder controls and independent review pass. CLI execution
and complete direct-Kani pilot qualification are separate gates. See ADR 0128.

`Observation::bounded_evidence()` builds the shared bounded-evidence record of
ADR 0185. Nothing consumes it yet.

The internal Linux release-bundle pin authenticates the official 0.68.0 archive
and every original file. `ci/kani-bundle.py` installs that fixture only; it does
not install or authenticate the external nightly Rust toolchain, execute a
harness, enable cache reuse or make a CLI backend available.

ADR0149 separately pins the original six Linux Rust component payloads. The
read-only `ci/kani-toolchain.py` compares original archives and optionally an
immutable installed prefix; Rustup bookkeeping remains untrusted. It does not
install or execute a backend. See `docs/validation/kani-rust-payload.md` for scope
and native qualification; backend execution remains separate.

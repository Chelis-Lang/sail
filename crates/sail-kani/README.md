# sail-kani

Development groundwork for the experimental Kani backend selected by ADR 0109.
This first library slice decodes bounded observations from pinned Kani 0.68.0
and CBMC 6.11.0. It neither invokes Kani nor creates proof receipts, currentness,
cache authority or an ordinary verified result. Bounded native Linux decoder controls and independent review pass. CLI execution
and complete direct-Kani pilot qualification are separate gates. See ADR 0128.

The internal Linux release-bundle pin authenticates the official 0.68.0 archive
and every original file. `ci/kani-bundle.py` installs that fixture only; it does
not install or authenticate the external nightly Rust toolchain, execute a
harness, enable cache reuse or make a CLI backend available.

ADR0149 separately pins the original six Linux Rust component payloads. The
read-only `ci/kani-toolchain.py` compares original archives and optionally an
immutable installed prefix; Rustup bookkeeping remains untrusted. It does not
install or execute a backend. See `docs/validation/kani-rust-payload.md` for scope
and native qualification; backend execution remains separate.

A development Rust tool-payload authenticator is being integrated for the
executor (ADR0152). Its focused Linux original-payload checks and workspace gate pass; independent
and Mac qualification remain open. It executes no backend and
grants no proof or cache authority. Prefixes must be owned and immutable.

An owned execution-layout candidate (ADR0153) stages the unchanged original
bundle with an exact route to authenticated Rust. It is uncompiled and supplies
no harness execution or CLI backend.

# Checked report verification mode

The managed adapter builds a reported command for the package whose effective
Verus options it observed. Its actual construction now delegates to the same
`request_mode::construct_report_verification_arguments` body checked by pinned
Stock Verus. Physical compiler arguments and environment are unchanged.

The constructor retains the original token bytes and order. With verification
enabled, it removes only forwarded `--no-verify` tokens after the original `--`.
With verification disabled, it preserves original arguments and adds only the
missing `--` and `--no-verify` markers. Unknown effective options return `None`;
they cannot silently establish that verification was enabled.

For example, enabled options map `["--", "--no-verify", "payload"]` to
`["--", "payload"]`. Disabled options map `["payload"]` to
`["payload", "--", "--no-verify"]`. A `--no-verify` after the effective options'
own Rust-argument separator does not disable verification. The existing
compile-only and partial-report admission guards remain in force.

The proof covers the real construction loop, its retained prefix, original mode,
removal/append branches and returned arguments. Five nonempty/unknown witnesses
and fourteen executable mutations retain the original contracts and loops.
The native regression includes prefix/focus tokens, Unicode, NUL and unknown
options; existing native assertions remain.

This is a report-mode operation, not complete request construction or Cargo
selection. The surrounding adapter and packed-option decoder remain native
boundaries, separately hash-bound and excluded from checked-body metadata.
Original effective-option/launch provenance, selector scanning, dictionary/header
construction and captured-request consumption remain separate work under
[#368](https://github.com/Chelis-Lang/sail/issues/368). Raw metadata/DOM and the
unproved builtin IEEE bridge remain on their separate candidate branches; this
delivery neither removes that obligation nor claims its scope.

The [qualification record](../validation/report-verification-mode.md) distinguishes
implemented source, native checking and direct Stock/control evidence.

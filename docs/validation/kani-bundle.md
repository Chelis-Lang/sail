# Kani release-bundle fixture

Status: implemented; eight local Python control groups pass. Native installation,
full local gate and fresh review remain pending. No Kani executor, CLI backend,
nightly Rust authentication, currentness or reusable proof authority is added.

The authoritative Linux pin names Kani 0.68.0 / CBMC 6.11.0, the official
140,315,343-byte archive SHA-256
`32e2b484d73ede0bbf64a2cf0879c4259422497d8aec4ee67d448de9ae7843d3`,
and 120 original files totaling 507,989,223 bytes. Independent local archive
inspection verifies the release API's digest and derives every file digest,
size and executable bit. That is artifact inspection, not native execution.

Required native scope: install the exact archive, independently rehash every
installed file, preserve notices/modes, verify no external toolchain symlink
is manufactured, run installer controls and the full local gate. Fresh review
must probe path/type/identity/bound refusals, pre-publication faults and retries.
The installer handles a Linux contributor fixture only. Real Kani/arrayvec,
process faults/cancellation, Rust input closure and Mac remain #712 package G.

Documentation changed: ADR0137, this record, crate README and changelog.
No released user command changes; consumer skill and installation recipes stay
unchanged. There is no selected-runtime, model/refinement or dependency change,
so no corpus, benchmark, formal or Mac result is claimed for this fixture.

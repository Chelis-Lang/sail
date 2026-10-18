# VI-0025: Loaded files do not describe absent module candidates

Status: reproduced with rustc 1.99.0 on macOS; conservative retention eligibility.
This is a compiler-input observation limitation, not a wrong-verdict finding.
No upstream report has been filed.

For `mod foo;`, both `foo.rs` and `foo/mod.rs` make the program ambiguous.
Removing the second candidate permits compilation; restoring it makes the
same unchanged loaded `foo.rs` insufficient to describe the earlier success.
A SourceMap record of files actually read does not certify candidates absent
during resolution. Endpoint source hashes must not substitute for that fact.

The minimal reproduction is in `corpus/repros/rust-module-candidate-absence/`.
The actual three commands returned 1, 0 and 1; both ambiguous stderr streams
have SHA-256 `3d166b2f029e9530cfe2fa1ef3b8aea6644f185684a35a4a2ea96f91dabca372`.
The external result index has SHA-256 `2cff15d21509de62d7a075f981d45a8d5d6d48ef7c66f30f5113960694efb4d3`.

The retained-proof implementation refuses candidate-pair ambiguity
anywhere in admitted roots. Outside its exact fixed provider closure, it also
refuses unsupported path/include syntax rather than claiming general lookup
closure. Final-source scoped native Vest ABA qualification passes in the
[retained record](../validation/instant-revert.md); it does not establish a
general-purpose source-absence witness.

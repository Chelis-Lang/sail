# Archive installer

The installer is qualified for actual Linux x86-64 and Apple-silicon Mac
archives, with the pinned Verus runtime `7d9d5410` and Rust `1.99.1`.
The Linux archive uses main's `9f819166` Rust inputs; the Mac archive also
contains the bounded source-check/transcript changes from `1ccf52ec`.
These checks qualify installation, not those changes' performance.

Both native hosts pass checksum/layout and bundled-runtime validation,
explicit pinned-toolchain setup, verification of an existing compatible
project without manifest/lock/source changes, and an unchanged call with zero
verifier invocations. Integrity, unsafe archive, existing-install and failed
installation cleanup controls pass. Private download failures explain how to
authenticate; no public release or arbitrary Verus-pin compatibility is claimed.

Raw install logs in the private evidence bucket, under `experiments/2026-10-24/`:
`install-linux.log`, SHA-256 `92c834c0526f9ce3f577eef33721509f232c828552ae03373bbdb05fcbc528a2`;
`install-mac.log`, SHA-256 `e000db2919685a399eb215d590dab469ba3924f7362cf1cebb32d6c239d4d80a`.

# Private authentication and proof reuse

The candidate based on main `1b8bfd31` excludes only
`SAIL_DEPENDENCY_READ_TOKEN` and `SAIL_PRIVATE_GIT_READY` from the generic
proof context. Resolved dependency source bytes remain inputs. Explicit
compiler and build-script environment declarations retain their value checks.
Background-job request environments keep their existing behavior.

On native EC2 Mac m4, us-east-1, three rotated rounds used an unchanged
generated binary project and factory-built installs with fork `7d9d5410`,
Rust 1.99.1 and pinned Z3. Each round changed both authentication variables
after the first successful verification. Source bytes stayed identical.

| CLI call | Baseline median | Candidate median | Candidate verification |
|---|---:|---:|---|
| First use | 7.955 s | 8.005 s | 1 invocation; shipped vstd served, 3/3 |
| Authentication changed | 7.875 s | 0.482 s | 0 invocations, same context, 3/3 |

The baseline changes context and verifies again on the second call. This
qualifies removal of that invalidation; it does not close ordinary unchanged,
small-edit or real-project cold overhead. The baseline install is from
`9f819166`; its relevant environment classification is unchanged at the parent.

Actual native Mac controls additionally add an `option_env!` source read:
changing the declared credential produces a fresh verifier invocation and a
nonreused requested target. The Linux managed E2E suite includes both the
authentication-only and declared-read controls. The Mac host gate passes
1,034 tests; native Linux workspace (1,045 tests), solver (42), full E2E plus
extra and all four PR conformance arms pass (33 Same, 311 functions each).
All four full-fast arms pass: 147 Same and 3,451 functions each. All four
standard arms pass: 159 Same and 13,166 functions each, followed by the
bounded mutation controls. No functional differences or infrastructure errors
occurred; the standard arms retain 17 resource-count-only drift cases each.

Raw native Mac controls, factory and host logs:
`s3://chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-24/credential-context-mac.tgz`,
sha256 `0812d0990e2438237e6be18994d87164a10094ba25e8c1c71c534eecd7f11ca7`.
Linux gate and conformance logs:
`s3://chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-24/credential-context-linux.tgz`,
sha256 `913af2c04f1aba602ad79ef39f6e0359b7c82ad83918cbcc98c6d5d1b01770b4`.

An initial Mac control assumed a library source path for a generated binary;
the corrected control uses its actual main source. Copied-binary cold timings
invalidated the bundle's executable admission and were excluded. The table
uses the normal factory, with positive shipped-proof controls on every round.

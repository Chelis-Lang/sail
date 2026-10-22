# Unchanged partial verification

**Status:** native implementation and focused controls pass; final release
matrix and Mac measurements remain pending. Anvil's redundant verification is
removed, but its remaining unchanged-run overhead is still visible.

On native c7i.8xlarge in us-east-1, runtime f8589ed4 on upstream 7d82994,
Cargo/Rust 1.99.1, the installed candidate at `486e46ff` verifies Anvil's
`--lib -- --verify-module vdeployment_controller` scope successfully. Two
unchanged calls take 2.05 and 1.89 seconds, with zero verifier invocations,
no build-script cleanup, and Current inputs. The requested target remains
incomplete and carries its exact `scope_reuse` arguments. Whole verified
dependencies retain their separate proof provenance.

The preceding installed attempt at `1908b51c` took 39.23 and 39.01 seconds:
`ring` declared Cargo package values and `OUT_DIR` that Sail could not validate.
It discarded those script contracts, cleaned `ring` and `rustls`, and repeated
the root verification. The correction binds package values to current Cargo
metadata and the output directory to Cargo's actual execution unit. A managed
fixture checks recorded values against the script's actual environment and
keeps the existing changed-input and failing-proof controls.

The earlier stock unchanged baseline was 0.287 seconds. These bounded runs
establish removal of the repeated work, not performance parity or release
acceptance. Coarse setup timings attribute the remaining time to input and
receipt validation, Cargo invocation, and publication; no diagnostic comparison
change is responsible for the improvement.

The composed installed candidate passes the native workspace, solver, full e2e,
PR/full-fast and standard conformance gates. Each standard corpus arm has
159 Same and zero disagreements or infrastructure errors; its bounded
HumanEval mutation controls also pass. The Mac host gate passes 946 tests,
Clippy clean. Build-script acceptance passes. Managed acceptance covers repeated exact
scope reuse, changed selectors, a failing edit, forced fresh verification,
missing proof receipts and restored-mtime content changes. Receipt controls
reject whole-proof consumption, different or reordered arguments and changed
artifacts. The three-round Mac matrix still finds a first unchanged Anvil run
that rebuilds dependencies (about 35 seconds). A separate repeated-call control
then reaches 2.1 seconds with zero verifier invocations. This first-rerun defect
and the remaining setup overhead remain open; these results do not authorize
release acceptance.

Raw bounded control: `experiments/2026-10-21/anvil-scope-control.tgz` in the
S3 evidence bucket, SHA-256
`c2e59a81c4855735dd59ae7e18957d9e76b0a13d304f3b3668460ddb44dac288`.

## First recheck after native-header discovery

The Mac cold Anvil call discovered OpenSSL headers only after compilation.
Nothing changed, but the scheduling snapshot gained external input paths, so
it withheld Current status and the reusable state. The first unchanged call
therefore compiled and verified again (34.9 seconds); only the next call reused
its reports.

Preobserving the explicitly configured OpenSSL include tree makes the first
call Current. A staged native Mac control (mac-m4.metal, us-east-1a, macOS 27.0,
Cargo/Rust 1.99.1, runtime f8589ed4 on upstream 7d82994) now records:

| Call | Seconds | Verifier invocations |
|---|---:|---:|
| Cold | 105.79 | 3 |
| First unchanged | 2.41 | 0 |
| Second unchanged | 2.15 | 0 |

CLI SHA-256 `718f93a74db86aff9e56dea7c338d8023ddda9bdd968a2e504115d5fabac7971`.
All three calls exit zero. Partial requested targets remain incomplete; the
unchanged calls reuse their exact-scope reports. This corrects first-recheck
rebuilding, not the remaining setup overhead or final release acceptance.

The filesystem control reproduces the newly declared external header, verifies
that the initial hinted and later declared inventories agree, and still detects
a same-length header change. Relative and missing hints remain ordinary misses.

The standalone first-recheck correction (`e581205c`) passes native Linux
workspace, solver, full e2e including extra, PR/full-fast and standard gates.
Standard records 159 Same and zero unstable, Different or infrastructure errors
in all four arms; its 18 HumanEval mutation controls and both reverts pass.
The composed Mac host build, lint and tests pass. Raw native gate logs:
`experiments/2026-10-21/anvil-first-recheck-gates.tgz` in the S3 evidence bucket,
SHA-256 `993581d78e83a5904d710d7b6075f87b775b9da91ec5b38cdcd947876501cc4a`.

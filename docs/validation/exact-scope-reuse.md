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

Native workspace gate: 910 tests pass, 40 skipped, Clippy clean. Build-script
acceptance passes. Earlier composed managed acceptance covers repeated exact
scope reuse, changed selectors, a failing edit, forced fresh verification,
missing proof receipts and restored-mtime content changes. Receipt controls
reject whole-proof consumption, different or reordered arguments and changed
artifacts. Final composed full gates and paired matrix rows remain pending.

# Cargo clean removes both versions of a package

Observed with Cargo 1.99.0 (`5f94df478`), rustc 1.99.0, macOS aarch64,
2026-10-16. Native Linux qualification is pending.

Run `python3 corpus/repros/cargo-clean-package-versions/repro.py` from the
repository root. It uses a temporary, offline workspace with two versions of
`same-name`, each with a build-script execution counter outside its source
directory. The application depends on both versions. No Verus or Sail binary
is involved.

Both `cargo clean -p same-name@1.0.0` and a clean using version 1's full
metadata package ID remove both versions' fingerprints, output and build
script products. The next build executes both scripts: `[1, 1]` becomes
`[2, 2]`, then `[3, 3]`. The verbose clean logs show the removed paths.
Specifying the exact package ID therefore does not isolate Sail's clean to
one version on this Cargo.

Sail's issue [#174](https://github.com/Chelis-Lang/sail/issues/174) arises when
metadata includes a script package Cargo never schedules, such as a dev-only
dependency. Repeatedly cleaning that package also removes a scheduled
same-name version. Sail can remember successful Current scheduling facts
under the identical complete input, graph, selection and runtime context;
changed or missing facts retain conservative cleaning. The memo does not
authorize proof reuse or replace any observed script's input checks.

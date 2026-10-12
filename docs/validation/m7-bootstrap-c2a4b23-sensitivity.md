# Bootstrap emission sensitivity on c2a4b23

The separately owned actual-Cargo component passes its on/off/on sensitivity
check on frozen `c2a4b2363b415aa2157fc38dc388192125b87109`. Each compile succeeds.
The unchanged exact positive test passes, fails with exit 101 when only independent
ordinary-generation emission is removed, then passes after exact source restoration.
The failed assertion is `independent ordinary-script contract must escape
absent-generation bootstrap`; its oracle was unchanged.

The [portable record](m7-bootstrap-c2a4b23-sensitivity.json) independently verifies
all retained host files and all 20 regular raw-archive members. The archive is
29,552,061 bytes, SHA `f308d0f464c8b2e0672b82e11d426c9b1210ba8597d8ebfa844a14c246263818`.
Each test command directly executes its retained image. Positive and restored
images have the same SHA (`ec7c7a94…`); the counterfactual has a distinct SHA
(`47d0492d…`). These are execution-bound copies, unlike the operator-selected
retention in the [whole gate](m7-core-c2a4b23-validation.md).

Host duration is 73.808 seconds including compilation, checks and archival work.
Compile limits remain 600 seconds and each exact test 180 seconds. Original
checks report source restoration, unchanged protected bytes/PID1, equal container
identity/image/birth, and no live, unknown or terminated-unreaped member in any
recorded group. This host review starts no native actor and makes no general
orphan-absence claim.

This confirms the component assertion detects removal of the new emission path.
It does not establish installed real-project recovery, proof currentness,
historical failure cause or performance improvement.

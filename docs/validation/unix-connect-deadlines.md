# Unix connection deadlines

Status: native Linux workspace, solver, complete E2E, full fast/standard
conformance and actual fault controls pass; Mac platform and fault controls pass.
No dependency, protocol, proof key, admission check or solver input changes.

The preparation, solver-cache and job clients previously called blocking
`UnixStream::connect` before installing read/write timeouts. A saturated
listening socket can therefore block them before those timeouts apply. The
existing runtime client already uses `socket2::Socket::connect_timeout`.
The other clients now share that same bounded mechanism, with their original
25 ms, 10 s and 5 s timeouts respectively. An unavailable cache still falls
back to the pinned solver; optional preparation declines normally.

The real-listener regression fills an accept queue, invokes the actual cache
client and requires a bounded failure. It closes the listener and reaps the
worker before asserting, allowing the old blocking implementation to fail
without hanging the gate. Healthy connections retain both I/O timeouts;
zero timeouts remain errors. These are transport controls, not proof acceptance.

Source `791381bf`, native c7i.4xlarge / us-east-1 / Ubuntu 24.04, pinned
`0ad405eb` runtime: workspace Clippy, 1,322 tests and policy/documentation checks,
all 51 solver controls, complete E2E including local `extra`, and four full fast
arms pass (each 147 Same / 3,451 function records; zero differences or errors).
The four standard arms also pass, each 159 Same / 13,166 function records,
zero differences, instability, infrastructure errors or skipped projects.
All 21 standard mutation/cold/return comparisons match, with zero false verified
results and both returns recovering their own answers. Full native capture,
including standard per-function reports and raw streams:
`connect-deadline-native-complete.tgz`, SHA-256
`1bf63fcad5e4d87abbff414ea77e7e1161ab99563655e35d8f3669268c73e7f0`.

The actual old main `5bd4a66a` binaries block beyond the five-second observation
on both `sail new` and cached `sail-z3 -smt2 -in`. The new binaries return exit
zero; shim stdout/stderr agree byte-for-byte with pinned Z3, including the model.
The original timed-out children are killed and reaped by the private control.
Gate logs and actual old/new streams: `connect-deadline-native-controls-r2.tgz`,
SHA-256 `ba124162840d151975a25741fe907161e76e0dfcc267486e37c76624939c5a42`.
Its standard log is an in-progress capture, not a completed standard result.

Native mac-m4.metal / us-east-1c / macOS 26.7 passes Clippy and all 1,311
non-solver tests, including the real saturated-listener API regression.
`mac-connect-deadline-evidence.tgz`, SHA-256
`bc55a863f95e769c579a1296f042ad401b129c66805e6a8c0a21794f5b951838`.
The first extra CLI/shim capture there used pass-through solver flags and
does not establish cached-shim fallback. The corrected Mac control passes with
actual cached-mode flags: healthy runtime setup is checked separately, then
both project creation and byte-identical pinned-Z3/model fallback succeed behind
full queues within the unchanged five-second fault observation. The old Mac
binary also returns; the old blocked behavior is established on Linux only.
`mac-connect-deadline-r3-evidence.tgz`, SHA-256
`c066f94e4221ab975c09088346689a8114fa4b3c5a9aa45271b89181a065793b`.

- 2026-11-02, actual Linux backlog control r1: old `sail new` blocks, but the
  old shim returns because the driver uses pass-through `-in`. The negative
  assertion rejects that setup. Using the actual cached-mode `-smt2 -in` flags
  passes both old-blocked/new-returned assertions with unchanged byte checks.
  Original capture SHA-256:
  `5dba04dc6244c92ebb44ea5c6db925df4370b6dbb92d285f87759505e70ef90e`.
 The release-expansion timing cohorts retain their frozen source scope;
this transport repair does not establish a new performance claim.

- 2026-11-02, recovered-Mac cached control r2: the old control does not create
  its project within the five-second observation; the comparison refuses before
  reaching a usable before/after result. The retry first requires successful
  healthy creation, then applies the unchanged fault checks. Original capture
  SHA-256 `5421460aaa7def89201612034d7be4bdcfd83f580d1766b4104640da9328edf2`.

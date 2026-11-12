# MCP missing-endpoint fixture reliability

| Field | State |
|---|---|
| Established source | Current main `33e6078d`, native c7i.4xlarge/us-west-2, Ubuntu24.04, Rust1.99.0 |
| Defect | The temporary shell fixture intermittently fails execution with ETXTBSY |
| Reproduction | Twelve complete native MCP library suites: eleven pass, round twelve fails the missing-endpoint fallback test (68 pass, one fail); all twelve separate no-reuse suites pass |
| Change | Launch the existing fake command/response logic through its installed-shell fixture helper |
| Classification | Test reliability only; no production, solver, cache/reuse, verdict or diagnostic change |
| Status | Independent review, 16 complete native MCP repetitions, Mac host, workspace/solver/E2E/PR gates pass; protected PR checks pending |

The test still starts actual subprocesses, uses the configured original CLI,
reads each response afresh and refuses a foreign job identity. The interpreter
helper already serves this module's responsiveness controls. No assertion,
timeout, expected response or authority check changes, and no runtime retry
is introduced. A missing endpoint never supplies cached proof authority.

The original failing log and every successful round remain outside git.
The observed error is established; its deeper kernel/filesystem cause is not
attributed. Separate ETXTBSY observations in a health-candidate no-reuse test
remain separately scoped; the twelve main no-reuse runs did not reproduce it.
This change claims no general operating-system or no-reuse-fixture fix.

At exact `0b66bcca3d4d1a963bad55cc10ffeceeb845f0f7`, the same native
Linux environment passes all sixteen complete MCP library suites (69 tests
each). The workspace gate passes 1,377 tests (53 skipped); solver passes 50;
complete E2E includes extra. Each of four PR conformance arms reports 33 Same,
311 functions, seven rlimit-drift units and zero different, unstable,
infrastructure or skipped-project results. Native EC2 Mac m4/us-east-1 host
clippy and all 1,364 non-solver tests pass (53 skipped).

Fresh independent review approves that exact source: the changed test, complete
69-test native MCP suite and explicit fresh-response/foreign-identity/missing-
endpoint control pass. Production source and every assertion are unchanged.
Review SHA-256:
`876801406b20d7da1624c8207906eb264ade2f5b43e085eb735b610c8b4a1976`.
Raw qualification logs, all sixteen repetitions and original conformance
captures remain outside git in archive SHA-256
`d6d6638be17307f3a60c7eb76d1124d29f09ea23638d1db7feecab164a3d84a5`.

No corpus entry or benchmark baseline changes. No new CLI/MCP command or
consumer-skill instruction changes; documentation impact is this focused
qualification record. Production implementation and user-visible release
behavior remain unchanged.

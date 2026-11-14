# Original Kani Rust payload

Status: contributor-only read-only fixture checker qualifies on native Linux
and receives independent approval at
`0ad5cd360bd7bb2f0b7ccfe6e5cbbeda96e9fa86`. Final checker, test and pin bytes match that reviewed source; the base also
includes the separately qualified attribution retirement. Tracks #712 G / ADR0149; no executable
backend, proof/currentness/cache, Mac or release acceptance is claimed.

The exact original Rust nightly channel file is SHA256
`3c30cc655a7549d0b30015cd032f949e8723da5ad039f672d04520bbaea4e76d`.
Six original official component archives contain 7,017 regular installed files,
1,586,050,167 bytes. The complete component identities and bounds are pinned in
`engine/kani/toolchain-linux.json`. Local static archive inspection is neither
Rust execution nor native backend acceptance.

With immutable original inputs in contributor-owned directories:

```sh
python3 ci/kani-toolchain.py --manifest INPUTS/channel-rust-nightly.toml \
  --archives INPUTS --out NEW-INVENTORY.json
python3 ci/kani-toolchain.py --manifest INPUTS/channel-rust-nightly.toml \
  --archives INPUTS --sysroot OWNED-RUST-PREFIX --out NEW-INSTALLED-INVENTORY.json
```

The output must not already exist. This reads every required original archive
and payload; it never runs `install.sh`, Rustup, Rust or Kani. Expected input
errors refuse with `SAIL6302`. Generated Rustup bookkeeping is explicitly
untrusted. The caller must retain immutable inputs; no hostile-executable or
concurrent-mutation guarantee is claimed. No public backend command is enabled.

Seven synthetic controls cover exact payload comparison, tampering, missing/extra
files, archive size/digest/count refusal, duplicate/noncanonical members, linked
and special files, installed links, and duplicate pin fields. The real native installation, exact compiler/tool version observations,
original-file negative controls, full contributor gate and independent review
pass at the exact source below.

## Retained attempts

| Source / attempt | Actual result |
|---|---|
| 32a82e06 / native1 | Docker refuses the nested target mount beneath a readonly source without the target directory; no checker or compiler runs. |
| 32a82e06 / native2 | Native original inventory matches all 7,017 files and private Rustup installs the six exact components. The wrapper then misuses rustup run with an isolated Cargo home lacking its proxy; this setup attempt fails and remains retained. |
| 32a82e06 / native3 | Invoking the actual isolated rustc/Cargo binaries directly reports the pinned identities. Complete installed-prefix comparison passes. This is compiler-payload qualification, not Kani execution. |
| 32a82e06 / independent review | Full native workspace gate and independent original/installed byte inventory pass, but a 32,001-byte deeply nested pin produces uncaught RecursionError. The source is rejected for that error-contract gap. |

The successor treats parser recursion refusal as the same stable SAIL6302
metadata error; its actual CLI regression requires exit 2, no traceback, empty
stdout and no output file. It does not admit the unsupported metadata. Fresh
successor qualification and independent review pass as recorded below.

| abea1844 / independent successor review | Full native gate and complete original/installed inventories pass, and the nested-pin refusal is fixed. A complete owned prefix with an unreadable extra directory is incorrectly accepted because os.walk skips the PermissionError. Original native non-root evidence remains rejected and retained. |

The next successor raises traversal errors instead of silently skipping a
subtree. Its small regression runs as an unprivileged reader (dropping only
the child process UID when the test runner is root), with a complete payload
and a real unreadable directory containing an extra file. Permission refusal
is required. No original payload bound or accepted-input rule is relaxed.
Fresh successor native qualification and independent review pass as recorded below.

The successor also restores SAIL6302 to the existing error table; the reviewed
blank-line rendering defect remains recorded with the rejected predecessor.

## Complete native qualification

Native c7i.4xlarge/us-east-2 qualification passes the unchanged complete local
workspace gate: fmt, Clippy, all 1,442 nextest tests (59 declared skips),
doctests, rustdoc, layering, dependency policy and benchmark-page freshness.
The seven focused payload controls have no skips. Independent actual decoding
and installed-prefix traversal confirm every one of 7,017 files,
1,586,050,167 bytes and executable bits against the six original archives and
exact official channel bytes. Actual isolated rustc/Cargo versions match the pin.
Generated Rustup metadata remains separately inventoried and untrusted.

Fresh review executes 47 primary commands, including 29 stable CLI refusals and
18 independent boundary controls. Both rejected predecessors reproduce their
original defects; the current native and CI environments refuse the same deep
JSON and unreadable owned subtree with exit 2, SAIL6302, empty stdout and no
output file or traceback. No payload bound or input rule is weakened.

All 187 original material members are independently lead-rehashed and externally
retained: archive SHA256
`9d195fda8cb193f261310ec32b2c333dce4fef2776bbd07e1a8807d84f9e2a19`;
index `303ae942320811088835687b673e3c0efc8edb10b3989ffa6d3e0f4670bfea1d`;
review `71e79664dd0c0f29b15eb8d51131ad04f22dcb1dec7f5d3085e0b8e6081936f3`.
Original mode-000 subtree metadata and bytes remain in their exact delta tar.
Installed bytes are represented by the complete authenticated original archives
and independently checked installed inventory, not a claimed closed backend.

The 32a rejection's 166 indexed originals plus embedded index remain retained
at archive SHA256
`1e3c3a1034a6453b451a069ed644103c93af3e81481730ec8d0c3845697acecb`;
the abea rejection's 202 originals remain at
`4f0d1bf14e342befe9d815f38c1cb5dd86f0b9434fd8873fe6606db172e097ec`.
The lead independently checks every material member of both predecessor archives.
Source/runtime verification behavior is unchanged by this contributor-only
metadata checker. No corpus entries change; solver/long conformance/model/Mac
suites are not implied. Final combined-base gate, hosted PR CI and post-merge
checks remain separate. Kani execution, complete source/tool/environment closure,
requested bounds, CLI/direct-arrayvec comparison and cache authority remain open.


The final combined base `f84a5ecc` also passes the full native Linux gate after
merging the current attribution-retirement main: 1,443 nextest tests, 59 declared
skips and every gate follow-on pass. Exact checker/tests/pins remain unchanged
from the independent approval. Hosted PR checks remain required before merge.

# Managed physical compiler argument reproduction

Status: confirmed default-mode conformance defect,
[Sail #393](https://github.com/Chelis-Lang/sail/issues/393). The
[corpus reproduction](../../corpus/repros/sail-managed-physical-time-argv/README.md)
uses an actual user procedural macro, normal Cargo-Verus project and selected
fork runtime. This is a failing correctness gate, not accepted production
qualification or a Z3 arithmetic counterexample.

## Source and runtime

The tested Sail binary comes from clean commit
`06f48beb635fdb07a8cec0d70b7fe3f7c62fa1cf`, tree
`1ecaebc01f295eb6fcc2197431d83b06221e8f6f`, built with Rust 1.99.0 on native
macOS/aarch64. Both verification arms use Rust 1.99.1 and the exact selected fork
`36d65c3f3161de34efa0150231a47c1777369a5a`; the macOS archive, manifest and full
runtime file inventory are validated against `engine/verus/artifact-macos.json`.
No fork candidate, runtime pin change or proof-mode change participates.

| Input | SHA-256 |
|---|---|
| Tested Sail binary | `6e0849dbc8d741cb6e3e11609fbb06a207415c348ed444b9621739717861e2a8` |
| Tested sail-z3 binary | `97fd2a87a31fe5048c0b65ecf865bd200c731f84c51b147939953940c474732e` |
| Selected fork archive | `be0d933322dabc8944d7ec39dd80946a20b3aeecc703a355257086b471249957` |
| Selected runtime manifest | `faa76d1b8319f4e7c674b8377449e06a67f17cfc78ba803d285ade7cf0ac5a25` |
| Actual rust_verify binary | `dec89bd8d2e53e76e04085a1f651fd15c29ed3b7c030c70fafefcf02a5221d57` |
| Actual Z3 binary | `edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb` |
| Tested flag producer source | `482932db04f42ec41f039202388ae463d21fb6c14a81dad2f118a1523fe1d722` |
| Tested native adapter source | `fc38b23096874f119745f268eb531a94e4545c46561518efe562a8f31163033c` |
| Actual macro source | `0809321b8544a19658393ea3b96c1e33841669f189589952f9c6bc37b99f129f` |
| Actual project lockfile | `8911ae6baca3e4a9ec23826614fb4b3abc489e2a93cb2b7bfd0aaf07ad8fec27` |

The flag producer file is unchanged at main `6d825736`; that establishes source
continuity only. No exact-main binary/runtime qualification is claimed.

## Results and controls

The macro emits `assert(false)` when physical process argv lacks `--time`, and
`assert(true)` when it finds that flag. The normal project and original user
arguments stay unchanged between each plain/Sail comparison. The counts below
refer only to `project::witness`, not the separately compiled vstd obligations.

| 2026-10-26 attempt | Plain fork | Actual Sail | Result |
|---|---|---|---|
| Existing global Cargo configuration | Exit 101; 0 verified/1 error | Exit 101; 0 verified/1 error; `SAIL3112` fallback | Safe fallback control; affected managed path untested |
| Isolated supported Cargo home, user `--output-json` | Exit 101; 0 verified/1 error | Exit 0; 1 verified/0 errors | **Failed conformance** |
| Same isolated home, user `--output-json --time` | Exit 101; 0 verified/1 error | Exit 101; 0 verified/1 error | Explicit-option transport control |

`UserFlags::sail_flags` injects a physical native `--time` only when the user did
not request it. Cargo-Verus's own option transport does not expose original user
`--time` in that physical argument vector. This explains why adding the original
user flag makes both arms reject rather than reproducing the injected case.
The same fork translates the macro's resulting proof normally in every arm.

The wrapper must preserve physical argv/environment, not only an internal
parser vector. Existing `--output-json`, timing and logging observations and the
unselected observer envelope need private in-memory configuration. Neither an
extra environment variable nor a self-described successful observation closes
this boundary. Cache keys binding exact SMT bytes cannot repair changed compiler
inputs that generate different SMT in the first place.

Raw commands, stdout/stderr, parsed JSON, explicit runtime identities and all
controls remain private. No inherited environment dump is recorded. The owner
retains the raw receipt/archive identity alongside the PR's overall acceptance
record. Native Linux, full corpus qualification and a corrected transport are
still required; this record does not close the acceptance deliverables.

The committed two-crate corpus fixture is also rerun from a separate scratch
copy: plain again exits 101 with zero verified/one error; actual Sail exits zero
with one verified/zero errors. Its explicit empty workspace and locked public
dependencies do not remove the discrepancy. The retained raw archive SHA-256 is
`b762c32425d6eb81b358681f6cfe5466fdb8e3896a8c9bb4e889f2aaef46e40c`;
its parsed receipt SHA-256 is
`91e61d49c20d47c5aa303b8131be0554a753e5a558926d222b1b15de7a08db00`.

# Error codes

Every error Sail reports carries a stable `SAILnnnn` code, printed as
`error[SAILnnnn]: …` or in a JSON `error.code`, `selection_error.code` or
`engine_fault.code` field. Scripts and agents may match on these codes.
`SAIL0899` and `SAIL0999` are internal errors: Sail reached a state it should
never reach. Report them with the full message and the command you ran. Sail
has stopped or fallen back without producing a wrong answer.

This page lists every code in the tools. A test
(`xtask/src/error_codes.rs`) fails when a code is used but not listed here, or
listed here but no longer used. Renamed and removed codes are listed in
`CHANGELOG.md`.

## Command line (`sail`)

| Code | Meaning | What to do |
|---|---|---|
| `SAIL0001` | `sail serve` needs Unix sockets | Run the daemon on Linux or macOS |
| `SAIL0002` | Usage error: unknown command, option or missing value | Fix the command line; `sail help` lists commands |
| `SAIL0003` | Cannot start `sail-bench`, serve on the socket, or run the daemon. As a warning from `sail verify`: the cache daemon is unavailable, so verification runs without the cache (same results) | `sail bench` is a contributor tool that is not in the release archives: build `sail-bench` from a Sail source checkout. Otherwise check that the socket path is free, writable and short enough (107 bytes on Linux, 103 on macOS; set `SAIL_CACHE_DIR` or `SAIL_DAEMON_SOCKET` to a shorter path). Run `sail serve` to see the daemon's own error |
| `SAIL0101` | Sail's Verus (the runtime built from Sail's Verus fork, which Sail runs for every verification and for `sail new`) is missing, does not match its pins, or its files changed. Sail never falls back to another Verus | Release archives bundle it under `engine/`: unpack the archive again and run its `bin/sail` without moving files out of the directory. In a source checkout, run `python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json --cache ~/.cache/sail` (`artifact-macos.json` on macOS) or `cargo xtask build-verus-engine`. `sail engine status` shows what is wrong |
| `SAIL0102` | `sail-z3` not found next to `sail` | Reinstall so `sail` and `sail-z3` share a directory |
| `SAIL0103` | Cannot find the Z3 that Verus uses (Sail's Verus bundles the pinned Z3) | Reinstall Sail, or set `VERUS_Z3_PATH` |
| `SAIL0104` | Cannot create the fragility trace directory | Check free space and permissions in the temporary directory, or pass `--keep-traces DIR` |
| `SAIL0105` | Cannot run `cargo-verus` (fragility measurement or portfolio) | Check Sail's Verus with `sail engine status` |
| `SAIL0106` | Unknown `--output` format | Use `--output json` |
| `SAIL0107` | Cannot create a scratch directory for JSON output | Check free space and permissions in the temporary directory |
| `SAIL0108` | Warning: `SAIL_CARGO_VERUS` replaces Sail's Verus. This development override is not Sail's default mode, and its results are not checked against stock Verus | Unset `SAIL_CARGO_VERUS` |
| `SAIL0110` | Invalid portfolio or fragility options | Fix the options; a portfolio and a fragility measurement cannot be combined, and neither takes `--background` or `--engine` |
| `SAIL0111` | A portfolio attempt could not run or was cancelled | Re-run; the message names the failing step |
| `SAIL0112` | Cannot retain portfolio or fragility evidence | Check free space and permissions under the cache directory |
| `SAIL0113` | Not every requested target produced a result | Read the report's diagnostics; the scope was not verified |
| `SAIL0115` | Portfolio mode needs the pinned Z3 | Point `VERUS_Z3_PATH` at the pinned Z3 |
| `SAIL0116` | `bench/portfolio/report.py` cannot render the portfolio probe | Check the retained probe files it names |
| `SAIL0130` | `sail new`: the path already exists and is not an empty directory. Nothing was changed | Choose a new path, or empty the directory |
| `SAIL0131` | `sail new`: the package name (from `--name`, or the path's last component) is not one Cargo accepts | Pass `--name NAME` with ASCII letters, digits, `-` and `_`, starting with a letter or `_`, and not a Rust keyword, `test` or `vstd` |
| `SAIL0132` | `sail new` could not create the project: Sail's Verus's `cargo verus new` failed, or a file operation failed. Nothing is left at the path | Read the message, which includes `cargo verus new`'s own error; check permissions in the parent directory |
| `SAIL0201` | `sail watch` failed | Read the message; restart `sail watch` |
| `SAIL0301` | `sail mcp` cannot start | Read the message; check the cache directory |
| `SAIL0302` | `sail mcp` cannot read stdin | The MCP client closed or broke the pipe; restart it |
| `SAIL0401` | No daemon is listening (`sail status`) | Start one with `sail serve`; `sail verify` starts one on demand |
| `SAIL0501` | `sail cache verify` found bad cache files | Run `sail cache verify --remove`; a bad entry is only ever a miss |
| `SAIL0502` | `sail cache clear` needs confirmation | Pass `--yes` |
| `SAIL0503` | Cannot clear the cache directory | Check permissions |
| `SAIL0504` | `sail cache projects` cannot read the managed `projects/` directory | Check permissions on `<cache>/projects` |
| `SAIL0505` | `sail cache projects prune` cannot remove a workspace's directory | Check permissions; a later prune removes anything left over, and no run uses it |

## Shared cache tier

| Code | Meaning | What to do |
|---|---|---|
| `SAIL0601` | Cannot generate a signing key | Check the destination directory and free space |
| `SAIL0602` | Cannot write a key or trust file | Check permissions |
| `SAIL0603` | Cannot read a public or secret key file | Check the path and that the file is a Sail key |
| `SAIL0604` | Cannot remove a trusted key | Check permissions |
| `SAIL0605` | Some entries could not be published | Re-run `sail cache publish`; written entries remain |
| `SAIL0650` | HTTP shared-cache client error (connect, trust, token or response) | Check the endpoint, token, CA and trusted key files; a failed lookup is a miss, never a wrong answer |
| `SAIL0651` | HTTP shared-cache server cannot start or stopped | Check the bind address, store directory and `--behind-tls-proxy` for non-loopback binds |
| `SAIL0652` | Some remote publications failed | Re-run; completed uploads remain available |

## Jobs and the project service

| Code | Meaning | What to do |
|---|---|---|
| `SAIL0800` | The job service cannot start, or local jobs need Unix sockets | Read the message; check the socket directory |
| `SAIL0802` | Job RPC protocol error: malformed, oversized or incompatible frame, or an unexpected response | Use the same `sail` version for the CLI, MCP and daemon; restart the daemon after upgrading |
| `SAIL0803` | A job endpoint is already running on this socket | Use the running daemon, or stop it first |
| `SAIL0805` | Unknown job ID | Check the ID with `sail jobs list` |
| `SAIL0806` | Invalid project directory or arguments in a job submission | Pass an existing directory; arguments cannot contain NUL |
| `SAIL0807` | The daemon restarted before the job completed | Submit the job again |
| `SAIL0808` | The job failed, or its result could not be saved | Read the message; check free space under the cache directory |
| `SAIL0810` | Cannot start the job daemon on demand | Read the message; try `sail serve` directly |
| `SAIL0811` | `sail jobs` usage error | Fix the command line; see the usage text |
| `SAIL0812` | Managed jobs need UTF-8 environment names and values | Remove or fix the non-UTF-8 variable |
| `SAIL0813` | An MCP job tool failed | Read the message; the CLI equivalent shows more detail |
| `SAIL0814` | A result exceeds the RPC size limit | Read the retained files in the private job store |
| `SAIL0815` | The job service is stopping or its queue is full | Retry later |
| `SAIL0816` | A retained result no longer matches its seal | Re-run the job; the stored output was changed or damaged |
| `SAIL0899` | Internal error in the job service | Report it with the message |

## Proof events, evidence and baselines

| Code | Meaning | What to do |
|---|---|---|
| `SAIL0902` | The proof event stream has a gap, or a cursor is invalid | Resume from a valid cursor; the final result stays authoritative |
| `SAIL0903` | No interactive proof stream for this job, or the endpoint does not support proof contracts | Use `--engine interactive`, or upgrade the daemon |
| `SAIL0904` | The requested target, function or obligation evidence is missing or ambiguous | Wait for the job to complete, or name the exact target and function |
| `SAIL0905` | Invalid, duplicate or unreadable baseline | Use a new name of 1–80 letters, digits, `.`, `_` or `-` |
| `SAIL0906` | Usage error in `explain`, `compare`, `baseline` or `--engine` | Fix the command line |
| `SAIL0907` | An MCP proof tool failed | Read the message |
| `SAIL0930` | The `actual_work` research observer cannot validate its arguments or complete current-run evidence | Pass a successful Current result and its matching retained run directory; missing or damaged evidence cannot count as zero solver work |
| `SAIL0944` | `ci/engine-artifact.py` rejected the engine pin or artifact | Fix `engine/verus/fork.toml` or the runtime artifact it names |
| `SAIL0999` | Internal error in the proof stream | Report it with the message |

## SMT-LIB and the solver shim (`sail-z3`)

| Code | Meaning | What to do |
|---|---|---|
| `SAIL1001` | Malformed SMT-LIB input (internal parsing; the shim still forwards the bytes unchanged) | Report it if it appears in output |
| `SAIL2001` | `VERUS_Z3_PATH` is not set | Point it at the pinned Z3 |
| `SAIL2002` | `VERUS_Z3_PATH` cannot be resolved | Fix the path |
| `SAIL2003` | `VERUS_Z3_PATH` points at `sail-z3` itself | Point it at the real Z3 |
| `SAIL2004` | Cannot start or keep track of the solver | Check the Z3 binary; re-run |

## Verus runs and corpus tooling (`sail-conformance`, `sail-bench`, `cargo xtask`)

| Code | Meaning | What to do |
|---|---|---|
| `SAIL3001` | No cache directory: none of `SAIL_CACHE_DIR`, `XDG_CACHE_HOME`, `HOME` is set (also reported by `sail verify`) | Set one of them |
| `SAIL3002` | Cannot read or parse the toolchain pins | Restore `corpus/toolchain.toml` |
| `SAIL3003` | No pinned Verus asset for this host, or it is not installed | Run `cargo xtask fetch-verus` |
| `SAIL3004` | Unknown Verus release | Use the primary or a secondary release in `corpus/toolchain.toml` |
| `SAIL3010` | Unknown corpus tier | Use `fast`, `standard` or `deep` |
| `SAIL3011` | Cannot read or parse a corpus file | Fix the file named in the message |
| `SAIL3012` | Cannot list a corpus directory | Check the path |
| `SAIL3013` | The corpus was validated against a different Verus pin | Re-validate the corpus or use the matching pin |
| `SAIL3014` | `git` failed while fetching a corpus project | Check network access and the pinned revision |
| `SAIL3015` | Unsupported corpus build settings | Fix the project entry in the corpus file |
| `SAIL3016` | Cannot patch a workspace manifest to the pinned Verus source | Check the project's `Cargo.toml` |
| `SAIL3020` | Cannot prepare or start a Verus run | Check the scratch directory and the Verus install |
| `SAIL3021` | The timing wrapper failed to run Verus | Check the Verus install |
| `SAIL3022` | A Verus run exceeded its limit or its scratch copy failed | An infrastructure error, never a verdict; re-run or raise the limit |
| `SAIL3030` | Shim not found, or a Verus log directory cannot be read | Build `sail-shim` or pass `--shim` |
| `SAIL3040` | Missing or incomplete Verus source build | Run `cargo xtask build-verus-source` |
| `SAIL3041` | Cannot find the toolchain sysroot for a source build | Install the Rust toolchain Verus pins |
| `SAIL3042` | A `rust_verify_test` binary cannot list its tests | Rebuild the source build |
| `SAIL3043` | Another process owns the source build | Wait for it, or remove a stale lock |

## Managed verification (`sail verify`, jobs, watch, MCP)

| Code | Meaning | What to do |
|---|---|---|
| `SAIL3100` | `sail verify` cannot run this project: Sail's Verus is missing (the message then carries `SAIL0101`), or managed state cannot be set up | Read the message; `cargo verus verify` runs the project without Sail |
| `SAIL3101` | Cannot read managed state | Check permissions under the cache directory |
| `SAIL3102` | Cannot write managed state | Check free space and permissions |
| `SAIL3103` | `cargo metadata` failed | Fix the error Cargo reports |
| `SAIL3104` | Cannot identify the toolchain, or the target stayed in use past `SAIL_MANAGED_LOCK_WAIT_SECS` | Check the Rust and Verus installs; wait or raise the limit |
| `SAIL3105` | Cleaning the edited package failed | Fix the error Cargo reports |
| `SAIL3106` | Cannot run Cargo or capture its output | Check the Cargo install |
| `SAIL3107` | Verification produced no complete proof receipt for a target | Re-run; the result is not reused |
| `SAIL3108` | No Current run is recorded for this project and settings | Verify once; later reads can then be Current |
| `SAIL3111` | The managed adapter failed (solver capture, receipts or Cargo wrapper) | Read the message; re-run |
| `SAIL3112` | A setup managed verification cannot bind to a proof receipt: `--target-dir`, Verus `--log` options, `--no-verify`, `-v`, `--message-format`, `--sysroot`, tests, benches or examples, Cargo `--config` or configuration Sail cannot identify, `RUSTC_WORKSPACE_WRAPPER`, a loader variable (`LD_LIBRARY_PATH`, `LD_PRELOAD`, `DYLD_*`), or no target with `verify = true`. A `note` from `sail verify`, a job, `watch` or MCP: the run is verified as plain `cargo verus verify` verifies it, without proof reuse; a job's project result carries `no_reuse` and a `SAIL3112:` note and is never `current`. Still an error from `--engine interactive` | Nothing. Remove the setting to get proof reuse |
| `SAIL3114` | Inputs could not be confirmed unchanged; the result is not Current | Re-run for a current result |

## Interactive engine (`--engine interactive`)

| Code | Meaning | What to do |
|---|---|---|
| `SAIL3120` | The engine is not installed, does not match its pin, or the request is invalid | The message says what is missing or mismatched. Release archives bundle the engine under `engine/`, so from a release install this means the unpacked directory is damaged or incomplete: unpack the archive again and run its `bin/sail` without moving files out of the directory. An engine in the Sail cache (`<cache>/verus-src-build/`) is used before the bundled one; remove a stale one. In a Sail source checkout, build it with `cargo xtask build-verus-engine` (no credentials needed) or download the prebuilt runtime with `python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json --cache ~/.cache/sail` (`artifact-macos.json` on macOS); or run `sail engine install --from PATH`. Repairs need one exact crate-qualified function |
| `SAIL3122` | The selected function was not found, or not exactly once | Name the exact function |
| `SAIL3123` | The engine faulted; the result is from stock Verus and accepts no repair | Use the stock result; report the fault if it repeats |
| `SAIL3124` | A target needs a Verus feature the engine is built without (for example `singular` for `by(integer_ring)`); it was verified by the configured stock Verus and accepts no repair | Use the stock result, or stock `sail verify`; see [Verus features the engine lacks](proof-repair.md#verus-features-the-engine-lacks) |
| `SAIL3130` | Note: a dependency (such as vstd) was verified by a run in another workspace and placed in this new project from Sail's [dependency store](decisions/0045-dependency-store.md) | None; plain Cargo-Verus would have verified it in this run, and its recorded report is the one shown |
| `SAIL3131` | Warning: a dependency store entry is damaged (its checksums or digests fail), so it was ignored | None; the dependency is verified normally. Delete `<cache>/dependency-store/` if the warning repeats |
| `SAIL3132` | Research note: a complete dependency proof was replayed at the vstd compiler boundary, with its original establishing invocation | The unselected [compiler-boundary experiment](validation/vstd-compile-boundary.md) records whether the proof was established locally or in a release build; this does not enable the quarantined native-artifact store |
| `SAIL3134` | Research dependency recording is incomplete, or checked restoration could not remove a partial output | Read the specific message. Incomplete recordings are unusable; a partial-restoration cleanup failure requires a fresh owned target |
| `SAIL3135` | Both compiler-reuse research switches are enabled; this combination is unsupported and refused before setup or child execution | Unset either `SAIL_VSTD_BOUNDARY_RESEARCH` or `SAIL_RETAINED_CONTENT_RESEARCH`; see the [integration limits](validation/proof-reuse-integration.md) |
| `SAIL3136` | Note: ordinary retained reuse (instant revert) is off because the valid engine in use is not the runtime Sail's ordinary reuse is qualified with on this platform (its manifest digest differs). Verification is unaffected | None for verdicts. Use the pinned runtime (`engine/verus/artifact-linux.json` or `artifact-macos.json`) to get instant revert; see [instant revert](validation/instant-revert.md) |
| `SAIL3137` | Note: the release ships a preverified vstd proof, but Rust's pinned toolchain (`pins.verus.rust_toolchain`, currently `1.99.1`) is not installed with `rust-src`, so managed verification ran the default Cargo and verifies vstd again ([ADR 0058](decisions/0058-managed-builds-use-the-pinned-cargo.md)). Verification is unaffected; Sail installs nothing | Run the command the note names, `rustup toolchain install 1.99.1 --profile minimal --component rust-src`, to use the shipped proof |
| `SAIL3138` | Note: the release ships a preverified vstd proof, but it did not apply to this new target, so the run verified vstd itself. The note names the first reason found: a `RUSTC_WRAPPER`-style variable; a Cargo configuration or flags variable that can change compilation; a vstd that is not from crates.io; an `LD_`/`DYLD_` variable; an unnameable Cargo home or target path; missing standard-library source for its diagnostics; Verus flags after `--` that reach vstd, other than one raised `--rlimit` ([ADR 0060](decisions/0060-shipped-vstd-under-a-raised-rlimit.md)), or a `--rlimit` below the default 10; extra components (such as `rustc-dev`) on the pinned toolchain, whose libraries rustc can load ([#264](https://github.com/Chelis-Lang/sail/issues/264)); vstd dependency versions that differ from the shipped build's; or another installed toolchain or Verus. Verification is unaffected, and output is otherwise unchanged | Remove the named cause to use the proof, for example `rustup component remove rustc-dev --toolchain 1.99.1`, or pass `--fwd-verus-args-to roots` before `--` to keep Verus flags off vstd. A project whose lockfile resolves vstd's dependencies differently cannot use it ([#238](https://github.com/Chelis-Lang/sail/issues/238)) |

## Portfolio and solver libraries

| Code | Meaning | What to do |
|---|---|---|
| `SAIL5001` | A solver process could not be started or driven | Check the Z3 path |
| `SAIL5101` | A portfolio file or directory cannot be read | Check the path |
| `SAIL5102` | Invalid portfolio options | Fix the options, for example a non-zero number of configurations |

## Benchmarks (`sail-bench`)

| Code | Meaning | What to do |
|---|---|---|
| `SAIL7001` | Usage error, or not the frozen repair catalogue | Fix the command line or catalogue path |
| `SAIL7002` | Cannot read or write a benchmark file | Check the path |
| `SAIL7003` | A trace line is not a valid trace record | Re-capture the trace |
| `SAIL7004` | The solver could not be started or driven | Check the Z3 path |
| `SAIL7005` | Results could not be serialised | Report it |
| `SAIL7101` | Repair gate: an invocation failed, timed out or was not recorded | Read the sample's stderr under `--out` |
| `SAIL7102` | Repair gate: full evidence was not Current | Re-run on an idle host |
| `SAIL7103` | Repair gate: more than one compiler driver per batch | Investigate the batch |
| `SAIL7104` | Repair gate: not a real source-engine batch | Check the engine install |
| `SAIL7106` | Repair gate: outcome differs from the catalogue | Investigate the cycle |
| `SAIL7107` | Repair gate: M6 and M7 full outcomes differ | Investigate the pair |
| `SAIL7108` | Repair gate: too few complete rounds | Run more rounds |
| `SAIL7109` | Repair gate: median speed-up below the target | A failing result; record it |
| `SAIL7110` | Repair gate: invalid M6 baseline sample | Read the baseline's stderr |

## Native benchmark host helper

| Code | Meaning | What to do |
|---|---|---|
| `SAIL9001` | `scripts/perf-host.sh down` could not verify termination or complete resource cleanup; recorded lane files are retained | Read the AWS error, refresh authentication or permissions as needed, then retry the same `down LANE`. A successful waiter must also be followed by a confirmed terminated/absent instance before resource deletion |

`SAIL3134`: the disabled retained-content experiment could not remove a
partially restored output. The attempt refuses; do not treat it as a cache miss
or reuse a receipt. A fresh owned target is required before retrying.

# End-to-end output failure evidence

The `output`, `flags` and `loader` CI jobs retain an
`e2e-output-evidence-<phase>` artifact when their original output check fails.
The same observer covers the local `extra` phase. Comparisons, verifier
arguments, warmups and test exit codes are unchanged. A collector failure
prints its own error and preserves the original failing exit code.

On a native Linux host, run the ordinary check:

```sh
ci/local.sh e2e output
```

Evidence defaults to `target/e2e-output-evidence`, outside the temporary test
project. `SAIL_E2E_OUTPUT_OUT` selects another path inside the container; a
custom path must be on a bind mount to survive container removal. Relative
paths resolve from the harness caller's working directory before fixture setup.
With `ci/local.sh`, choose a container path under `/work` (for example,
`/work/target/e2e-output-evidence`); arbitrary host paths outside the bind mount
are not translated or mounted automatically. This variable
is removed before invoking either verifier, so the observer destination is not
a proof input. Passing checks remove their fixture and create no failure
artifact.

Each failure directory contains `index.json` and selected original files under
`files/`: stdout/stderr, recorded child exit codes, NUL-delimited argument
vectors, working and target directory observations, source/manifests at command
start, and available durable producer reports, invocation records and sidecar
streams. Workspace member sources and manifests are included in command-start
snapshots. Command finish snapshots preserve raw stderr before the loader
check's existing note filtering. Start/finish timestamps and producer paths
are observations; they do not reconstruct cross-stream write order or establish
a unique command-to-run mapping.

The collector retains at most 512 selected files, 8 MiB per file and 64 MiB
in total, with a 60-second deadline. During the test, duplicate command
snapshots have a separate shared 32 MiB ceiling and the same 8 MiB per-file ceiling.
An incomplete command snapshot writes a bounded reason, relative source path,
source/retained sizes and truncation/change flags to the outer log, so passing
fixture cleanup does not erase its cause. Payload bytes are not printed.
Truncation, changed files, missing executable identity and collection errors
are indexed as incomplete capture. Original fixture streams may be larger than
the retained prefix. Targets, compiler artifacts, dependency-store payloads,
executable copies and environment dumps are excluded. The Sail executable is
identified by a bounded hash read only. Inspect omissions and `capture_complete`
before drawing conclusions from an artifact.

The original #174 combined-output failure remains unresolved. Its controlled
three-arm reproduction did not reproduce the disagreement and did not qualify
the memo candidate. See the [validation record](validation/e2e-output-evidence.md).

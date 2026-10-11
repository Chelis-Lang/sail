# Late imported-authority complete source controls

The unselected late-handoff candidate passes the actual pinned complete source
controls: 40 VIR tests, 78 driver tests and both strict all-target Clippy checks.
The [machine-readable record](m7-late-import-handoff-source-controls.json)
contains every command, phase duration, raw artifact digest and tested ELF
identity. The complete controller took 180.872 seconds in the Linux container
under Docker Desktop emulation.

This tests full patch
`bb9cf6ab85b3316b5a90551dd615864bd564b5e052c0d767fa2b97fb5ba84372`
and delta
`0cd5dfe76862acc95cc726e66be1fca3ae3b84938d3478955136206b6be83b55`
from the frozen original-descriptor candidate. All seven source hashes and the
lockfile match before and after the checks. The complete patch reverses cleanly
against the pinned source. The [independent review](m7-late-import-handoff-independent-review.md)
describes the authority, namespace and allocation boundaries.

The private controller reused the existing closed target, with compiler jobs
two, independent test cases serial and ordinary priority. It removed inherited
engine selectors and used the same pinned 1.98.1 compiler configuration as the
earlier source controls. The commands, in order, were:

```sh
cargo +1.98.1 test -p vir --release --lib sail_ -- --test-threads=1
cargo +1.98.1 clippy -p vir --release --all-targets -- -D warnings
cargo +1.98.1 test -p rust_verify --release --lib sail_ -- --test-threads=1
cargo +1.98.1 clippy -p rust_verify --release --all-targets -- -D warnings
```

No phase failed. Tested binaries, dependency identities, exact raw logs,
controller and source backup are retained. Both older runtimes retain their
complete 13-artifact seals. Command, working-directory and file-descriptor
ownership scans in both containers found no holders or unreadable processes
before mutation and after closure. The private source and target were returned
to the runtime-build owner only after closure and preservation.

These source controls do not establish a cross-edit hit, source qualification,
performance or candidate selection. The original 0bbc revision-zero failure
remains zero hits with four provenance refusals despite equal stock verdicts
and diagnostics. Its separate trace observed a namespace growing from zero
Foreign hash matches to one, and correctly refused the old immutable owner.
The new late classification still needs an ordinary runtime build, fresh vstd,
the unchanged fourteen-stage qualification and actual previous-revision focus
reuse. The original diagnostics, ordered wire/effects checks and 600-second
source deadline remain required. The prospective 500-MiB preparation argument
is separate from ordinary compiler RSS and product latency.

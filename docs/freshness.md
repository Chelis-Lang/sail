# Freshness of retained workspace proof reports

A proof report and a claim that it describes current inputs are separate facts.
The daemon retains establishing reports even when current input coverage is
unknown. `Current` requires complete declarations and coherent content validation;
`Historical` records changed inputs. `Unknown` never authorizes proof reuse.

On Linux, the daemon keeps at most 32 process-local context projections and runs
at most two background revalidations. A context binds its cache root, workspace
spelling, runtime identity, invocation configuration and explicit environment.
Restart begins with no trusted projections. Overlapping readers may share a coherent
snapshot only when its validation completes within each reader's own call interval;
sequential reads never reuse it by age. Publication and read completion require
the same runtime-generation identity and native barriers. Hash work happens outside job-service
and verifier-worker locks. Established reads retain the existing five-second RPC
deadline; uncertain readers return promptly with Unknown and an explanatory note.

The same coherent guard brackets the managed verifier invocation: leases and
native coverage begin before its establishing content snapshot and remain held
through verification and post-run hashes. A preexisting writable mapping,
conflicting write, replacement or lost coverage makes the result Unknown and
prevents current receipt publication. Fresh Rust verification still runs.

A projection combines two independent mechanisms:

- Kernel read leases on every existing declared regular source, configuration,
  build and runtime file. The leases are acquired before baseline hashes and must
  still be held at each read barrier. Expensive runtime digests can then be reused;
  mutable source/configuration/build contents are nevertheless rehashed on each
  inspection. A conflicting write or truncate releases the projection immediately.
- Direct native watches for files, directories, aliases, ancestors and absent
  configuration paths. They detect replacement, newly created inputs and transient
  configuration. Reads drain the kernel queue synchronously; no callback-delivery
  delay is interpreted as absence. Mount-table changes, lost watches, overflow,
  unsupported filesystems or a bounded barrier that cannot complete invalidate it.

Inotify alone is insufficient. It cannot see writes through existing shared memory
mappings. Directory watches also miss writes through an outside hardlink unless
its inode is watched directly. See the executable reproduction in
`corpus/repros/linux-inotify-mmap/`. Read leases are rejected while a conflicting
writable file description or shared mapping exists, including a mapping that
survives closing its originating descriptor. Ordinary read-only mode bits or a
read-only bind mount do not supply that guarantee.

The optional lease owner is an embedded Python 3 standard-library helper launched
with isolated module loading, structured arguments and private bounded socket I/O.
Sail resolves an absolute interpreter once per establishment and binds its
loaded standard-library source/bytecode files and mapped ELF libraries. The
cold manifest and content hashes precede launch; the helper checks its actual
loaded closure before and after lease acquisition, and parent hashes must still
match afterward. Direct watches and read leases retain this closure. Changed
PATH resolution, replacement, new bytecode, or a broken runtime lease invalidates
the projection. These exact dependency digests also join the managed context.
The native kernel and the installed interpreter/ELF-loader bootstrap are the
trusted OS boundary: self-reported module maps cannot attest a malicious bootstrap.
No Python package or loader supplied by an untrusted party becomes trusted by
printing a manifest. It uses no third-party modules, owns at
most 4,096 file descriptors, and exits immediately on SIGIO to release leases so
an editor can write. No unsafe Rust or FFI is introduced. Missing Python, denied
lease ownership, unsupported kernel/filesystem behavior, an existing writable
mapping, helper loss or a timed-out check disables the fast path. Unprivileged
leases require matching file ownership; system-owned runtime files may prevent
establishment. MacOS retains conservative content revalidation.

The fast path accepts only supported local filesystems. A host-shared mount such
as virtiofs, or a network filesystem whose event/lease coverage is uncertain, does
not become trusted because a test happens to pass. Container validation copies
verification sources, binaries and runtime files onto Linux-local filesystems.
Container timings under emulation are labeled as such; they are not native Linux
reference speedups.

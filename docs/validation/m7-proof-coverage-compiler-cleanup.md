# Closed proof-coverage compiler scratch cleanup

The completed private diagnostic target reclaimed **1,419,780,096 allocated
bytes** from its `debug/incremental`, `debug/build` and `debug/deps` directories.
Before removal, the three actual tested ELF executables were copied to a separate
retained evidence directory and checked against the previously recorded hashes.
A complete live process command/cwd/FD ownership check found no holder and no
unreadable live process. All seven raw logs and all three retained executable
hashes matched after removal.

The [machine-readable cleanup record](m7-proof-coverage-compiler-cleanup.json)
retains exact relative removed names, allocated bytes and evidence identities.
The [original diagnostic results](m7-proof-coverage-diagnostics.md) remain separate.
Only regenerable private compiler intermediates were removed. Source, raw
records, tested binaries, the common gate target, original standard run,
prepared project targets and all pinned runtimes were preserved. Future private
compilation must recreate its dependencies; no test or qualification was rerun.

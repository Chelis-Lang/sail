# Closed host compiler retirement

The owned `m7-sysroot-reuse/target/debug` subtree was retired after checking
process command paths and open files twice. Both open-file checks found no
holders. Six runnable host test executables were copied and their SHA-256
digests checked before deletion; their names, sizes and hashes are recorded in
[the inventory](m7-closed-host-compiler-retirement.json).

The removed subtree contained 1,750,315,896 logical bytes across 10,111 files.
The retained executable copies contain 41,349,384 bytes. These are file sizes,
not a measurement of physical disk space reclaimed.

The active native retained-proof workspace, daemon, cache and runtime were
untouched. Original diagnostic logs, controllers, sealed product binaries,
source inventories and recorded failure evidence were retained. No shared
compiler target or another project's files were removed. This cleanup does
not change any validation result.

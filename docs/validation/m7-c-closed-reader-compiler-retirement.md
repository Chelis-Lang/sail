# Closed reader compiler intermediates retired

Only the `debug` and `release` subtrees of the closed `m7-reader-cb07` and
`m7-result-client` private target families were retired. Their initial unique
regular-file allocation was 2,297,212,928 bytes. Every discovered executable or
shared-library ELF image was moved outside those subtrees and rehashed before
and after deletion: 149 paths, with 939,417,600 unique allocated bytes retained.
The retired compiler allocation is 1,357,795,328 bytes (about 1.26 GiB).

Both Linux containers passed initial and immediate predelete holder checks for
each subtree. These bind process births and inspect bounded command, working
directory, maps and descriptor metadata, including exact target file inodes.
All checks found zero holders and zero unreadable or ambiguous live processes.
The host command-metadata check also found zero references; native VM descriptor
and map checks come from the two actual Linux namespaces. The raw limits remain
8,192 processes, 1 MiB command reads, 2 MiB map reads and 4,096 descriptors per
process. No process environment was disclosed and no process was signalled.
Both containers confirm the retired subtrees are absent and retained images are
present. The [portable record](m7-c-closed-reader-compiler-retirement.json) binds
every retained image SHA and the complete original file inventory.

Evidence, component, final and rebuilt-result records outside those compiler
subtrees remain intact. The original c77 runtime and source, M6 and current
products, corpus pins, all reader raw records and proof caches/receipts, and
future benchmark inputs remain protected. The common Cargo target and other
workstreams were untouched. No proof or benchmark actor was launched.

These numbers count unique file inode allocations. Host free space was 5.4 GiB
afterward; concurrent filesystem changes prevent attributing that entire change
to this retirement. This is stale-build cleanup, not product validation or
performance acceptance.

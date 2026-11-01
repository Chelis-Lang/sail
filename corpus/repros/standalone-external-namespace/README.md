# External inline-module namespace invalidation

A standalone source outside the usual module tree can use inline modules whose
intermediate directories contain no read source file. Merely observing the
parent of `leaf/mod.rs` misses a new competing `leaf.rs` in its parent directory.

From this directory, run the pinned tools with a fresh Sail cache:

```sh
cd work
verus proof.rs
sail verify proof.rs --output json
sail verify proof.rs --output json
sail verify proof.rs --output json
cp ../external/inner/leaf/mod.rs ../external/inner/leaf.rs
verus proof.rs
sail verify proof.rs --output json
rm ../external/inner/leaf.rs
```

The initial calls succeed. After the copy, stock reports Rust error E0761 and
exits 1. Sail must execute fresh verification, preserve that complete diagnostic
and exit 1. The initial candidate reused a Current successful proof and exited
0. The regression was reproduced on native Linux with the `5575a925` feature
binary and qualified `0ad405eb` runtime. The correction observes every source
ancestor directory through the workspace boundary or filesystem root, including
those introduced only by inline module structure. It adds no content trust.

The native standalone acceptance suite checks a real receipt hit before adding
the competing file, then exact stock diagnostics and a fresh refusal afterward.

The same acceptance also checks the reverse layout (adding `leaf/mod.rs` to an
already existing empty directory), an unread symlinked module directory, and
a broken `leaf.rs` link becoming a file. Namespace observations bind followed
entry kinds and the `mod.rs` candidate under directories, as well as ancestor
entries and actual read-file contents. Version 2 separates corrected contexts
from earlier receipts.

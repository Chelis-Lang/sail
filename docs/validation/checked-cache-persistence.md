# Cache persistence qualification

Status: unqualified source candidate. No fresh Stock, native build, solver,
conformance or cost result is claimed.

| Scope | State |
|---|---|
| Base | Command-classification candidate `155d3766`; its qualification remains separate |
| Existing work extracted | Transaction and native host from `1df34cb1`, with only the record-helper namespace relocated; original record operations and roundtrip witnesses retained |
| Current production calls | `DiskCache::put_scoped` and `parse_entry` use the shared transaction and record split |
| Source guard | Exact host contract, checked scripted implementation, native bindings and separate syntax module; private transaction-result fields |
| Proof inventory | Complete `persistence` root awaits actual discovery; no guessed verified count or function inventory |
| Failure controls | Three record-framing and six transaction controls; all ordinary Stock outcomes pending |
| Source checks | `f89a5b6f`: all 72 source tests pass; all 278 complete programs assemble, and the 268 prior programs remain byte-identical to `155d3766` |
| Source review | Native put/parse correspondence and the explicit host boundary reviewed; subsequent import and automatic-build-script guards only strengthen source admission |
| Required before selection | Direct Stock discovery and controls, native gate, solver faults, full end-to-end, full fast/standard conformance and added-cost measurement |
| Limits | Native I/O, path resolution, durability, store initialization and truthful solver origins remain unproved |

The historical separator mutation descriptor used an overescaped string that
did not match the Rust source. The candidate matches the actual newline push
and substitutes a carriage return; its intended fault is unchanged. This is a
correction within the unmerged proof work, not a current-main product bug.

The [design](../design/checked-cache-persistence.md) distinguishes shared checked
operations from the native host assumptions. A completed write is not a proof
that its payload is a truthful solver answer.

The canonical source-comparison report for `f89a5b6f` has SHA256
`1e35f541b0cd9d4b122e5b0809f260c93392270809a121575eced99f402b24bc`.
It compares complete generated drivers and dependency bytes, including every
prior failure control, and confirms that pending inventories refuse normal
qualification. This is source correspondence, not execution evidence.

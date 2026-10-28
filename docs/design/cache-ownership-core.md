# Cache ownership core

This staged core contains the cache data operations checked directly by stock
Verus. The native memory tier, disk writer, daemon, cache format, solver keys and
default verdict path keep their existing implementations. Native adoption is a
separate change. The [acceptance design](evidence-acceptance-core.md) and its
single [work tracker](https://github.com/Chelis-Lang/sail/issues/368) retain the
unfinished service proof.

The seven modules own real Rust records and collections:

| Module | Checked operation |
|---|---|
| `cache_candidate` | Checked local insertion generations; original key/value/endpoint/ticket transfer; Written/Exists/error transitions; declared counter arithmetic |
| `cache_bucket` | Exact little-endian route packing and complete-key bucket operations; the earlier Vec bucket is not selected by native storage |
| `cache_chain` | Real Box-chain lookup, insertion, removal and recency updates; iterative tail-detached destruction |
| `cache_slots` | One actual primitive-key HashMap of complete-key collision chains; placement, uniqueness and original-entry framing |
| `cache_counted` | The same map with an actual checked entry counter equal to its stored rows |
| `cache_order` | Actual BTreeMap ownership transfer and least-original-tick selection |
| `cache_indexed` | One private counted map and its actual full-key/recency index, including dense ordered renumbering |

A route uses the first 16 key bytes; every identity decision compares all 32
bytes. Different full keys may share a route. No collision-freedom or array-key
hash-model assumption supplies the result. Lookup, retirement, replacement and
touch preserve the original colliding neighbors.

Generation freshness is local to one Clock. Equal bytes or reuse of the same
payload do not reuse an issued generation. Completion compares the original key,
generation and Pending state before changing that same owned entry. The native
endpoint domain and the fact that one original Clock remains installed under
that endpoint require the separate caller/lock refinement; a local ordinal is
not a globally unique process or allocation identity.

The counted/indexed records have private fields and empty-only constructors.
The source gate closes their imports, constructor-bearing records and exact
runtime contracts. Full-body mutation descriptors retain signatures, contracts,
checked loop annotations, proof groups and their runtime intervals. Source
refusal, compilation failure, timeout or resource exhaustion cannot qualify an
ordinary negative. The registry keeps all 60 candidate-generation/accounting/
order controls and all 72 indexed-map controls as 132 distinct required cases.
They join the original seven query controls in the existing local-only production
refinement suite; inputs and authority retain their own complete roots. No new
runner, CI workload, corpus selection or verifier resource override is introduced.

The syntax addition retains the old expansions, supports the existing checked
Copy bound, and adds proof snapshots plus fixed no-unwind/closed-invariant
method and Drop envelopes. Native and Stock profiles use the same executable
bodies. No host-trait interface, assumed specification, unsafe code, new crate
edge or source-token memoization is added by this extraction.

The main-context qualification records its own complete observed function and
obligation inventories. The earlier larger branch's input module differs;
its 355-function tally is not transplanted. Every cache operation, constructive
witness and control is retained, and the one imported `inputs::byte_eq` body and
contract match main exactly. The [validation record](../validation/cache-ownership-core.md)
separates that historical evidence from this delivery's scoped qualification
and remaining merge requirements.

A successful candidate transition does not prove a solver produced its claimed
reply. Arbitrary PUT, initial disk contents, restart, shared/bundled writers,
physical persistence, full input closure and service composition remain open.
No origin-admission architecture is selected here. The later Resident experiment
is frozen separately and is not part of this core or a native selection.

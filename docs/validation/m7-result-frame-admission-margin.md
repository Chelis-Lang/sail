# Complete-result fixed admission headroom

Result-cache admission now reserves fixed storage inside the existing 128 MiB
ceiling. It also releases the two unselected currentness variants when quota
pressure returns an uncached response. Complete response bytes, freshness checks,
schemas, deadlines and the endpoint's 32-client limit are unchanged.

The original [capacity and output ownership controls](m7-result-frame-ownership.md)
remain useful. Independent review found a narrower gap: Rust drops the fields
of `Frames`, including its charge, before freeing the outer Arc allocation.
An allocation admission can therefore follow a refund while that outer block
still exists. Pinned Rust also retains a BTree leaf root when its last entry is
removed. These are storage-accounting gaps; no verdict or latency failure is
attributed to them.

The pinned Linux layout reserves 6,992 bytes:

| Allocation | Reserved bytes |
| --- | ---: |
| Empty index root (audited conservative node bound) | 1,024 |
| Inline cache/index owner | 48 |
| Budget value and Arc header | 32 |
| 32 retiring outer frame allocations, 56 bytes each | 1,792 |
| 32 off-lock retirement vectors, 16 Arc slots each | 4,096 |
| Total | 6,992 |

The refundable quota is 134,210,736 bytes; the sum remains 134,217,728.
Production computes these amounts from actual Rust type sizes and the same
client-limit constant used by endpoint admission. Each endpoint worker can be
suspended in one final frame-owner destructor at a time. Each retirement vector
can contain at most the existing 16 indexed entries. Private frame fields never
export Weak references or bare charged buffers. Last-cache destruction cannot
race a new admission through that destroyed cache. These bounds concern one
endpoint's cache-owned storage, not process RSS or ordinary uncached encoding.

The static Rust 1.99 audit follows `Arc::drop_slow` through the data destructor
and later weak deallocation, plus BTree entry removal and internal-root popping.
The indexed String/Arc node has eleven key/value slots and up to twelve edges;
the audited upper size is below the existing 1 KiB node allowance. The remaining
per-family admission still charges all three actual Vec capacities, owners and
key storage. The fixed margin covers the allocations that outlive those refunds.

An actual-destructor control pauses the refund on an owned test thread while
the outer Arc remains live, with an empty index. It checks that spending the
entire ceiling is refused, then releases and joins before asserting. Removing
only the margin fails both new margin controls; the four original controls
still pass. A second control keeps an earlier charged output alive to refuse
new cache admission, then verifies that uncached output retains complete selected
bytes while Weak references to both unused variants can no longer upgrade.
Restoring the old unconditional family owner fails that control.

The final complete daemon scope passes 89 tests in 35.770 seconds including
compilation; strict all-target Clippy passes in 7.764 seconds. The
[machine-readable record](m7-result-frame-admission-margin.json) retains
the actual test/strict scopes, counterfactual failures, earlier strict assertion
failure, source identities, tested executables and raw hashes. These local
controls do not establish the original live reader latency threshold, full
workspace acceptance or source-engine qualification.

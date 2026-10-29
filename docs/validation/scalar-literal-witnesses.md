# Scalar literal-contract witness qualification

Status: native Linux code gates pass on `4a2ebca23c0435456505e30aa3841d52cdb434f5`. Release,
native oracle and actual installed source-owner witness acceptance pass on the
notice-corrected frozen source `cef3125c7ec3ebc39f9dcf25eb4bb11781f72011`. Exact source comparison
binds the code-gate results to the corrected packaging. Protected delivery
remains a separate check on the final pull-request head.

## Source and installation

The code-qualified source composes the corrected witness implementation with
protected main `42bb75aed485c8a9386809bee89777bb995ee11b`. Its source index
SHA-256 is `bce071573d85451212ba82d9d799d52330ed2700b7c1debe8b84e512b2d52a41`.
The packaging correction adds two generated notice lines for `num-bigint 0.4.8`,
which the new normal `sail-cli -> sail-verus -> num-bigint` dependency path
redistributes. Every Rust, manifest, lock, checker, CI and toolchain byte is
unchanged. The source-equivalence receipt SHA-256 is `fb98ce2817ce5083fbe729959239f78d60425f39f5aec2ac9ee095361ec303c4`.
The corrected source index SHA-256 is
`868f53ec742d07bcf0dbe7825ce54110a64fd09dbdc19308c905fc3bc224678d`. Later earned-record
amendments are Markdown only. Source admission and typed
specification semantics reuse the neutral lowerer. Default Verus lowering,
native mutation classification and solver configuration retain their meanings.
The separate policy is `neutral-scalar-literal-contract-witness-v1`.

Qualification ran in the repository's pinned native Linux CI images on one
dedicated EC2 c7i.4xlarge in us-east-1. The staged installation passed complete
shipped-file verification before and after the installed witness matrix, then
again after its original archive was exported and extracted locally. The
installed service used its bundled runtime with no external Verus or engine
build override.

| Identity | SHA-256 |
|---|---|
| Original six-code-gate and failed-packaging archive (`4a2ebca2`) | `96f9bfafbb12d1fb01aa0207a445765e241cf33f8a8c73c3db99176d8249fb50` |
| Complete original archive-member index (`4a2ebca2`) | `a79bab7617e03112431a6864f0892e3cb7efaa8fa845ba7a02234efbe6181e9c` |
| New release, oracle and installed-service archive (`cef3125c`) | `a7c49ddefbaa418f748b48a77642808833c3f71d7300750ad1ba4a4e2978684b` |
| Complete exported original-file index | `9e346dbbced63c68c22fbff1f3eb6ea05ec922e16fbd6f836086d116927dd632` |
| Complete shipped release index | `cddffbac68d06ec3fce43800064dea608ee8e54d3f2fc77663b8f2f4a8ad7086` |
| Installed Sail executable | `af074450a39c13147bf9676706becc30a40b08cba614658080491515c292fabf` |
| Installed runtime worker | `7f7eae1d8bc7e1da6ecf7f34e5085a53e0bef1b93f5038d4046470c4a1480fb3` |

## Executed native gates

Every listed gate completed successfully on its explicitly identified source.
The six code gates retain their original execution and original source; they
were not rerun or relabeled as executions on the notice-only correction.
End to end includes all phases and the extra local cases; no phase was skipped.

| Gate | Result | Executed source | Original log SHA-256 |
|---|---|---|---|
| gate | Pass | `4a2ebca2` | `4a673a1a256611dc6174554b979902a83321fb18a9b28a7eada938ea1ba672dd` |
| solver | Pass | `4a2ebca2` | `484ed314285e5c3bc0316f2b0c6e4a299c31089325c47697600e1d7454a68a40` |
| e2e | Pass | `4a2ebca2` | `1c535f59b23b6b4ed54a899e827c0cd111c33a505f1ae182b00df215369ed95f` |
| conformance-fast-pr | Pass | `4a2ebca2` | `7dafbf28eb8069308842a66d3204af780e42079cca4f1d77595c3a3644b43c0d` |
| conformance-fast | Pass | `4a2ebca2` | `a84133f64603078a6887a7bbe0979e43e9eb080758358db66869a83237d2d350` |
| conformance-standard | Pass | `4a2ebca2` | `e016bc634e17bbbee1bc61fa51aa85e66963838a10ba083949fae72036ce1b3d` |
| release | Pass | `cef3125c` | `42de1d0608ec1ba94bd18fd605e54fbf02158e65bab73fdd1d453541e3f0cd9e` |
| native-oracle | Pass | `cef3125c` | `47af593e4e59c4ff580af87430c3c9898fe27e4ffdec49df841b3959c30df6af` |
| Installed witness service | Pass | `cef3125c` | `404ea1b7747f810173c141621377355e4c96111fe8488f868f675f7c8f2db22f` |

Fast conformance compared all four arms with zero unstable or different results,
infrastructure errors or skipped projects. Standard conformance retained these
exact summary lines, including resource-limit drift:

```text
conformance (standard tier): 159 units, 159 same, 0 unstable, 0 different, 0 infra errors; 13166 functions compared; 17 units with rlimit drift; 0 projects skipped
conformance (standard tier): 159 units, 159 same, 0 unstable, 0 different, 0 infra errors; 13166 functions compared; 17 units with rlimit drift; 0 projects skipped
conformance (standard tier): 159 units, 159 same, 0 unstable, 0 different, 0 infra errors; 13166 functions compared; 17 units with rlimit drift; 0 projects skipped
engine conformance (standard tier): 159 units, 159 same, 0 unstable, 0 different, 0 infra errors; 13166 functions compared; 17 units with rlimit drift; 0 projects skipped; engine 35fdb4942460: 158 focused units
```

The full standard gate also passed its human-eval soundness probe: eighteen
mutants plus cold, revert and forced-revert controls, twenty-one comparisons
all same, zero false-verified steps or functions, zero instability, differences,
infrastructure failures or unexpected stock outcomes. Both reverts recovered
their own original answers. This separate default-Verus probe does not change
the neutral literal-mutation UNKNOWN classification.

## Actual installed source-owner and solver acceptance

The source-owned acceptance example ran against the new installed service.
Preparation and concrete validation used its actual source-owner RPCs; raw SMT
requests used the same service's pinned runtime. All twelve cases retained their
original source, plan, requests, responses and independently evaluated concrete
assignments: sixteen SAT queries and sixteen validated assignments, plus eight
UNSAT controls. The matrix covers Boolean identity, guarded unsigned arithmetic,
signed and wide integers, multiplication, conditionals, eager old captures and
the original order-service price function. Nominal, reference and multiple-ID
targets in that service remain explicitly unsupported, with no omitted literal
candidates. Tautologies, infeasible preconditions and undefined postcondition or
eager-old arithmetic provide the UNSAT controls. Explicitly altered UNKNOWN
responses and changed-source negative checks remain nonpassing; those altered
responses are not original solver UNKNOWN observations. Summary SHA-256:
`7d6c12e36e3f7bc47eedccac1d54a933a999493c47664a000a33aa1b100a247d`.

This establishes concrete violations of the exact unchanged admitted scalar
contract. Requirement IDs provide attribution; a stronger contract can fail
while its weaker linked requirement holds. No semantic requirement rejection,
native Verus kill, proof tier, complete implementation correctness or full
realizability follows from this diagnostic. Native Verus UNKNOWN remains
Inconclusive.

## Independent review and retained attempts

Fresh corrected-source review passes, including independent concrete checks,
actual runtime queries and negative source-owner RPC tests. Its report SHA-256
is `5f6962c9e808897131cc06a1d99b7d7b022d86e917fc9949f8addd4b7d63054c`;
raw index SHA-256 is
`0e947757a1a0825ede8ac28c71148eac3613a5d6abbe484cfa7baa300a68da56`.
Fresh merge-boundary review also passes. Its report SHA-256 is
`8cd4428ea76a517fe36f020f54abc53b4862f00f459434168cb12b3ac437850d`;
raw index SHA-256 is
`cfb52ec479dfc5dc16b37a2830a34686cc26b353385483a819e68a0f6b59581d`.
Neither review is substituted for the native or installed gates above.

Failed and superseded originals remain outside the repository:

| Candidate or attempt | Result and disposition |
|---|---|
| Initial local probes | An incorrect engine directory returned Unavailable; a source-admission expectation and intermediate lints were corrected. Original logs remain retained. |
| `76aa6e19` independent review | Rejected unbound check-command attribution and malformed integer decoding. The correction binds the exact final check and structurally decodes scalar terms; the general presentation decoder is unchanged. |
| `76aa6e19` native gate | Completed failure: 1,061 tests passed and the error registry failed for unregistered SAIL3145. Log SHA-256 `d99b0d842cb8f33086655912191d7b8a5fdd168841526e17098355198478c7a9`. Corrected source registers the code. |
| `b4448685` native run | Gate and solver passed; the owned end-to-end container was deliberately stopped to integrate newer protected main. Exit 143 is interrupted, not passing. Original archive SHA-256 `f2a55ca126588291ed1207c5e6e5ed15ccaff995512c999fb2012975102543d1`. |
| `4a2ebca2` release | Completed failure after six successful code gates: generated notices omitted the newly normal `num-bigint 0.4.8` dependency. Original release log SHA-256 `b4a27cf66f5ae35ff1b777bf6a1571c1f19a2eeee2267837e505e068780b3e5d`. The exact two-line generated notice correction is qualified separately above. |
| Local `cef3125c` extraction | The safe archive data filter suppressed directory modes, producing 0700 instead of the recorded 0755 on thirteen directories. File bytes and file modes matched. Original verification failure SHA-256 `83fb378146fd4ea9534df6f99b8683debfde6e095d06f8832311affb3b94b698`. Restoring those modes against the original archive headers and unchanged index allowed the unchanged full verifier to pass; corrected log SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`. |
| Initial native image setup | A remote-ref-only source bundle produced an empty checkout. The corrected complete-history bundle was explicitly fetched; no product code changed. |
| Initial installed-run controller | Started before the status file existed and failed before any service acceptance. The queued controller was corrected and the successful installed run is recorded above. |

Earlier local generated-query checks used the previously qualified installed
runtime, not this new service's RPCs. Their corrected raw index SHA-256 remains
`937d4e410e4989b6f9550987be4086df836757d6a38b5dcdc21f2e52d951060d`.
All original failures remain distinct from the successful new installed matrix.

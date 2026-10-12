# Original c2 reader workflow readiness

This file-only review prepares the next original reader workflow after the
serialized real-project benchmark. It launches nothing. The historical
[730 reader pass](m7-reader-730a321-terminal.md) remains a separate product scope;
neither that pass nor c2 installation supplies a new Current proof.

Use the unchanged [current-product launcher](../../scripts/m7-current-reader-controller.py)
with the rebound acceptance execution record:

```sh
python3 scripts/m7-current-reader-controller.py \
  --rev c2a4b23 --product-grant "$READER_EXECUTION_GRANT" \
  --container sail-m7-acceptance --execute
```

`READER_EXECUTION_GRANT` names the retained host `product-grant-acceptance.json`.
Its exact SHA-256 is
`f66180acb44bad138566ac494cd436ef79a82b4f7b64463e601ca22f9f6d6d19`
(7,314 bytes), identical to the successful rebind's execution record. It binds
source `c2a4b2363b415aa2157fc38dc388192125b87109`, source archive
`590c4fd2de781a9312393d73d0bd0256bfe92b403be6640eaa6c321f11746a11`,
and both successful release/install phases. The product content is unchanged:

| Binary | Bytes | SHA-256 |
| --- | ---: | --- |
| sail | 76,677,936 | `d198d931057be4a3f1e122eb053c30738a799379c161881ff1de75b7fc3aea50` |
| sail-z3 | 11,000,936 | `0abda23619c4147a8f0a7f595f7ae5954c5d98d7ba37e710473bad9f4c3e65ca` |

The default c2 stage must be absent. If it was previously used, choose a new
`--stage` satisfying the launcher's private Sail stage naming rule. Preparation
alone cannot subsequently be reused as an execution stage: every invocation
creates its own fresh stage. Execution uses `grant.container`; `--container`
selects where to read the grant only when its host file is absent.

The launcher checks actual container birth, private executable ownership,
selected runtime bytes and inherited selector refusal, then runs the original
wrapper's freeze-only check before any workflow. Any failure stops that scope.
It preserves the sealed product and, where installation freeze metadata is
absent, makes a separate byte-verified owned copy. No receipts, proof targets or
old Current authority are transferred.

The following bindings match the current repository and retained 730 inventory:

| Input | SHA-256 |
| --- | --- |
| Host launcher | `625787a93919433fc384297f26f6079cc4ab5e9ccf9553c6340d5ce15309e75f` |
| Original reader controller | `84966d77cd679653b96ab9f05019d51f805538f3d4dd509c1591da1679b7634d` |
| Corrected retained controller | `207b45ef034b6c1e947dbb0efdb87368b62d5ef8a4c42ab5768413670f775a11` |
| Python collector | `3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720` |
| Prebuilt Rust collector | `f1632772daacc28e4bb5e7f7747e0e048fce7973aeff31b6ed65c5bbbd0e0b4d` |
| Selected c77 manifest | `54109b76e8bf79d92332194431e5b4dd83038b9f285994c319b5a2e3391c3e9d` |
| Complete 274-file template inventory | `c81ef6ca164608180e35ca4d6e4d92814dec2fefb8f9b177242e7f2e14416156` |

Retained `preparation.json` records all 274 template files and the actual probe
bundle; the archived probes rehash to these digests. Source files were equal
before and after the 730 run. The source reference is input identity evidence,
not transferred proof authority. The source template and probe locations are
fixed by the existing launcher. Their present native existence is not certified
by this host review; normal launch preflight must recheck them. The Rust probe
remains the original collector built with the staged 02fd1fc SDK, not a new c2
SDK build. The wrapper's deliberate scope-label replacement accounts for its
different staged hash; no workflow expression is changed.

The workflow retains independent source-only baseline/active copies, original
forced ordinary preparation and 600-second setup limits, genuine complete
Current proofs, and continuous actual verifier admission. Python runs first,
then Rust: each collector has four concurrent clients and 200 total alternating
samples (100 Result and 100 Status), strict p95 below 200 ms and five-second RPCs.
Python receives and parses the full Result graph. First failure stops; retained
CLI/MCP checks run only after both collectors pass, under the original
120-second whole / 30-second CLI / five-second RPC bounds.

Retained functional checks validate complete CLI JSON and MCP structured
results, explanations, immutable named baselines, exact self-comparison,
journal reconnect/cursor continuity, the corrected exact wrong-stream SAIL0902
error projection and an independently owned cancellation. Full MCP exchanges
retain both text and structured payloads, but successful text rendering is not
independently compared with the structured graph. These checks do not certify
graphical rendering or reader performance outside the two original collectors.
Keep raw reports, all preparation costs, normal owned closure and post-input
seals, including any failure or incomplete observation. No c2 reader result,
paired speedup, benchmark pass or overall M7 acceptance is claimed here.

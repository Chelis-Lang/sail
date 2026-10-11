# Current-product original reader launcher

The reusable host launcher is [m7-current-reader-controller.py](../../scripts/m7-current-reader-controller.py).
It accepts the final product short revision and installation-only grant, requiring
both successful release-build and owned-install phases. Default operation stages
and seals inputs only. It starts no proof, daemon, census or retained workflow.

```sh
python3 scripts/m7-current-reader-controller.py --self-test
python3 scripts/m7-current-reader-controller.py --rev SHORTREV --product-grant product-grant.json
```

An execution invocation adds `--execute` and requires a fresh stage; an existing
stage is refused. The host checks selected Docker container ID, image and start
birth before staging, after static sealing and before execution. A changed birth
refuses old process grants. Interruption signals only the native supervisor whose
retained PID, kernel birth and UID still match. Unconfirmed interrupted cleanup
is an error requiring actual actor inspection, never a successful closure.

The launcher transfers a bounded regular-file bundle into a fresh native-owned
private directory. It binds the original 274-file source-only HE input, unchanged
Python and Rust collectors, original reader `84966d77…`, corrected retained
controller `207b45ef…`, and selected qualified source manifest `54109b76…`.
The reviewed wrapper's support manifest and human-facing scope label are rebound;
its workflow logic, 600-second setup, four-client 200-sample censuses, complete
result graphs and strict p95 below 200 ms remain unchanged. Retained checks run
only after both censuses pass, using their original 120/30/5 settings.

A product installation lacking controller `freeze.json` gets a separate native
byte-verified two-binary copy through the existing audited installation helper.
The original installation remains unchanged; the derived grant binds both
locations. This fallback costs one additional complete product copy. Supplying
freeze metadata in the root installation avoids that cost. No hardlinks or
copied proof authority are used. Private environment values stay in the native
stage; public seals retain only their digest.

[Preparation evidence](m7-current-reader-controller.json) records 13 pure controls
and an actual preparation-only seal using the earlier cdb6 product. That scope
verified all 13 selected source artifacts, exact source and probes and native
ownership. Initial syntax and missing-freeze setup failures remain separate.
The successful native preparation preceded final host-only admission checks and
interruption forwarding, whose native execution remains untested. The public
original-wrapper identity output is separately retained. These checks supply no
proof, reader timing, complete-standard or overall M7 qualification credit.

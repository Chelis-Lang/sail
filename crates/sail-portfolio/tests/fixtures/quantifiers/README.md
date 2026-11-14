# Pinned Z3 counter fixtures

These small complete traces are regression inputs, captured natively on
Linux x86-64 c7i.4xlarge/us-east-2 with pinned Z3 4.16.0, binary SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
Commands use `z3 -smt2 -in -st trace=true trace_file_name=CASE.trace` with the
adjacent original input. Every actual exit is0 and stderr is empty.

The two-check input comes from the historical health spike a7b9f261. Fresh
traces and quoted-qid/MBQI controls were collected on2026-11-14; the historical
trace is not used as new acceptance evidence. Z3's own captured statistics
report8quantifier instances in each two-check/quoted case and1in the MBQI case.
The two-check trace has16additional theory instances and2begin-check events.
Original stdout/statistics, source/solver bindings and acceptance logs stay
outside the repository; only these minimal decoder fixtures are tracked.

Primary format source is Z3 tagz3-4.16.0, commit
`ddb49568d3520e99799e364fb22f35fc67d887b1`: ast/ast.cpp (quoted qid declarations),
smt/smt_quantifier.cpp (E-matching/MBQI discoveries), smt/qi_queue.cpp (actual
instances) and smt/fingerprints.cpp (hashes are not unique object identities).
Counts describe the traced execution, with explicit unattributed events;
they establish no verifier outcome or proof/currentness authority.

Trace fixtures deliberately retain Z3's emitted trailing spaces. Git's whitespace
warning is expected for those raw payload lines; source/docs formatting checks
exclude only these byte-preserved fixture inputs. No fixture byte is trimmed or
normalized to satisfy a text-style check.

The datatype input/trace is an independently captured pinned-Z3 reproduction
from rejected08ea65a4. Exact trace SHA256
`0320f69970298372a93eebdaab3d7a17b01e11922238cb142ed4a35173f8631c`;
input SHA256 `18f92939fdeccdf2c90569939baec2cef22e4f2c3193a75248d4984113afea8c`.
One native datatype accessor axiom emits one explicit theory instance. Original
stdout/stderr/exit and the failed counter result remain in the external review
archive `a6eddf400860641017bade56180b328cd9c22afaa662c3cf4022891690a39176`.

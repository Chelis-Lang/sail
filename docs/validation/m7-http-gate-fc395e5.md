# Complete gate after the retained MCP deadline correction

The frozen `fc395e5` complete workspace gate fails: strict Clippy and the nine
retained CLI/MCP collector controls pass, followed by 523 of 524 workspace
tests. The remaining failure is the original setup-publication HTTP lookup at
its unchanged 250 ms budget and 75 ms server delay. Later gate steps are not
reached. [The record](../../bench/validation/m7-linux-gate-fc395-failure-emulated-2026-10-11.json)
binds the source archive, command, terminal log and counts.

The original concurrent eight-reader, credential/disconnect and paused-initiator
authentic/forged controls pass in this scope. They cannot replace the failed
test. The failing assertion does not contain a request phase trace, so the
complete cause remains unestablished. Container emulation and concurrent work
are labels, not causal conclusions.

The subsequent loopback transport-refresh change removes redundant runtime
construction after complete fresh configuration validation. It does not change
the original assertion or budgets; failure reporting adds existing phase and
request metadata. Its focused and complete validation remain separate from this
unchanged frozen failure. HTTPS retains reconstruction on refresh.

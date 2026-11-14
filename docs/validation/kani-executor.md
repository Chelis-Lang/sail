# Kani executor integration

Status: tool-payload authenticator implemented as an uncompiled development
candidate. Execution, CLI/backend selection and complete pilot qualification
remain planned. Tracks #712 G / ADR0152.

The Rust authenticator binds every original bundle/Rust file and executable bit;
it executes nothing. The canonical Rust commitment is derived from the same
complete original and independently installed maps accepted in ADR0149.

Pending: native compile/Clippy, original Kani and full Rust prefix positive,
modified/missing/extra/linked/special/unreadable controls, native platform checks,
source/lock guard review and full local gate. No check or baseline is waived.
The later executor still needs original process/bounds/output authentication,
source/tool/environment closure, bounded outcome attribution, process cleanup,
direct-arrayvec comparisons, conservative cache policy, CLI and Mac acceptance.

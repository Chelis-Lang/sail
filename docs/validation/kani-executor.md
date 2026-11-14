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

Independent lock-binding review approves the exact new lock only (ADR0152):
all78 controls,316 complete generated-driver/input-map pairs and39 focused
refusals pass; all934 raw review payloads are lead-rehashed and retained.
The contributor inspect_tools example performs original payload inspection and
reinspection only. Runtime authentication remains uncompiled/unqualified.

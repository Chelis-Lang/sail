//! Auditable requested-storage upper bound for retained Result ownership.
//!
//! Counts inline enum/struct storage once at its owning allocation; descendants
//! contribute actual String/Vec/PathBuf capacities, boxes, and conservative map
//! node storage. Exhaustive destructuring makes schema growth a compile error.

use std::mem::size_of;
use std::path::PathBuf;

use serde_json::Value;

// Workspace serde_json1.0.151 default/std uses non-allocating Number and
// BTreeMap-backed Map. preserve_order/arbitrary_precision change these layouts;
// unknown layout must remain ordinary correct decoding without retention.
pub(super) fn audited_layout() -> bool {
    size_of::<usize>() == 8
        && size_of::<Value>() == 32
        && size_of::<serde_json::Number>() == 16
        && size_of::<serde_json::Map<String, Value>>()
            == size_of::<std::collections::BTreeMap<String, Value>>()
}

pub(super) trait Owned {
    fn heap(&self) -> Option<usize>;
}

macro_rules! scalar {
    ($($ty:ty),* $(,)?) => { $(impl Owned for $ty {
        fn heap(&self) -> Option<usize> { Some(0) }
    })* };
}
scalar!(bool, u32, u64, i32, f64);

impl Owned for String {
    fn heap(&self) -> Option<usize> {
        Some(self.capacity())
    }
}
impl Owned for PathBuf {
    fn heap(&self) -> Option<usize> {
        Some(self.capacity())
    }
}
impl<T: Owned> Owned for Option<T> {
    fn heap(&self) -> Option<usize> {
        self.as_ref().map_or(Some(0), Owned::heap)
    }
}
impl<T: Owned> Owned for Box<T> {
    fn heap(&self) -> Option<usize> {
        size_of::<T>().checked_add((**self).heap()?)
    }
}
impl<T: Owned> Owned for Vec<T> {
    fn heap(&self) -> Option<usize> {
        self.iter().try_fold(
            self.capacity().checked_mul(size_of::<T>())?,
            |used, item| used.checked_add(item.heap()?),
        )
    }
}
impl Owned for Value {
    fn heap(&self) -> Option<usize> {
        match self {
            Self::Null | Self::Bool(_) => Some(0),
            Self::Number(_) if size_of::<serde_json::Number>() == 16 => Some(0),
            Self::Number(_) => None, // unaudited arbitrary-precision storage
            Self::String(value) => value.heap(),
            Self::Array(values) => values.heap(),
            Self::Object(values) => {
                // Rust1.99 BTreeMap: B=6, eleven key/value slots and twelve
                // edges; String/Value nodes fit 1024B on audited64-bit targets.
                // Every nonroot node owns >=5 entries, hence <=1+len/5 nodes.
                // Include one root even for an empty map. Inline keys/Values
                // are already covered by this deliberately conservative charge.
                if !audited_layout() {
                    return None;
                }
                values.iter().try_fold(
                    (values.len() / 5 + 1).checked_mul(1024)?,
                    |used, (key, value)| used.checked_add(key.heap()?)?.checked_add(value.heap()?),
                )
            }
        }
    }
}

macro_rules! owned_struct {
    ($ty:path { $($field:ident),* $(,)? }) => {
        impl Owned for $ty {
            fn heap(&self) -> Option<usize> {
                let Self { $($field),* } = self;
                let used: usize = 0;
                $(let used = used.checked_add($field.heap()?)?;)*
                Some(used)
            }
        }
    };
}
macro_rules! owned_enum {
    ($ty:path { $($variant:ident),* $(,)? }) => {
        impl Owned for $ty {
            fn heap(&self) -> Option<usize> {
                match self { $(Self::$variant => Some(0)),* }
            }
        }
    };
}
owned_struct!(super::Status {
    version,
    id,
    project_dir,
    args,
    state,
    submitted_at_ms,
    started_at_ms,
    finished_at_ms,
    exit_code,
    input_revision,
    currentness,
    error
});
owned_struct!(super::ResultDocument {
    repair,
    job,
    result
});
owned_struct!(crate::output::Report {
    portfolio,
    schema_version,
    sail_version,
    mode,
    command,
    exit_code,
    solver,
    cache,
    attribution,
    crates,
    diagnostics,
    summary,
    error,
    engine
});
owned_struct!(crate::output::EngineProvenance {
    name,
    manifest_sha256,
    established_by
});
owned_struct!(crate::project::EngineIdentity {
    name,
    manifest_sha256
});
owned_enum!(crate::output::EngineEstablishment {
    Engine,
    StockFallback
});
owned_struct!(crate::output::SolverConfiguration { seed });
owned_struct!(crate::output::PortfolioAttempt {
    configuration,
    verified,
    exit_code,
    wall_ms,
    error,
    report,
    stderr
});
owned_struct!(crate::output::PortfolioReport {
    seeds,
    winner_seed,
    attempts
});
owned_struct!(crate::output::SolverInfo {
    name,
    version,
    path,
    sha256
});
owned_struct!(crate::output::CacheInfo { daemon });
owned_struct!(crate::output::AttributionInfo {
    status,
    notes,
    unmatched_sessions
});
owned_struct!(crate::output::CrateReport {
    name,
    verus,
    functions
});
owned_struct!(crate::output::VerusResults {
    verified,
    errors,
    success,
    encountered_vir_error
});
owned_struct!(crate::output::FunctionReport {
    function,
    module,
    fn_mode,
    verdict,
    tier,
    unstable,
    verus_success,
    provenance,
    timing,
    fragility,
    obligations
});
owned_struct!(crate::output::ProvenanceReport {
    configuration,
    mode,
    source,
    checks_solved,
    checks_served,
    checks_unrecorded,
    engine
});
owned_struct!(crate::output::Timing { smt_micros, rlimit });
owned_struct!(crate::output::Fragility {
    score,
    runs,
    proved
});
owned_struct!(crate::output::ObligationReport {
    id,
    kind,
    ordinal,
    spinoff,
    advisory,
    span,
    outcome,
    unstable,
    checks,
    counterexample
});
owned_struct!(crate::output::SpinoffInfo { index, reason });
owned_struct!(crate::output::SourceSpan {
    text,
    file,
    line,
    column,
    end_line,
    end_column
});
owned_struct!(crate::output::CheckReport {
    answer,
    rlimit_limit,
    rlimit_used,
    reason_unknown,
    source
});
owned_struct!(crate::output::Counterexample {
    check,
    model,
    assignments
});
owned_struct!(crate::output::Assignment { name, sort, value });
owned_struct!(crate::output::DiagnosticSpan {
    file,
    line,
    column,
    primary
});
owned_struct!(crate::output::Diagnostic {
    level,
    code,
    message,
    spans,
    rendered
});
owned_struct!(crate::output::Summary {
    functions,
    verified,
    failed,
    unknown,
    provisional,
    unstable
});
owned_struct!(crate::output::ErrorInfo { code, message });
owned_struct!(crate::project::ProjectResult {
    kind,
    schema_version,
    run_id,
    workspace,
    context_sha256,
    input_revision,
    currentness,
    exit_code,
    targets,
    execution,
    notes,
    error,
    engine
});
owned_struct!(crate::project::TargetResult {
    package_id,
    target,
    kind,
    establishing_run,
    reused,
    requested,
    complete,
    report
});
owned_struct!(crate::project::Execution {
    verifier_invocations,
    reused_targets,
    wall_ms
});
owned_struct!(crate::repair::BatchResult {
    focus,
    full,
    manifest_sha256,
    compiler_invocations,
    batched,
    selection_error,
    engine_fault
});
owned_struct!(crate::repair::ResultDocument {
    version,
    batch,
    accepted
});
owned_struct!(crate::proof::Target {
    package_id,
    name,
    kind
});
owned_enum!(crate::output::AttributionStatus {
    Complete,
    Partial,
    Unavailable
});
owned_enum!(crate::output::VerdictName {
    Verified,
    Failed,
    Unknown,
    Provisional
});
owned_enum!(crate::output::TierName {
    Provisional,
    Confirmed
});
owned_enum!(crate::output::SourceSummaryName {
    Fresh,
    Cache,
    Mixed,
    Unrecorded
});
owned_enum!(crate::output::OutcomeName {
    Proved,
    Failed,
    Unknown
});
owned_enum!(crate::output::AnswerName {
    Sat,
    Unsat,
    Unknown
});
owned_enum!(crate::output::SourceName { Solved, Cache });
owned_enum!(crate::project::Currentness {
    Current,
    Historical,
    Unknown
});
owned_enum!(crate::proof::Phase { Focus, Full });
owned_enum!(crate::jobs::Lifecycle {
    Queued,
    Running,
    Completed,
    Cancelled,
    Superseded,
    InfrastructureError
});
owned_enum!(crate::jobs::Currentness {
    Current,
    Historical,
    Unknown
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spare_string_and_vec_capacity_and_nested_boxes_are_counted() {
        let mut text = String::with_capacity(4096);
        text.push('x');
        assert_eq!(text.heap(), Some(4096));
        let mut values = Vec::with_capacity(1024);
        values.push(text);
        assert_eq!(values.heap(), Some(1024 * size_of::<String>() + 4096));
        let boxed = Box::new(Some(values));
        assert_eq!(
            boxed.heap(),
            Some(size_of::<Option<Vec<String>>>() + 1024 * size_of::<String>() + 4096)
        );
    }
    #[test]
    fn map_nodes_keys_enum_slots_and_descendant_capacities_are_charged() {
        let mut key = String::with_capacity(2048);
        key.push('k');
        let mut value = String::with_capacity(4096);
        value.push('v');
        let mut map = serde_json::Map::new();
        map.insert(key, Value::String(value));
        let graph = Value::Object(map);
        assert_eq!(graph.heap(), Some(1024 + 2048 + 4096));
        let mut array = Vec::with_capacity(1024);
        array.push(graph);
        assert_eq!(
            Value::Array(array).heap(),
            Some(1024 * size_of::<Value>() + 1024 + 2048 + 4096)
        );
    }
}

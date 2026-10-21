//! The permitted internal dependency graph between `sail-*` crates.
//!
//! This encodes the charter's dependency structure (ADR 0001):
//! `sail-smtlib` feeds `sail-session`, which feeds `sail-key`; the cache
//! depends on keys; the solver pool depends on the session model; the daemon
//! composes cache, solver pool, ledger and portfolio. The shim may depend
//! only on the protocol, the session model, key derivation (pure
//! computation, ADR 0005) and the thin daemon client in `sail-api`, so its
//! fallback path stays small and simple. `sail-bench` may use key
//! derivation too (pure computation), to recompute and check trace keys in
//! its history experiment. `sail-bench` and `sail-cli` use
//! `sail-portfolio` for fragility measurement (ADR 0018); no crate on the
//! cache-key path may reach it, so measurement mutations can never feed a
//! key. Changing this
//! table is an architectural decision: record an ADR.

/// `(crate, internal crates it may depend on directly)`.
pub const ALLOWED: &[(&str, &[&str])] = &[
    ("sail-smtlib", &[]),
    ("sail-session", &["sail-smtlib"]),
    ("sail-key", &["sail-smtlib", "sail-session"]),
    ("sail-cache", &["sail-key"]),
    ("sail-solver", &["sail-smtlib", "sail-session"]),
    ("sail-ledger", &["sail-key"]),
    (
        "sail-portfolio",
        &["sail-smtlib", "sail-session", "sail-solver", "sail-ledger"],
    ),
    ("sail-api", &["sail-key", "sail-ledger"]),
    (
        "sail-runtime",
        &["sail-api", "sail-cache", "sail-key", "sail-smtlib"],
    ),
    (
        "sail-shim",
        &["sail-smtlib", "sail-session", "sail-key", "sail-api"],
    ),
    (
        "sail-verus",
        &["sail-smtlib", "sail-session", "sail-ledger", "sail-api"],
    ),
    (
        "sail-daemon",
        &[
            "sail-smtlib",
            "sail-session",
            "sail-key",
            "sail-cache",
            "sail-solver",
            "sail-ledger",
            "sail-portfolio",
            "sail-verus",
            "sail-api",
            "sail-runtime",
        ],
    ),
    ("sail-mcp", &["sail-api"]),
    (
        "sail-conformance",
        &[
            "sail-smtlib",
            "sail-session",
            "sail-ledger",
            "sail-verus",
            "sail-api",
        ],
    ),
    (
        "sail-bench",
        &[
            "sail-smtlib",
            "sail-session",
            "sail-key",
            "sail-solver",
            "sail-ledger",
            "sail-portfolio",
            "sail-verus",
            "sail-api",
            "sail-conformance",
        ],
    ),
    (
        "sail-cli",
        &[
            "sail-api",
            "sail-cache",
            "sail-runtime",
            "sail-ledger",
            "sail-verus",
            "sail-daemon",
            "sail-mcp",
            "sail-portfolio",
            "sail-conformance",
            "sail-bench",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::ALLOWED;
    use std::collections::{BTreeMap, BTreeSet};

    fn graph() -> BTreeMap<&'static str, &'static [&'static str]> {
        ALLOWED.iter().copied().collect()
    }

    /// Every crate reachable from `start`, excluding `start` itself.
    fn reachable(start: &str) -> BTreeSet<&'static str> {
        let g = graph();
        let mut seen = BTreeSet::new();
        let mut stack: Vec<&str> = g.get(start).copied().unwrap_or_default().to_vec();
        while let Some(n) = stack.pop() {
            if seen.insert(n) {
                stack.extend(g.get(n).copied().unwrap_or_default());
            }
        }
        seen
    }

    #[test]
    fn every_edge_targets_a_listed_crate() {
        let g = graph();
        for (name, deps) in ALLOWED {
            for dep in *deps {
                assert!(
                    g.contains_key(dep),
                    "{name} -> {dep} names an unlisted crate"
                );
            }
        }
    }

    #[test]
    fn permitted_graph_is_acyclic() {
        for (name, _) in ALLOWED {
            assert!(!reachable(name).contains(name), "{name} reaches itself");
        }
    }

    #[test]
    fn shim_stays_small() {
        // Charter: the shim depends only on the protocol, the session model
        // and a thin client, so its fallback path stays simple.
        let reach = reachable("sail-shim");
        for heavy in [
            "sail-daemon",
            "sail-cache",
            "sail-solver",
            "sail-portfolio",
            "sail-verus",
        ] {
            assert!(!reach.contains(heavy), "sail-shim may not reach {heavy}");
        }
    }

    #[test]
    fn measurement_mutations_cannot_reach_cache_keys() {
        // ADR 0018: the portfolio's reseeded and shuffled sessions exist
        // only to measure. Nothing that derives, stores or serves cache
        // keys may depend on it.
        for keyed in [
            "sail-shim",
            "sail-key",
            "sail-cache",
            "sail-session",
            "sail-api",
        ] {
            assert!(
                !reachable(keyed).contains("sail-portfolio"),
                "{keyed} may not reach sail-portfolio"
            );
        }
    }

    #[test]
    fn foundation_crates_are_solver_and_verifier_agnostic() {
        // The protocol, session and key layers must not know about Verus.
        for base in ["sail-smtlib", "sail-session", "sail-key", "sail-cache"] {
            assert!(
                !reachable(base).contains("sail-verus"),
                "{base} may not depend on sail-verus"
            );
        }
    }
}

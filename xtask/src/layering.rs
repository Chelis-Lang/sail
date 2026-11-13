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
    ("sail-acceptance", &[]),
    ("sail-smtlib", &["sail-acceptance"]),
    ("sail-session", &["sail-smtlib"]),
    (
        "sail-key",
        &["sail-smtlib", "sail-session", "sail-acceptance"],
    ),
    (
        "sail-cache",
        &["sail-key", "sail-smtlib", "sail-acceptance"],
    ),
    ("sail-solver", &["sail-smtlib", "sail-session"]),
    ("sail-ledger", &["sail-key"]),
    (
        "sail-portfolio",
        &["sail-smtlib", "sail-session", "sail-solver", "sail-ledger"],
    ),
    (
        "sail-api",
        &["sail-key", "sail-ledger", "sail-smtlib", "sail-acceptance"],
    ),
    (
        "sail-runtime",
        &["sail-api", "sail-cache", "sail-key", "sail-smtlib"],
    ),
    (
        "sail-shim",
        &[
            "sail-smtlib",
            "sail-session",
            "sail-key",
            "sail-api",
            "sail-acceptance",
        ],
    ),
    (
        "sail-verus",
        &[
            "sail-smtlib",
            "sail-session",
            "sail-ledger",
            "sail-api",
            "sail-acceptance",
        ],
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
            "sail-acceptance",
        ],
    ),
    ("sail-mcp", &["sail-api"]),
    ("sail-kani", &["sail-api"]),
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
        "sail-solve",
        &[
            "sail-shim",
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

/// The temporal extension workspace (`extensions/temporal`, ADR 0083).
///
/// Its crates may depend on each other and on main-workspace crates; no
/// main-workspace crate may depend on them, which [`ALLOWED`] enforces by
/// not naming them.
pub const EXTENSION: &str = "extensions/temporal";

/// `(extension crate, crates it may depend on directly)`.
pub const EXTENSION_ALLOWED: &[(&str, &[&str])] = &[
    ("sail-dive", &["sail-api", "sail-runtime"]),
    ("sail-runtime-worker", &["sail-dive"]),
    ("sail-temporal", &["sail-solve", "sail-dive"]),
];

/// The private D.I.V.E. repository. Only the extension lock may name it.
pub const DIVE_REPOSITORY: &str = "github.com/Chelis-Lang/dive";

/// Entries of the main workspace's `Cargo.lock` that would put D.I.V.E. in a
/// public build (ADR 0101): any package fetched from [`DIVE_REPOSITORY`],
/// any `dive-*` package and any extension crate. An empty result means the
/// lock resolves without the private sources.
///
/// # Errors
///
/// When the lock is not TOML or has no package list.
pub fn private_lock_entries(lock: &str) -> Result<Vec<String>, String> {
    let lock: toml::Table = lock
        .parse()
        .map_err(|e| format!("cannot parse Cargo.lock: {e}"))?;
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock has no package list")?;
    let extension: Vec<&str> = EXTENSION_ALLOWED.iter().map(|(name, _)| *name).collect();
    let mut found = Vec::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let source = package
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if source.contains(DIVE_REPOSITORY)
            || name.starts_with("dive-")
            || extension.contains(&name)
        {
            found.push(format!("{name} {source}").trim_end().to_owned());
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED, EXTENSION_ALLOWED, private_lock_entries};
    use std::collections::{BTreeMap, BTreeSet};

    fn graph() -> BTreeMap<&'static str, &'static [&'static str]> {
        ALLOWED.iter().chain(EXTENSION_ALLOWED).copied().collect()
    }

    #[test]
    fn main_workspace_never_reaches_the_extension() {
        let main: BTreeSet<&str> = ALLOWED.iter().map(|(name, _)| *name).collect();
        for (name, deps) in ALLOWED {
            for dep in *deps {
                assert!(
                    main.contains(dep),
                    "{name} -> {dep} leaves the main workspace"
                );
            }
        }
        for (name, _) in EXTENSION_ALLOWED {
            assert!(!main.contains(name), "{name} is listed in both workspaces");
        }
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
        for (name, deps) in ALLOWED.iter().chain(EXTENSION_ALLOWED) {
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
        for (name, _) in ALLOWED.iter().chain(EXTENSION_ALLOWED) {
            assert!(!reachable(name).contains(name), "{name} reaches itself");
        }
    }

    #[test]
    fn public_lock_refuses_every_private_source() -> Result<(), String> {
        let clean = "version = 4\n\n[[package]]\nname = \"sail-api\"\nversion = \"0.2.0\"\n";
        assert_eq!(private_lock_entries(clean)?, Vec::<String>::new());
        for (entry, expected) in [
            (
                "name = \"dive-api\"\nversion = \"0.0.0\"\nsource = \"git+https://github.com/Chelis-Lang/dive?rev=1#1\"",
                "dive-api git+https://github.com/Chelis-Lang/dive?rev=1#1",
            ),
            (
                "name = \"other\"\nversion = \"0.0.0\"\nsource = \"git+https://github.com/Chelis-Lang/dive#2\"",
                "other git+https://github.com/Chelis-Lang/dive#2",
            ),
            ("name = \"dive-ir\"\nversion = \"0.1.0\"", "dive-ir"),
            ("name = \"sail-dive\"\nversion = \"0.1.0\"", "sail-dive"),
            (
                "name = \"sail-temporal\"\nversion = \"0.1.0\"",
                "sail-temporal",
            ),
        ] {
            let lock = format!("{clean}\n[[package]]\n{entry}\n");
            assert_eq!(private_lock_entries(&lock)?, vec![expected.to_owned()]);
        }
        assert!(private_lock_entries("version = 4").is_err());
        Ok(())
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

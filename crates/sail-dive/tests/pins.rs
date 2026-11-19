//! The D.I.V.E. release Sail reports is the one this workspace links.
//!
//! `sail_api::temporal::DIVE_VERSION` must equal the exact `=` requirement on
//! `dive-api` and `dive-ir` in the root manifest and the version both resolve
//! to in `Cargo.lock`, and the lock must take them from crates.io. The only
//! exception is a `[patch.crates-io]` entry for the crate in the root
//! manifest, used while developing against an unpublished D.I.V.E. release:
//! then the patched source is accepted, but its version must still match.
//! Publication cannot carry such a patch (`cargo package` drops `[patch]`
//! and verifies against the registry), so the exception never ships.

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

fn table(text: &str, what: &str) -> Result<toml::Table, String> {
    text.parse().map_err(|e| format!("{what} is not TOML: {e}"))
}

#[test]
fn reported_version_is_the_linked_release() -> Result<(), String> {
    let manifest = table(include_str!("../../../Cargo.toml"), "Cargo.toml")?;
    let lock = table(include_str!("../../../Cargo.lock"), "Cargo.lock")?;
    let version = sail_dive::DIVE_VERSION;
    let dependencies = manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .ok_or("no workspace dependencies")?;
    let patches = manifest
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(toml::Value::as_table);
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock has no packages")?;
    for name in ["dive-api", "dive-ir"] {
        let requirement = dependencies
            .get(name)
            .and_then(|d| d.get("version"))
            .and_then(toml::Value::as_str);
        assert_eq!(
            requirement,
            Some(format!("={version}").as_str()),
            "{name} is not required at exactly {version}"
        );
        let locked: Vec<_> = packages
            .iter()
            .filter(|p| p.get("name").and_then(toml::Value::as_str) == Some(name))
            .collect();
        assert_eq!(locked.len(), 1, "{name} must resolve exactly once");
        let entry = locked[0];
        assert_eq!(
            entry.get("version").and_then(toml::Value::as_str),
            Some(version),
            "{name} is not locked at {version}"
        );
        let source = entry
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if patches.is_some_and(|p| p.contains_key(name)) {
            eprintln!("{name}: temporary [patch.crates-io] source {source}");
        } else {
            assert_eq!(source, CRATES_IO, "{name} must come from crates.io");
        }
    }
    Ok(())
}

//! The worker's reported package version, Git source and immutable build agree
//! with both direct workspace dependencies and the resolved lockfile (ADR 0176).

const UPSTREAM: &str = "https://github.com/Chelis-Lang/dive";

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
    assert!(manifest.get("patch").is_none() && manifest.get("replace").is_none());
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock has no packages")?;
    for name in ["dive-api", "dive-ir"] {
        let spec = dependencies
            .get(name)
            .and_then(toml::Value::as_table)
            .ok_or("missing direct Dive dependency")?;
        assert_eq!(spec.len(), 3, "unexpected selectors on {name}");
        assert_eq!(
            spec.get("version").and_then(toml::Value::as_str),
            Some(format!("={version}").as_str())
        );
        assert_eq!(
            spec.get("git").and_then(toml::Value::as_str),
            Some(UPSTREAM)
        );
        assert_eq!(
            spec.get("rev").and_then(toml::Value::as_str),
            Some(sail_dive::edits::BACKEND_SOURCE)
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
    }
    Ok(())
}

#[test]
fn native_edit_source_matches_every_dive_package_in_the_lock() -> Result<(), String> {
    let lock = table(include_str!("../../../Cargo.lock"), "Cargo.lock")?;
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("missing packages")?;
    let sha = sail_dive::edits::BACKEND_SOURCE;
    assert!(sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()));
    let source = format!("git+{UPSTREAM}?rev={sha}#{sha}");
    for package in packages {
        if package
            .get("name")
            .and_then(toml::Value::as_str)
            .is_some_and(|n| n.starts_with("dive-"))
        {
            assert_eq!(
                package.get("source").and_then(toml::Value::as_str),
                Some(source.as_str())
            );
        }
    }
    Ok(())
}

//! Preobserve explicitly configured native headers before the first build.
//!
//! These are inventory hints, never script contracts or proof authority. Cargo's
//! actual declarations, complete before/after reads and scheduling checks still
//! decide whether the result is Current. Unknown layouts retain ordinary learning.

use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

pub(super) fn roots(data: &Value, environment: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for package in data["packages"].as_array().into_iter().flatten() {
        let Some(links) = package["links"].as_str().filter(|links| {
            !links.is_empty()
                && links
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        }) else {
            continue;
        };
        let prefix = links.to_ascii_uppercase();
        for (key, suffix) in [
            (format!("{prefix}_DIR"), Some("include")),
            (format!("{prefix}_INCLUDE_DIR"), None),
        ] {
            let Some(directory) = environment(&key)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
            else {
                continue;
            };
            let path = suffix.map_or_else(|| directory.clone(), |suffix| directory.join(suffix));
            if path.is_dir() {
                roots.insert(path);
            }
        }
    }
    roots.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::input::snapshot_metadata_home;
    use serde_json::json;

    #[test]
    fn first_declared_header_is_already_observed_and_changed_bytes_still_refuse()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-native-headers-{}", std::process::id()));
        std::fs::create_dir_all(root.join("package"))?;
        std::fs::create_dir_all(root.join("native/include"))?;
        std::fs::write(
            root.join("package/Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'\n",
        )?;
        let header = root.join("native/include/fixture.h");
        std::fs::write(&header, "#define VALUE 1\n")?;
        let data = json!({"workspace_root":root.join("package"),"packages":[{
            "links":"fixture", "manifest_path":root.join("package/Cargo.toml")}]});
        let hints = roots(&data, |key| {
            (key == "FIXTURE_DIR").then(|| root.join("native").into_os_string())
        });
        assert_eq!(hints, [root.join("native/include")]);
        let mut after_data = data.clone();
        after_data["sail_source_inputs"] = json!([header]);
        let before = snapshot_metadata_home(&data, &hints, None);
        let after = snapshot_metadata_home(&after_data, &hints, None);
        assert!(before.complete && after.complete);
        assert_eq!(before.digests, after.digests);
        let unhinted = snapshot_metadata_home(&data, &[], None);
        assert_ne!(unhinted.digests, after.digests);
        std::fs::write(&header, "#define VALUE 2\n")?;
        let changed = snapshot_metadata_home(&after_data, &hints, None);
        assert_ne!(before.digests, changed.digests);
        assert_eq!(roots(&data, |_| Some("relative".into())).len(), 0);
        assert_eq!(roots(&data, |_| None).len(), 0);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}

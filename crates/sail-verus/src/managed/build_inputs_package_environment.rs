//! Exact package and output-directory environment supplied by Cargo to scripts.

#![expect(
    clippy::option_option,
    reason = "Invalid evidence differs from an absent value."
)]

use super::BuildInput;
use serde_json::Value;
use std::path::Path;

pub(super) fn key(key: &str) -> bool {
    matches!(
        key,
        "CARGO_MANIFEST_DIR"
            | "CARGO_MANIFEST_LINKS"
            | "CARGO_PKG_NAME"
            | "CARGO_PKG_VERSION"
            | "CARGO_PKG_VERSION_MAJOR"
            | "CARGO_PKG_VERSION_MINOR"
            | "CARGO_PKG_VERSION_PATCH"
            | "CARGO_PKG_VERSION_PRE"
            | "OUT_DIR"
    )
}

pub(super) fn value(record: &BuildInput, data: &Value, key: &str) -> Option<Option<String>> {
    authority_value(record, data, key, std::env::var_os(key).is_some())
}

#[cfg(test)]
pub(super) fn uninherited(record: &BuildInput, data: &Value, key: &str) -> Option<Option<String>> {
    authority_value(record, data, key, false)
}

fn authority_value(
    record: &BuildInput,
    data: &Value,
    key: &str,
    inherited: bool,
) -> Option<Option<String>> {
    // Never infer child values from caller environment. A collision or an
    // unknown Cargo execution boundary remains ineligible for shared reuse.
    (self::key(key) && !inherited).then_some(())?;
    super::super::build_environment::package_values(data, &record.package_id)?;
    let mut packages = data["packages"]
        .as_array()?
        .iter()
        .filter(|package| package["id"].as_str() == Some(&record.package_id));
    let package = packages.next()?;
    packages.next().is_none().then_some(())?;
    (package["manifest_path"].as_str()? == record.manifest.to_str()?).then_some(())?;
    let result = match key {
        "CARGO_MANIFEST_DIR" => record.manifest.parent()?.to_str()?.to_owned(),
        "CARGO_PKG_NAME" => package["name"].as_str()?.to_owned(),
        "CARGO_MANIFEST_LINKS" => match package.get("links")? {
            Value::Null => return Some(None),
            value => value.as_str()?.to_owned(),
        },
        "OUT_DIR" => {
            let raw = record.unit_out_dir.as_deref()?;
            let path = Path::new(raw);
            (path.is_absolute()
                && path.file_name()? == "out"
                && path.parent()?.join("output") == record.output.as_ref()?.0)
                .then_some(())?;
            raw.to_owned()
        }
        _ => {
            let version = package["version"].as_str()?;
            let (core, pre) = version
                .split('+')
                .next()?
                .split_once('-')
                .unwrap_or((version.split('+').next()?, ""));
            let parts: Vec<&str> = core.split('.').collect();
            (parts.len() == 3
                && parts.iter().all(|part| {
                    part.parse::<u64>()
                        .is_ok_and(|number| number.to_string() == *part)
                }))
            .then_some(())?;
            match key {
                "CARGO_PKG_VERSION" => version,
                "CARGO_PKG_VERSION_MAJOR" => parts[0],
                "CARGO_PKG_VERSION_MINOR" => parts[1],
                "CARGO_PKG_VERSION_PATCH" => parts[2],
                "CARGO_PKG_VERSION_PRE" => pre,
                _ => return None,
            }
            .to_owned()
        }
    };
    Some(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn fixture() -> (BuildInput, Value) {
        let record = BuildInput {
            package_id: "pkg".to_owned(),
            package_name: "script".to_owned(),
            manifest: "/registry/script/Cargo.toml".into(),
            paths: Vec::new(),
            env: BTreeMap::new(),
            digest: String::new(),
            covered: true,
            output: Some(("/target/build/script/output".into(), "hash".to_owned())),
            unit_out_dir: Some("/target/build/script/out".to_owned()),
        };
        let data = json!({"packages":[{"id":"pkg", "name":"script", "version":"1.2.3-beta.4+build.5",
            "links":"script-native", "manifest_path":record.manifest}],
            "sail_build_environment":{"version":1,"packages":{"pkg":{"values":{}}}}});
        (record, data)
    }

    #[test]
    fn cargo_package_and_unit_values_are_exact() {
        let (record, mut data) = fixture();
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_NAME", true),
            None
        );
        for (key, expected) in [
            ("CARGO_MANIFEST_DIR", "/registry/script"),
            ("CARGO_PKG_NAME", "script"),
            ("CARGO_MANIFEST_LINKS", "script-native"),
            ("CARGO_PKG_VERSION", "1.2.3-beta.4+build.5"),
            ("CARGO_PKG_VERSION_MAJOR", "1"),
            ("CARGO_PKG_VERSION_MINOR", "2"),
            ("CARGO_PKG_VERSION_PATCH", "3"),
            ("CARGO_PKG_VERSION_PRE", "beta.4"),
            ("OUT_DIR", "/target/build/script/out"),
        ] {
            assert_eq!(
                authority_value(&record, &data, key, false),
                Some(Some(expected.to_owned()))
            );
        }
        data["packages"][0]["links"] = Value::Null;
        assert_eq!(
            authority_value(&record, &data, "CARGO_MANIFEST_LINKS", false),
            Some(None)
        );
        data["packages"][0]["version"] = json!("2.0.1");
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_VERSION_PRE", false),
            Some(Some(String::new()))
        );
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_VERSION_MAJOR", false),
            Some(Some("2".to_owned()))
        );
    }

    #[test]
    fn unknown_boundary_package_or_output_binding_is_refused() {
        let (mut record, mut data) = fixture();
        for raw in [
            "relative/out",
            "/target/build/other/out",
            "/target/build/script/not-out",
        ] {
            record.unit_out_dir = Some(raw.to_owned());
            assert_eq!(authority_value(&record, &data, "OUT_DIR", false), None);
        }
        record.unit_out_dir = None;
        assert_eq!(authority_value(&record, &data, "OUT_DIR", false), None);
        assert_eq!(
            authority_value(&record, &data, "CARGO_UNKNOWN", false),
            None
        );
        data["packages"][0]["version"] = json!("01.2.3");
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_VERSION_MAJOR", false),
            None
        );
        data["packages"][0]["manifest_path"] = json!("/other/Cargo.toml");
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_NAME", false),
            None
        );
        let (record, mut data) = fixture();
        let duplicate = data["packages"][0].clone();
        if let Some(packages) = data["packages"].as_array_mut() {
            packages.push(duplicate);
        }
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_NAME", false),
            None
        );
        let (record, mut data) = fixture();
        data["sail_build_environment"]["version"] = json!(2);
        assert_eq!(
            authority_value(&record, &data, "CARGO_PKG_NAME", false),
            None
        );
    }
}

//! Conservative Cargo target coverage for complete portfolio attempts.

use std::collections::BTreeSet;

use serde_json::Value;

#[derive(Default)]
struct Selection {
    workspace: bool,
    packages: Vec<String>,
    excluded: Vec<String>,
    lib: bool,
    bins: bool,
    named_bins: Vec<String>,
}

fn selection(args: &[String]) -> Result<Selection, String> {
    let mut selected = Selection::default();
    let mut args = args.iter().take_while(|a| *a != "--");
    while let Some(arg) = args.next() {
        let (flag, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        let mut value = || {
            inline
                .map(str::to_owned)
                .or_else(|| args.next().cloned())
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        match flag {
            "--workspace" | "--all" => selected.workspace = true,
            "-p" | "--package" => selected.packages.push(value()?),
            "--exclude" => selected.excluded.push(value()?),
            "--lib" => selected.lib = true,
            "--bins" => selected.bins = true,
            "--bin" => selected.named_bins.push(value()?),
            "--all-targets" | "--test" | "--tests" | "--example" | "--examples" | "--bench"
            | "--benches" => {
                return Err("portfolio target coverage supports libraries and binaries; select those targets explicitly".to_owned());
            }
            _ if arg.starts_with("-p") && arg.len() > 2 => {
                selected.packages.push(arg[2..].to_owned());
            }
            _ => {}
        }
    }
    Ok(selected)
}

fn matches(package: &Value, spec: &str) -> bool {
    package["id"].as_str() == Some(spec)
        || package["name"].as_str() == Some(spec)
        || package["name"]
            .as_str()
            .zip(package["version"].as_str())
            .is_some_and(|(name, version)| spec == format!("{name}@{version}"))
}

fn packages<'a>(metadata: &'a Value, selected: &Selection) -> Result<Vec<&'a Value>, String> {
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("Cargo metadata has no workspace members")?;
    let workspace: Vec<_> = metadata["packages"]
        .as_array()
        .ok_or("Cargo metadata has no packages")?
        .iter()
        .filter(|p| members.iter().any(|id| p.get("id") == Some(id)))
        .collect();
    for spec in selected.packages.iter().chain(&selected.excluded) {
        if workspace.iter().filter(|p| matches(p, spec)).count() != 1 {
            return Err(format!(
                "portfolio package selector {spec:?} must identify one exact workspace package"
            ));
        }
    }
    let defaults = metadata["workspace_default_members"]
        .as_array()
        .ok_or("Cargo metadata has no default members")?;
    Ok(workspace
        .into_iter()
        .filter(|p| {
            let included = selected.workspace
                || if selected.packages.is_empty() {
                    defaults.iter().any(|id| p.get("id") == Some(id))
                } else {
                    selected.packages.iter().any(|spec| matches(p, spec))
                };
            included
                && !selected.excluded.iter().any(|spec| matches(p, spec))
                && p.pointer("/metadata/verus/verify").and_then(Value::as_bool) == Some(true)
        })
        .collect())
}

/// All requested verifying library/binary targets must appear in the report.
pub(crate) fn required(metadata: &Value, args: &[String]) -> Result<Vec<String>, String> {
    let selected = selection(args)?;
    let explicit = selected.lib || selected.bins || !selected.named_bins.is_empty();
    let mut names = BTreeSet::new();
    for package in packages(metadata, &selected)? {
        let targets = package["targets"]
            .as_array()
            .ok_or("Cargo package has no targets")?;
        for target in targets {
            let name = target["name"].as_str().ok_or("Cargo target has no name")?;
            let kinds = target["kind"]
                .as_array()
                .ok_or("Cargo target has no kind")?;
            let lib = kinds.iter().any(|k| {
                matches!(
                    k.as_str(),
                    Some("lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro")
                )
            });
            let bin = kinds.iter().any(|k| k.as_str() == Some("bin"));
            let included = (lib && (!explicit || selected.lib))
                || (bin
                    && (!explicit
                        || selected.bins
                        || selected.named_bins.iter().any(|n| n == name)));
            if !included {
                continue;
            }
            if !names.insert(name.replace('-', "_")) {
                return Err(
                    "selected targets share a crate name; select one library or binary explicitly"
                        .to_owned(),
                );
            }
        }
    }
    if names.is_empty() {
        return Err("no selected verification-enabled library or binary targets".to_owned());
    }
    Ok(names.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> Value {
        serde_json::json!({"workspace_members":["a","b"],"workspace_default_members":["a"],"packages":[
            {"id":"a","name":"first","version":"1.0.0","metadata":{"verus":{"verify":true}},"targets":[{"name":"custom_lib","kind":["lib"]},{"name":"runner","kind":["bin"]}]},
            {"id":"b","name":"second","version":"1.0.0","metadata":{"verus":{"verify":true}},"targets":[{"name":"second","kind":["lib"]}]}
        ]})
    }
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }
    #[test]
    fn required_coverage_tracks_default_workspace_and_package_selection() {
        let m = metadata();
        assert_eq!(
            required(&m, &[]).ok(),
            Some(args(&["custom_lib", "runner"]))
        );
        assert_eq!(
            required(&m, &args(&["--workspace"])).ok(),
            Some(args(&["custom_lib", "runner", "second"]))
        );
        assert_eq!(
            required(&m, &args(&["-psecond"])).ok(),
            Some(args(&["second"]))
        );
        assert_eq!(
            required(&m, &args(&["--workspace", "--exclude=second", "--lib"])).ok(),
            Some(args(&["custom_lib"]))
        );
        assert_eq!(
            required(&m, &args(&["--package=first@1.0.0", "--bin", "runner"])).ok(),
            Some(args(&["runner"]))
        );
        assert!(required(&m, &args(&["-p", "unknown"])).is_err());
        assert!(required(&m, &args(&["--all-targets"])).is_err());
        let mut same_names = m.clone();
        same_names["packages"][1]["targets"][0]["name"] = Value::String("custom_lib".to_owned());
        assert!(required(&same_names, &args(&["--workspace"])).is_err());
        let mut rlib = m;
        rlib["packages"][0]["targets"][0]["kind"] = serde_json::json!(["rlib"]);
        assert_eq!(
            required(&rlib, &args(&["--lib"])).ok(),
            Some(args(&["custom_lib"]))
        );
    }
}

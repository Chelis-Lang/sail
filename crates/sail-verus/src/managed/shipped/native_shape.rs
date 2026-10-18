//! Restrict portable compiler changes to the observed closed Cargo shape.
//!
//! Keys still bind all original arguments. This is a separate eligibility
//! restriction: unsupported invocations execute their literal original args.

use super::Call;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
struct Shape {
    edition: String,
    builtin: bool,
    builtin_macros: bool,
    non_feature_cfg: BTreeSet<String>,
    check_cfg: Vec<String>,
    lints: BTreeSet<String>,
}

/// Fresh source admission precedes this function; private graph, compiler and
/// actual source-read authority still have their original independent gates.
pub(super) fn admitted(call: &Call<'_>) -> bool {
    let Ok(shapes) =
        serde_json::from_str::<BTreeMap<String, Shape>>(include_str!("native_shapes.json"))
    else {
        return false;
    };
    let Some(shape) = shapes.get(&call.package) else {
        return false;
    };
    let Some(package_id) = super::super::package_id(&|name| call.vars.get(name).cloned()) else {
        return false;
    };
    let role = |prefix: &str, expected: bool| {
        call.vars
            .get(&format!("{prefix}{package_id}"))
            .map(String::as_str)
            == expected.then_some("1")
    };
    if !role(" __VERUS_DRIVER_IS_BUILTIN_", shape.builtin)
        || !role(" __VERUS_DRIVER_IS_BUILTIN_MACROS_", shape.builtin_macros)
        || !role("__VERUS_DRIVER_VERIFY_", false)
        || call.vars.get("RUSTC_BOOTSTRAP").map(String::as_str) != Some("1")
        || ["RUSTC_LOG", "RUSTVERIFY_LOG"]
            .iter()
            .any(|name| call.vars.get(*name).is_some_and(|value| !value.is_empty()))
    {
        return false;
    }
    let Some(args) = call
        .arguments
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    arguments_admitted(shape, &call.package, &args)
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the closed compiler tuple parser and its refusal branches in one auditable predicate."
)]
fn arguments_admitted(shape: &Shape, package: &str, args: &[&str]) -> bool {
    if args
        .first()
        .and_then(|arg| std::path::Path::new(arg).file_name())
        != Some(std::ffi::OsStr::new("rustc"))
    {
        return false;
    }
    let mut scalar = BTreeMap::new();
    let mut codegen = BTreeMap::new();
    let mut cfg = BTreeSet::new();
    let mut check_cfg = BTreeMap::new();
    let mut lints = BTreeSet::new();
    let mut index = 1;
    while let Some(arg) = args.get(index) {
        // Forced warnings bypass Cargo's cap and expose remapped source names.
        if *arg == "--force-warn" || arg.starts_with("--force-warn=") {
            return false;
        }
        if let Some(count) = super::lint_arguments(&args[index..]) {
            if count != 1 || !shape.lints.contains(*arg) || !lints.insert(*arg) {
                return false;
            }
            index += count;
            continue;
        }
        if *arg == "-L" || *arg == "--extern" {
            // emitted_outputs/native_source and the unchanged full private
            // graph checks validate these actual lookup/artifact paths.
            index += 2;
            continue;
        }
        if arg.starts_with("--extern=") {
            index += 1;
            continue;
        }
        if *arg == "-Z" {
            if args.get(index + 1) != Some(&"checksum-hash-algorithm=blake3") {
                return false;
            }
            index += 2;
            continue;
        }
        if let Some(attached) = arg.strip_prefix("-C") {
            let (value, count) = if *arg == "-C" {
                let Some(value) = args.get(index + 1) else {
                    return false;
                };
                (*value, 2)
            } else {
                (attached, 1)
            };
            let (name, value) = value.split_once('=').unwrap_or((value, ""));
            if codegen.insert(name, value).is_some() {
                return false;
            }
            index += count;
            continue;
        }
        if let Some(flag) = arg.strip_prefix("--") {
            let (name, value, count) = if let Some((name, value)) = flag.split_once('=') {
                (name, value, 1)
            } else {
                let Some(value) = args.get(index + 1) else {
                    return false;
                };
                (flag, *value, 2)
            };
            let inserted = match name {
                "cfg" => cfg.insert(value),
                "check-cfg" => {
                    let count = check_cfg.entry(value).or_insert(0);
                    *count += 1;
                    *count
                        <= shape
                            .check_cfg
                            .iter()
                            .filter(|known| *known == value)
                            .count()
                }
                _ => scalar.insert(name, value).is_none(),
            };
            if !inserted {
                return false;
            }
            index += count;
            continue;
        }
        // native_source already required exactly the real manifest's one
        // source file. This guard never invents or substitutes that source.
        index += 1;
    }
    let non_feature: BTreeSet<_> = cfg
        .iter()
        .filter(|value| !value.starts_with("feature="))
        .copied()
        .collect();
    if non_feature != shape.non_feature_cfg.iter().map(String::as_str).collect()
        || scalar.get("cap-lints") != Some(&"allow")
        || scalar.get("edition").copied() != Some(shape.edition.as_str())
        || !matches!(
            scalar.get("emit").copied(),
            Some("dep-info,metadata" | "dep-info,link" | "dep-info,metadata,link")
        )
        || scalar
            .get("error-format")
            .is_some_and(|value| !matches!(*value, "human" | "json"))
        || scalar
            .get("json")
            .is_some_and(|value| *value != "diagnostic-rendered-ansi,artifacts,future-incompat")
        || scalar.keys().any(|name| {
            ![
                "crate-name",
                "crate-type",
                "edition",
                "emit",
                "out-dir",
                "error-format",
                "json",
                "cap-lints",
            ]
            .contains(name)
        })
    {
        return false;
    }
    let macro_crate = scalar.get("crate-type") == Some(&"proc-macro");
    codegen.get("embed-bitcode") == Some(&"no")
        && codegen.contains_key("extra-filename")
        && codegen.iter().all(|(name, value)| match *name {
            "embed-bitcode" => *value == "no",
            "prefer-dynamic" => macro_crate && value.is_empty(),
            "metadata" => value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "extra-filename" => {
                value.len() <= 128
                    && value.starts_with('-')
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            }
            "debuginfo" => package.starts_with("verus_builtin-") && *value == "2",
            "split-debuginfo" => package.starts_with("verus_builtin-") && *value == "unpacked",
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explicit external retained-data control, run only with the bounded
    /// source-qualified witness. No fixture can supply production authority.
    #[test]
    #[ignore = "requires the exact retained genuine Cargo tuple evidence"]
    fn actual_retained_native_shapes() -> Result<(), Box<dyn std::error::Error>> {
        let input = std::env::var_os("SAIL_ACTUAL_NATIVE_TUPLES")
            .ok_or("missing actual retained tuple evidence")?;
        let tuples: serde_json::Value = serde_json::from_slice(&std::fs::read(input)?)?;
        let shapes: BTreeMap<String, Shape> =
            serde_json::from_str(include_str!("native_shapes.json"))?;
        let rows = tuples["records"]
            .as_array()
            .ok_or("missing actual records")?;
        assert_eq!(rows.len(), 25);
        assert_eq!(shapes.len(), 14);
        let mut packages = BTreeSet::new();
        for row in rows {
            let package = row["package"].as_str().ok_or("missing recorded package")?;
            let args = row["actual_args"]
                .as_array()
                .ok_or("missing recorded arguments")?
                .iter()
                .map(|arg| arg.as_str().ok_or("non-string recorded argument"))
                .collect::<Result<Vec<_>, _>>()?;
            let shape = shapes.get(package).ok_or("unlisted recorded package")?;
            assert!(arguments_admitted(shape, package, &args), "{package}");
            packages.insert(package);
        }
        assert_eq!(packages.len(), 14);
        Ok(())
    }

    fn builtin_shape() -> Result<Shape, Box<dyn std::error::Error>> {
        let mut shapes: BTreeMap<String, Shape> =
            serde_json::from_str(include_str!("native_shapes.json"))?;
        shapes
            .remove("verus_builtin-0.0.0-2026-09-16-0054")
            .ok_or_else(|| "required builtin tuple missing".into())
    }

    #[test]
    fn qualified_builtin_rejects_warning_cfg_codegen_and_duplicate_overrides()
    -> Result<(), Box<dyn std::error::Error>> {
        let shape = builtin_shape()?;
        let original = [
            "rustc",
            "--crate-name",
            "verus_builtin",
            "--edition=2018",
            "/owned/registry/verus_builtin/src/lib.rs",
            "--crate-type",
            "lib",
            "--emit=dep-info,metadata",
            "--out-dir",
            "/owned/target/debug/deps",
            "-C",
            "extra-filename=-admission",
            "-C",
            "embed-bitcode=no",
            "-L",
            "dependency=/owned/target/debug/deps",
            "--cap-lints",
            "allow",
            "-Z",
            "checksum-hash-algorithm=blake3",
        ];
        assert!(arguments_admitted(
            &shape,
            "verus_builtin-0.0.0-2026-09-16-0054",
            &original
        ));
        for extra in [
            vec!["--force-warn", "missing_docs"],
            vec!["--force-warn=missing_docs"],
            vec!["--cfg", "verus_verify_core"],
            vec!["--cfg=sail_custom"],
            vec!["--cap-lints", "warn"],
            vec!["--edition=2024"],
            vec!["-C", "opt-level=invalid"],
            vec!["-C", "linker=/foreign/cc"],
            vec!["--error-format=invalid"],
            vec!["--check-cfg=invalid"],
        ] {
            let mut args = original.to_vec();
            args.extend(extra);
            assert!(!arguments_admitted(
                &shape,
                "verus_builtin-0.0.0-2026-09-16-0054",
                &args
            ));
        }
        let mut repeated_check_cfg = original.to_vec();
        repeated_check_cfg.extend(["--check-cfg", "cfg(docsrs,test)"]);
        assert!(arguments_admitted(
            &shape,
            "verus_builtin-0.0.0-2026-09-16-0054",
            &repeated_check_cfg
        ));
        repeated_check_cfg.extend(["--check-cfg", "cfg(docsrs,test)"]);
        assert!(!arguments_admitted(
            &shape,
            "verus_builtin-0.0.0-2026-09-16-0054",
            &repeated_check_cfg
        ));
        // Some genuine Cargo invocations repeat a build-script check-cfg.
        // Preserve that recorded multiplicity without accepting more copies.
        let mut repeated_shape = shape;
        repeated_shape.check_cfg.push("cfg(docsrs,test)".into());
        assert!(arguments_admitted(
            &repeated_shape,
            "verus_builtin-0.0.0-2026-09-16-0054",
            &repeated_check_cfg
        ));
        Ok(())
    }

    #[test]
    fn proc_macro2_custom_cfg_is_not_the_recorded_probe_tuple()
    -> Result<(), Box<dyn std::error::Error>> {
        let shapes: BTreeMap<String, Shape> =
            serde_json::from_str(include_str!("native_shapes.json"))?;
        let shape = shapes
            .get("proc-macro2-1.0.107")
            .ok_or("required proc-macro2 tuple missing")?;
        let mut args = vec![
            "rustc",
            "--crate-name",
            "proc_macro2",
            "--edition=2021",
            "/owned/registry/proc-macro2/src/lib.rs",
            "--crate-type",
            "lib",
            "--emit=dep-info,metadata,link",
            "--out-dir",
            "/owned/target/debug/deps",
            "-C",
            "extra-filename=-admission",
            "-C",
            "embed-bitcode=no",
            "-L",
            "dependency=/owned/target/debug/deps",
            "--cap-lints",
            "allow",
        ];
        for cfg in &shape.non_feature_cfg {
            args.extend(["--cfg", cfg.as_str()]);
        }
        assert!(arguments_admitted(shape, "proc-macro2-1.0.107", &args));
        args.extend(["--cfg", "procmacro2_semver_exempt"]);
        assert!(!arguments_admitted(shape, "proc-macro2-1.0.107", &args));
        Ok(())
    }
}

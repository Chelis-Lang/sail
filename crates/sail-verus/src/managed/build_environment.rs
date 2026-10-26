//! Exact Cargo execution-unit values for declared script environment inputs.

#![expect(
    clippy::option_option,
    reason = "Nested options distinguish invalid evidence, absent values, and present values."
)]

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

pub(crate) const REQUEST: &str = "SAIL_MANAGED_BUILD_ENVIRONMENT";
const VERSION: u64 = 1;
const CARGO_COMMIT: &str = "commit-hash: 5f94df4789f005f9a352888e8355ffc645b7ed0e";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    version: u64,
    success: bool,
    target: Value,
    arguments: Vec<Vec<u8>>,
    cfg_stdout: Vec<u8>,
}

#[derive(Serialize)]
struct Authority {
    version: u64,
    packages: BTreeMap<String, Package>,
}

#[derive(PartialEq, Eq, Serialize)]
struct Package {
    values: BTreeMap<String, String>,
}

struct CfgObservation<'a> {
    source: &'a Observation,
    arguments: Vec<usize>,
    cfg: Vec<(&'a str, Option<&'a str>)>,
}

/// Execute Cargo's native query before any compiler-cache observation.
pub(crate) fn probe(wrapper: &Path, args: &[OsString]) -> Option<Result<ExitCode>> {
    let directory = super::transport::var(REQUEST)?;
    let arguments: Vec<Vec<u8>> = args
        .iter()
        .map(|arg| arg.as_encoded_bytes().to_vec())
        .collect();
    let retained = cfg_arguments(&arguments)?;
    Some((|| {
        let output = Command::new(wrapper)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| Error::new("SAIL3111", error.to_string()))?;
        std::io::stdout()
            .write_all(&output.stdout)
            .and_then(|()| std::io::stderr().write_all(&output.stderr))
            .map_err(|error| Error::new("SAIL3111", error.to_string()))?;
        let cfg = output.status.success().then(|| {
            Command::new(wrapper)
                .args(retained.iter().map(|index| &args[*index]))
                .arg("--print=cfg")
                .stdin(Stdio::null())
                .output()
        });
        let success = cfg
            .as_ref()
            .is_some_and(|result| result.as_ref().is_ok_and(|output| output.status.success()));
        let observation = Observation {
            version: VERSION,
            success,
            target: target(&arguments)
                .flatten()
                .map_or(Value::Null, Value::String),
            arguments,
            cfg_stdout: cfg
                .and_then(std::result::Result::ok)
                .map_or_else(Vec::new, |output| output.stdout),
        };
        let path = Path::new(&directory).join(format!("cfg-{}.json", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| Error::new("SAIL3111", error.to_string()))?;
        serde_json::to_writer(&mut file, &observation)
            .map_err(|error| Error::new("SAIL3111", error.to_string()))?;
        Ok(ExitCode::from(
            output
                .status
                .code()
                .and_then(|code| u8::try_from(code & 255).ok())
                .unwrap_or(1),
        ))
    })())
}

// Keep every non-print argument in its original position and byte form.
fn cfg_arguments(arguments: &[Vec<u8>]) -> Option<Vec<usize>> {
    let mut retained = Vec::new();
    let mut cfg = false;
    let mut index = 0;
    while index < arguments.len() {
        let arg = arguments[index].as_slice();
        if arg == b"--" {
            retained.extend(index..arguments.len());
            break;
        }
        let print = if arg == b"--print" {
            index += 1;
            Some(arguments.get(index)?.as_slice())
        } else {
            arg.strip_prefix(b"--print=")
        };
        if let Some(print) = print {
            cfg |= print.split(|byte| *byte == b',').any(|name| name == b"cfg");
        } else {
            retained.push(index);
        }
        index += 1;
    }
    cfg.then_some(retained)
}

fn target(arguments: &[Vec<u8>]) -> Option<Option<String>> {
    let mut target = None;
    let mut index = 0;
    while index < arguments.len() {
        let arg = arguments[index].as_slice();
        if arg == b"--" {
            break;
        }
        let value = if arg == b"--target" {
            index += 1;
            Some(arguments.get(index)?.as_slice())
        } else {
            arg.strip_prefix(b"--target=")
        };
        if let Some(value) = value {
            if target.is_some() || value.is_empty() {
                return None;
            }
            target = Some(std::str::from_utf8(value).ok()?.to_owned());
        }
        index += 1;
    }
    Some(target)
}

pub(super) fn load(directory: &Path, metadata: &Value) -> Option<Value> {
    let version = std::fs::read_to_string(directory.join("cargo-version.txt")).ok()?;
    version
        .lines()
        .any(|line| line == CARGO_COMMIT)
        .then_some(())?;
    let graph: Value =
        serde_json::from_slice(&std::fs::read(directory.join("graph.json")).ok()?).ok()?;
    let mut observations = Vec::new();
    for entry in std::fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_str()?;
        if name.starts_with("cfg-") && entry.path().extension().is_some_and(|ext| ext == "json") {
            let bytes = std::fs::read(entry.path()).ok()?;
            observations.push(serde_json::from_slice::<Observation>(&bytes).ok()?);
        }
    }
    derive(&graph, metadata, &observations)
}

/// A reported Cargo version outside the qualified script-input protocol.
/// Missing or malformed observations do not identify an unsupported Cargo.
pub(super) fn unsupported_cargo(directory: &Path) -> Option<String> {
    let text = std::fs::read_to_string(directory.join("cargo-version.txt")).ok()?;
    if text.lines().any(|line| line == CARGO_COMMIT) {
        return None;
    }
    let commit = text
        .lines()
        .find_map(|line| line.strip_prefix("commit-hash: "))?;
    (commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(())?;
    let version = text
        .lines()
        .next()?
        .strip_prefix("cargo ")?
        .split_whitespace()
        .next()?;
    Some(version.to_owned())
}

fn derive(graph: &Value, metadata: &Value, observations: &[Observation]) -> Option<Value> {
    (graph.get("version")?.as_u64()? == VERSION).then_some(())?;
    let mut identities = BTreeSet::new();
    for package in metadata.get("packages")?.as_array()? {
        identities
            .insert(package.get("id")?.as_str()?)
            .then_some(())?;
    }
    let mut cfgs: BTreeMap<Option<String>, Option<CfgObservation<'_>>> = BTreeMap::new();
    for observation in observations {
        (observation.version == VERSION && observation.success).then_some(())?;
        let identity = target(&observation.arguments)?;
        let recorded = platform(&observation.target)?;
        (identity == recorded).then_some(())?;
        let arguments = cfg_arguments(&observation.arguments)?;
        let cfg = parse_cfg(&observation.cfg_stdout)?;
        match cfgs.entry(identity) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Some(CfgObservation {
                    source: observation,
                    arguments,
                    cfg,
                }));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if let Some(previous) = entry.get() {
                    let same_args = previous
                        .arguments
                        .iter()
                        .map(|index| &previous.source.arguments[*index])
                        .eq(arguments.iter().map(|index| &observation.arguments[*index]));
                    if !same_args || previous.source.cfg_stdout != observation.cfg_stdout {
                        entry.insert(None);
                    }
                }
            }
        }
    }
    let mut packages: BTreeMap<String, Option<Package>> = BTreeMap::new();
    for unit in graph.get("units")?.as_array()? {
        if unit.get("mode")?.as_str()? != "run-custom-build" {
            continue;
        }
        let id = unit.get("pkg_id")?.as_str()?;
        identities.contains(id).then_some(())?;
        let unit_platform = platform(unit.get("platform")?)?;
        let features: Vec<&str> = unit
            .get("features")?
            .as_array()?
            .iter()
            .map(Value::as_str)
            .collect::<Option<_>>()?;
        let debug_assertions = unit.get("profile")?.get("debug_assertions")?.as_bool()?;
        let generated = cfgs
            .get(&unit_platform)
            .and_then(Option::as_ref)
            .and_then(|observation| generated(&features, debug_assertions, &observation.cfg));
        match packages.entry(id.to_owned()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(generated);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get() != &generated {
                    entry.insert(None);
                }
            }
        }
    }
    let packages = packages
        .into_iter()
        .filter_map(|(id, package)| package.map(|package| (id, package)))
        .collect();
    serde_json::to_value(Authority {
        version: VERSION,
        packages,
    })
    .ok()
}

fn platform(value: &Value) -> Option<Option<String>> {
    if value.is_null() {
        Some(None)
    } else {
        Some(Some(value.as_str()?.to_owned()))
    }
}

// Match Cargo 5f94df4789f005f9a352888e8355ffc645b7ed0e:
// src/compiler/custom_build.rs and build_context/target_info.rs.
fn generated(
    features: &[&str],
    debug_assertions: bool,
    observations: &[(&str, Option<&str>)],
) -> Option<Package> {
    let mut values = BTreeMap::new();
    for feature in features {
        values.insert(format!("CARGO_FEATURE_{}", envify(feature)), "1".to_owned());
    }
    let mut cfg = BTreeMap::new();
    cfg.insert("feature", features.to_vec());
    if debug_assertions {
        cfg.insert("debug_assertions", Vec::new());
    }
    for &(name, value) in observations {
        if let Some(value) = value {
            cfg.entry(name).or_default().push(value);
        } else if name != "proc_macro" && name != "debug_assertions" {
            cfg.insert(name, Vec::new());
        }
    }
    for (name, entries) in cfg {
        let key = format!("CARGO_CFG_{}", envify(name));
        let value = entries.join(",");
        match values.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                // Cargo's map iteration cannot resolve a converted-name collision.
                (entry.get() == &value).then_some(())?;
            }
        }
    }
    Some(Package { values })
}

fn envify(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_uppercase)
        .map(|ch| if ch == '-' { '_' } else { ch })
        .collect()
}

// Cargo's cfg parser accepts ASCII identifiers, raw identifiers, and spaces.
// It preserves quoted content without JSON or Rust string unescaping.
fn parse_cfg(stdout: &[u8]) -> Option<Vec<(&str, Option<&str>)>> {
    let text = std::str::from_utf8(stdout).ok()?;
    (!text.is_empty()).then_some(())?;
    text.lines()
        .map(|line| {
            let line = line.trim_matches(' ');
            let line = line.strip_prefix("r#").unwrap_or(line);
            let end = line
                .find(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
                .unwrap_or(line.len());
            let name = &line[..end];
            let first = name.chars().next()?;
            (first == '_' || first.is_ascii_alphabetic()).then_some(())?;
            let rest = line[end..].trim_start_matches(' ');
            if rest.is_empty() {
                return Some((name, None));
            }
            let rest = rest
                .strip_prefix('=')?
                .trim_start_matches(' ')
                .strip_prefix('"')?;
            let end = rest.find('"')?;
            rest[end + 1..].chars().all(|ch| ch == ' ').then_some(())?;
            let value = &rest[..end];
            (!value.contains('\0')).then_some(())?;
            Some((name, Some(value)))
        })
        .collect()
}

pub(super) fn value(data: &Value, package_id: &str, key: &str) -> Option<Option<String>> {
    authority_value(data, package_id, key, std::env::var_os(key).is_some())
}

fn generated_key(key: &str) -> bool {
    key.starts_with("CARGO_FEATURE_") || key.starts_with("CARGO_CFG_")
}

fn authority_value(
    data: &Value,
    package_id: &str,
    key: &str,
    parent: bool,
) -> Option<Option<String>> {
    (generated_key(key) && !parent).then_some(())?;
    let values = package_values(data, package_id)?;
    Some(
        values
            .get(key)
            .map(|value| value.as_str().unwrap_or_default().to_owned()),
    )
}

/// Current execution-unit authority, installed only for the pinned Cargo boundary.
pub(super) fn package_values<'a>(
    data: &'a Value,
    package_id: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    let authority = data.get("sail_build_environment")?.as_object()?;
    (authority.len() == 2 && authority.get("version")?.as_u64()? == VERSION).then_some(())?;
    let package = authority
        .get("packages")?
        .as_object()?
        .get(package_id)?
        .as_object()?;
    (package.len() == 1).then_some(())?;
    let values = package.get("values")?.as_object()?;
    values
        .iter()
        .all(|(key, value)| generated_key(key) && value.is_string())
        .then_some(())?;
    Some(values)
}

const CACHE_VERSION: u64 = 1;
const CACHE_FILE: &str = "build-environment.json";

/// A retained query: the exact files one complete query wrote, under the key
/// of everything that query depends on ([`cache_key`]).
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    version: u64,
    key: String,
    files: BTreeMap<String, String>,
}

/// The key of one execution-unit query. The proof context binds the exact
/// arguments (features, package and target selection, profile flags), every
/// tool identity (Cargo, rustc, Cargo-Verus and their sysroots), the
/// environment families Cargo and rustc read (`CARGO_*`, `RUST*`,
/// `RUSTFLAGS`), the bytes of every Cargo configuration and toolchain file,
/// and named target specifications. This key adds what the proof context
/// leaves to the source revision but Cargo's unit graph reads: the complete
/// metadata (packages, targets Cargo discovered, dependency edges and the
/// resolve), the bytes of every manifest (profiles are not in metadata) and of
/// the lockfile, the invoking directory and the owned target directory.
/// Source contents are not part of it: Cargo plans units without them.
/// `None` (no reuse) when a configuration file may include another file.
pub(super) fn cache_key(
    context: &str,
    data: &Value,
    invocation: &Path,
    target: &Path,
    configuration: &[std::path::PathBuf],
) -> Option<String> {
    for path in configuration {
        match std::fs::read(path) {
            Ok(bytes) => {
                // An included configuration file is not in the proof context.
                bytes
                    .windows(b"include".len())
                    .all(|window| window != b"include")
                    .then_some(())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    let metadata: Vec<(&String, &Value)> = data
        .as_object()?
        .iter()
        .filter(|(key, _)| !key.starts_with("sail_"))
        .collect();
    let workspace = Path::new(data.get("workspace_root")?.as_str()?);
    let mut files = BTreeSet::from([workspace.join("Cargo.toml"), workspace.join("Cargo.lock")]);
    for package in data.get("packages")?.as_array()? {
        files.insert(std::path::PathBuf::from(
            package.get("manifest_path")?.as_str()?,
        ));
    }
    let mut digests = Vec::with_capacity(files.len());
    for path in &files {
        let digest = match std::fs::read(path) {
            Ok(bytes) => super::files::digest(&bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "missing".to_owned(),
            Err(_) => return None,
        };
        digests.push((path, digest));
    }
    let bytes = serde_json::to_vec(&(
        "sail-build-environment-cache",
        CACHE_VERSION,
        context,
        invocation,
        target,
        metadata,
        digests,
    ))
    .ok()?;
    Some(super::files::digest(&bytes))
}

/// Restores the retained query of `key` into the empty `directory`, as the
/// query itself would have written it. The caller then loads it exactly as a
/// fresh query; a unit graph naming any target the current metadata does not
/// list exactly refuses.
pub(super) fn restore(context: &Path, key: &str, directory: &Path, metadata: &Value) -> bool {
    let restored = || -> Option<()> {
        let retained: Retained =
            serde_json::from_slice(&std::fs::read(context.join(CACHE_FILE)).ok()?).ok()?;
        (retained.version == CACHE_VERSION && retained.key == key).then_some(())?;
        let graph: Value = serde_json::from_str(retained.files.get("graph.json")?).ok()?;
        consistent(&graph, metadata)?;
        for (name, text) in &retained.files {
            retained_name(name).then_some(())?;
            std::fs::write(directory.join(name), text).ok()?;
        }
        Some(())
    };
    restored().is_some()
}

/// Retains this run's complete query for `key`. Only a query whose files load
/// as complete authority, with no custom target specification (its bytes are
/// not bound), is retained. Failure to retain changes nothing.
pub(super) fn retain(context: &Path, key: &str, directory: &Path, metadata: &Value) {
    let retained = || -> Option<Retained> {
        load(directory, metadata)?;
        let mut files = BTreeMap::new();
        for entry in std::fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if !retained_name(&name) {
                continue;
            }
            let text = String::from_utf8(std::fs::read(entry.path()).ok()?).ok()?;
            if name.starts_with("cfg-") {
                let observation: Observation = serde_json::from_str(&text).ok()?;
                target(&observation.arguments)?
                    .is_none_or(|target| {
                        !target.contains('/')
                            && !Path::new(&target)
                                .extension()
                                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                    })
                    .then_some(())?;
            }
            files.insert(name, text);
        }
        let graph: Value = serde_json::from_str(files.get("graph.json")?).ok()?;
        consistent(&graph, metadata)?;
        files.contains_key("cargo-version.txt").then_some(())?;
        Some(Retained {
            version: CACHE_VERSION,
            key: key.to_owned(),
            files,
        })
    };
    if let Some(retained) = retained() {
        let _ = super::files::json(&context.join(CACHE_FILE), &retained);
    }
}

fn retained_name(name: &str) -> bool {
    name == "graph.json"
        || name == "cargo-version.txt"
        || (name.starts_with("cfg-")
            && Path::new(name).extension().is_some_and(|ext| ext == "json")
            && !name.contains('/'))
}

/// Every unit's package and target is exactly one the metadata lists.
fn consistent(graph: &Value, metadata: &Value) -> Option<()> {
    let mut targets = BTreeSet::new();
    for package in metadata.get("packages")?.as_array()? {
        let id = package.get("id")?.as_str()?;
        for target in package.get("targets")?.as_array()? {
            targets.insert((
                id,
                target.get("name")?.as_str()?,
                target.get("kind")?.to_string(),
                target.get("src_path")?.as_str()?,
            ));
        }
    }
    for unit in graph.get("units")?.as_array()? {
        let target = unit.get("target")?;
        targets
            .contains(&(
                unit.get("pkg_id")?.as_str()?,
                target.get("name")?.as_str()?,
                target.get("kind")?.to_string(),
                target.get("src_path")?.as_str()?,
            ))
            .then_some(())?;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID: &str = "path+file:///exact#script@0.1.0";
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    fn metadata() -> Value {
        json!({"packages":[{"id":ID,"features":{"unused":[]}}],
            "resolve":{"nodes":[{"id":ID,"features":["metadata-union"]}]}})
    }

    fn unit(features: &[&str], platform: Value) -> Value {
        let mut unit = json!({"pkg_id":ID,"mode":"run-custom-build","features":features,
            "profile":{"debug_assertions":true}});
        unit["platform"] = platform;
        unit
    }

    fn graph(units: Vec<Value>) -> Value {
        json!({"version":1,"units":Value::Array(units),"roots":[0]})
    }

    fn observation(target: Option<&str>, cfg: &[u8]) -> Observation {
        let mut arguments = vec![b"rustc".to_vec(), b"-".to_vec(), b"--print=cfg".to_vec()];
        if let Some(target) = target {
            arguments.extend([b"--target".to_vec(), target.as_bytes().to_vec()]);
        }
        Observation {
            version: VERSION,
            success: true,
            target: target.map_or(Value::Null, |target| json!(target)),
            arguments,
            cfg_stdout: cfg.to_vec(),
        }
    }

    fn data(graph: &Value, observations: &[Observation]) -> Value {
        json!({"sail_build_environment":derive(graph, &metadata(), observations)})
    }

    fn get(data: &Value, key: &str) -> Option<Option<String>> {
        authority_value(data, ID, key, false)
    }

    #[test]
    fn exact_unit_features_replace_metadata_union() {
        let first = data(
            &graph(vec![unit(&["fast-path", "ß"], Value::Null)]),
            &[observation(None, b"unix\nfeature=\"query\"\n")],
        );
        assert_eq!(
            get(&first, "CARGO_FEATURE_FAST_PATH"),
            Some(Some("1".to_owned()))
        );
        assert_eq!(get(&first, "CARGO_FEATURE_SS"), Some(Some("1".to_owned())));
        assert_eq!(get(&first, "CARGO_FEATURE_METADATA_UNION"), Some(None));
        assert_eq!(
            get(&first, "CARGO_CFG_FEATURE"),
            Some(Some("fast-path,ß,query".to_owned()))
        );
        let changed = data(
            &graph(vec![unit(&["slow"], Value::Null)]),
            &[observation(None, b"unix\nfeature=\"query\"\n")],
        );
        assert_eq!(get(&changed, "CARGO_FEATURE_FAST_PATH"), Some(None));
        assert_eq!(
            get(&changed, "CARGO_CFG_FEATURE"),
            Some(Some("slow,query".to_owned()))
        );
    }

    #[test]
    fn cfg_values_preserve_order_and_change_with_queries() {
        let first = data(
            &graph(vec![unit(&[], Value::Null)]),
            &[observation(
                None,
                b"target_feature=\"z\"\ntarget_feature=\"a\"\ntarget_feature=\"z\"\nunix\n",
            )],
        );
        assert_eq!(
            get(&first, "CARGO_CFG_TARGET_FEATURE"),
            Some(Some("z,a,z".to_owned()))
        );
        assert_eq!(get(&first, "CARGO_CFG_UNIX"), Some(Some(String::new())));
        let changed = data(
            &graph(vec![unit(&[], Value::Null)]),
            &[observation(None, b"target_feature=\"b\"\nwindows\n")],
        );
        assert_eq!(
            get(&changed, "CARGO_CFG_TARGET_FEATURE"),
            Some(Some("b".to_owned()))
        );
        assert_eq!(get(&changed, "CARGO_CFG_UNIX"), Some(None));
    }

    #[test]
    fn profile_controls_debug_assertions_and_query_controls_overflow_checks() {
        let mut execution = unit(&[], Value::Null);
        execution["profile"]["debug_assertions"] = json!(false);
        let observed = b"debug_assertions\noverflow_checks\nproc_macro\n";
        let release = data(&graph(vec![execution]), &[observation(None, observed)]);
        assert_eq!(get(&release, "CARGO_CFG_DEBUG_ASSERTIONS"), Some(None));
        assert_eq!(
            get(&release, "CARGO_CFG_OVERFLOW_CHECKS"),
            Some(Some(String::new()))
        );
        assert_eq!(get(&release, "CARGO_CFG_PROC_MACRO"), Some(None));
        let debug = data(
            &graph(vec![unit(&[], Value::Null)]),
            &[observation(None, observed)],
        );
        assert_eq!(
            get(&debug, "CARGO_CFG_DEBUG_ASSERTIONS"),
            Some(Some(String::new()))
        );
    }

    #[test]
    fn absence_needs_complete_unshadowed_package_authority() {
        let complete = data(
            &graph(vec![unit(&[], Value::Null)]),
            &[observation(None, b"unix\n")],
        );
        assert_eq!(get(&complete, "CARGO_FEATURE_UNKNOWN"), Some(None));
        assert_eq!(get(&complete, "CARGO_CFG_UNKNOWN"), Some(None));
        assert_eq!(
            get(&complete, "CARGO_CFG_FEATURE"),
            Some(Some(String::new()))
        );
        assert_eq!(get(&complete, "CARGO_PKG_NAME"), None);
        assert_eq!(get(&complete, "DEP_NATIVE_ROOT"), None);
        assert_eq!(
            authority_value(&complete, "other", "CARGO_CFG_UNIX", false),
            None
        );
        assert_eq!(authority_value(&complete, ID, "CARGO_CFG_UNIX", true), None);
        assert_eq!(
            authority_value(&complete, ID, "CARGO_FEATURE_UNKNOWN", true),
            None
        );
    }

    #[test]
    fn different_unit_values_refuse_package_authority() {
        let ambiguous = data(
            &graph(vec![unit(&["a"], Value::Null), unit(&["b"], Value::Null)]),
            &[observation(None, b"unix\n")],
        );
        assert_eq!(get(&ambiguous, "CARGO_FEATURE_A"), None);
        let equal = data(
            &graph(vec![unit(&["a"], Value::Null), unit(&["a"], Value::Null)]),
            &[observation(None, b"unix\n")],
        );
        assert_eq!(get(&equal, "CARGO_FEATURE_A"), Some(Some("1".to_owned())));
        let platforms = data(
            &graph(vec![
                unit(&[], Value::Null),
                unit(&[], json!("custom.json")),
            ]),
            &[
                observation(None, b"unix\n"),
                observation(Some("custom.json"), b"windows\n"),
            ],
        );
        assert_eq!(get(&platforms, "CARGO_CFG_UNIX"), None);
    }

    #[test]
    fn ambiguous_target_queries_do_not_select_one_record() {
        let first = observation(None, b"unix\n");
        let mut second = observation(None, b"unix\n");
        second
            .arguments
            .extend([b"--cfg".to_vec(), b"extra".to_vec()]);
        let ambiguous = data(&graph(vec![unit(&[], Value::Null)]), &[first, second]);
        assert_eq!(get(&ambiguous, "CARGO_CFG_UNIX"), None);
        let ambiguous = data(
            &graph(vec![unit(&[], Value::Null)]),
            &[
                observation(None, b"unix\n"),
                observation(None, b"windows\n"),
            ],
        );
        assert_eq!(get(&ambiguous, "CARGO_CFG_UNIX"), None);
    }

    #[test]
    fn unknown_schemas_and_incomplete_observations_refuse_authority() {
        let mut unknown = graph(vec![unit(&[], Value::Null)]);
        unknown["version"] = json!(2);
        assert!(derive(&unknown, &metadata(), &[observation(None, b"unix\n")]).is_none());
        let mut unknown = observation(None, b"unix\n");
        unknown.version = 2;
        assert!(
            derive(
                &graph(vec![unit(&[], Value::Null)]),
                &metadata(),
                &[unknown]
            )
            .is_none()
        );
        let missing = data(&graph(vec![unit(&[], Value::Null)]), &[]);
        assert_eq!(get(&missing, "CARGO_FEATURE_UNKNOWN"), None);
        let missing_target = data(
            &graph(vec![unit(&[], json!("custom.json"))]),
            &[observation(None, b"unix\n")],
        );
        assert_eq!(get(&missing_target, "CARGO_CFG_UNIX"), None);
        let mut failed = observation(None, b"unix\n");
        failed.success = false;
        assert!(derive(&graph(vec![unit(&[], Value::Null)]), &metadata(), &[failed]).is_none());
        assert!(
            serde_json::from_value::<Observation>(json!({"version":1,"success":true,
            "arguments":[],"cfg_stdout":[]}))
            .is_err()
        );
        let mut unknown = data(
            &graph(vec![unit(&[], Value::Null)]),
            &[observation(None, b"unix\n")],
        );
        unknown["sail_build_environment"]["version"] = json!(2);
        assert_eq!(get(&unknown, "CARGO_CFG_UNIX"), None);
    }

    #[test]
    fn missing_profile_and_nonidentical_package_ids_refuse_authority() -> TestResult {
        let mut execution = unit(&[], Value::Null);
        execution
            .as_object_mut()
            .ok_or("unit object absent")?
            .remove("profile");
        assert!(
            derive(
                &graph(vec![execution]),
                &metadata(),
                &[observation(None, b"unix\n")]
            )
            .is_none()
        );
        let mut execution = unit(&[], Value::Null);
        execution["pkg_id"] = json!("script 0.1.0 (path+file:///exact)");
        assert!(
            derive(
                &graph(vec![execution]),
                &metadata(),
                &[observation(None, b"unix\n")]
            )
            .is_none()
        );
        Ok(())
    }

    #[test]
    fn strict_cfg_parser_matches_cargo_string_rules() -> TestResult {
        assert_eq!(
            parse_cfg(b" r#type = \"a\\nb\" \n"),
            Some(vec![("type", Some("a\\nb"))])
        );
        for invalid in [
            b"".as_slice(),
            b"\xff",
            b"unix\n\n",
            b"name=bare",
            b"name=\"x\" trailing",
            b"name=\"x",
            b"1name",
            b"name\t",
            b"all(unix)",
            b"name=\"x\"=\"y\"",
        ] {
            assert!(parse_cfg(invalid).is_none(), "{invalid:?}");
        }
        let cfg = parse_cfg(b"lower=\"one\"\nLOWER=\"two\"\n").ok_or("cfg parse failed")?;
        let collision = generated(&[], true, &cfg);
        assert!(collision.is_none());
        Ok(())
    }

    #[test]
    fn cfg_only_query_keeps_native_arguments_and_exact_target_spelling() {
        let arguments = vec![
            b"rustc".to_vec(),
            b"--print=file-names".to_vec(),
            b"--print".to_vec(),
            b"cfg".to_vec(),
            b"--target=./custom.json".to_vec(),
            b"--cfg".to_vec(),
            b"feature=\"fast\"".to_vec(),
            vec![0xff],
        ];
        assert_eq!(cfg_arguments(&arguments), Some(vec![0, 4, 5, 6, 7]));
        assert_eq!(target(&arguments), Some(Some("./custom.json".to_owned())));
        assert_eq!(target(&[b"rustc".to_vec()]), Some(None));
        assert_eq!(cfg_arguments(&[b"--print=sysroot".to_vec()]), None);
        assert_eq!(target(&[b"--target".to_vec()]), None);
    }
    struct CargoFixture {
        root: std::path::PathBuf,
        cargo: OsString,
    }

    impl Drop for CargoFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl CargoFixture {
        fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let fixture = Self {
                root: std::env::temp_dir().join(format!(
                    "sail-build-environment-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                )),
                cargo: std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
            };
            std::fs::create_dir_all(fixture.root.join("src"))?;
            std::fs::write(
                fixture.root.join("Cargo.toml"),
                "[package]\nname='environment-fixture'\nversion='0.1.0'\nedition='2024'\n\
                 [features]\nfast-path=[]\nunused=[]\n[workspace]\n",
            )?;
            std::fs::write(fixture.root.join("src/lib.rs"), "")?;
            std::fs::write(
                fixture.root.join("build.rs"),
                r#"fn main() {
    let mut values: Vec<_> = std::env::vars().filter(|(key, _)| {
        key.starts_with("CARGO_FEATURE_") || key.starts_with("CARGO_CFG_")
    }).collect();
    values.sort();
    let mut text = String::new();
    for (key, value) in values {
        text.push_str(&key);
        text.push('\t');
        text.push_str(&value);
        text.push('\n');
    }
    std::fs::write(std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap())
        .join("environment.txt"), text).unwrap();
}
"#,
            )?;
            Ok(fixture)
        }

        fn command(&self, program: &std::ffi::OsStr) -> Command {
            let mut command = Command::new(program);
            command.current_dir(&self.root);
            for (key, _) in std::env::vars_os() {
                if key.to_str().is_some_and(|key| key.starts_with("CARGO_"))
                    || matches!(
                        key.to_str(),
                        Some(
                            "RUSTC"
                                | "RUSTC_WRAPPER"
                                | "RUSTC_WORKSPACE_WRAPPER"
                                | "RUSTFLAGS"
                                | "RUSTDOCFLAGS"
                                | "RUSTC_LOG"
                        )
                    )
                {
                    command.env_remove(key);
                }
            }
            command
                .env("CARGO_HOME", self.root.join("cargo-home"))
                .env("RUSTC_BOOTSTRAP", "1")
                .env("RUSTFLAGS", "--cfg=sail_environment_fixture=\"native\"");
            command
        }

        fn cargo(&self, args: &[&str]) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
            let output = self.command(&self.cargo).args(args).output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(output.stdout)
        }
    }

    #[test]
    fn real_cargo_script_values_match_exact_unit_authority()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = CargoFixture::new()?;
        let metadata: Value = serde_json::from_slice(&fixture.cargo(&[
            "metadata",
            "--offline",
            "--format-version=1",
            "--no-deps",
        ])?)?;
        let id = metadata["packages"][0]["id"]
            .as_str()
            .ok_or("package ID absent")?;
        let query_arguments: Vec<OsString> = [
            "-",
            "--crate-name",
            "___",
            "--crate-type=bin",
            "--crate-type=rlib",
            "--crate-type=dylib",
            "--crate-type=cdylib",
            "--crate-type=staticlib",
            "--crate-type=proc-macro",
            "--cfg=sail_environment_fixture=\"native\"",
            "--print=cfg",
            "-Wwarnings",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let cfg = fixture
            .command(std::ffi::OsStr::new("rustc"))
            .args(&query_arguments)
            .stdin(Stdio::null())
            .output()?;
        assert!(
            cfg.status.success(),
            "{}",
            String::from_utf8_lossy(&cfg.stderr)
        );
        let observed = Observation {
            version: VERSION,
            success: true,
            target: Value::Null,
            arguments: query_arguments
                .into_iter()
                .map(OsString::into_encoded_bytes)
                .collect(),
            cfg_stdout: cfg.stdout,
        };
        for release in [false, true] {
            let mut args = vec!["build", "--offline", "--features=fast-path"];
            if release {
                args.push("--release");
            }
            let mut query = args.clone();
            query.extend(["--unit-graph", "-Z", "unstable-options"]);
            let graph: Value = serde_json::from_slice(&fixture.cargo(&query)?)?;
            let authority = derive(&graph, &metadata, std::slice::from_ref(&observed))
                .ok_or("unit authority absent")?;
            args.push("--message-format=json");
            let messages = fixture.cargo(&args)?;
            let output = std::str::from_utf8(&messages)?
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find(|message| message["reason"] == "build-script-executed")
                .ok_or("script output absent")?;
            let actual = std::fs::read_to_string(
                Path::new(
                    output["out_dir"]
                        .as_str()
                        .ok_or("script directory absent")?,
                )
                .join("environment.txt"),
            )?;
            let actual: BTreeMap<String, String> = actual
                .lines()
                .map(|line| {
                    let (key, value) = line.split_once('\t').ok_or("invalid script environment")?;
                    Ok((key.to_owned(), value.to_owned()))
                })
                .collect::<std::result::Result<_, &str>>()?;
            assert_eq!(
                serde_json::to_value(&actual)?,
                authority["packages"][id]["values"]
            );
            assert_eq!(
                actual.get("CARGO_FEATURE_FAST_PATH").map(String::as_str),
                Some("1")
            );
            assert!(!actual.contains_key("CARGO_FEATURE_UNUSED"));
            assert_eq!(actual.contains_key("CARGO_CFG_DEBUG_ASSERTIONS"), !release);
            assert!(actual.contains_key("CARGO_CFG_TARGET_ARCH"));
            assert!(actual.contains_key("CARGO_CFG_TARGET_OS"));
        }
        Ok(())
    }

    #[test]
    fn unknown_cargo_source_cannot_authorize_complete_unit_evidence() -> TestResult {
        let fixture = CargoFixture::new()?;
        std::fs::write(
            fixture.root.join("graph.json"),
            serde_json::to_vec(&graph(vec![unit(&[], Value::Null)]))?,
        )?;
        std::fs::write(
            fixture.root.join("cfg-1.json"),
            serde_json::to_vec(&observation(None, b"unix\n"))?,
        )?;
        let version = fixture.root.join("cargo-version.txt");
        std::fs::write(
            &version,
            "cargo 1.98.0 (unknown)\ncommit-hash: 0000000000000000000000000000000000000000\n",
        )?;
        assert!(load(&fixture.root, &metadata()).is_none());
        assert_eq!(unsupported_cargo(&fixture.root).as_deref(), Some("1.98.0"));
        std::fs::write(
            &version,
            "commit-hash: 5f94df4789f005f9a352888e8355ffc645b7ed0e\n",
        )?;
        let authority = load(&fixture.root, &metadata()).ok_or("known Cargo source refused")?;
        assert!(unsupported_cargo(&fixture.root).is_none());
        let current = json!({"sail_build_environment":authority});
        assert_eq!(get(&current, "CARGO_CFG_UNIX"), Some(Some(String::new())));
        assert_eq!(get(&current, "CARGO_FEATURE_ABSENT"), Some(None));
        Ok(())
    }

    fn cached_metadata(root: &Path) -> Value {
        json!({"workspace_root":root,"packages":[{"id":ID,
            "manifest_path":root.join("Cargo.toml"),"features":{},
            "targets":[{"name":"build-script-build","kind":["custom-build"],
                "src_path":root.join("build.rs")}]}],
            "sail_selected_package_ids":[ID]})
    }

    fn cached_graph(root: &Path) -> Value {
        let mut execution = unit(&[], Value::Null);
        execution["target"] = json!({"name":"build-script-build","kind":["custom-build"],
            "src_path":root.join("build.rs")});
        graph(vec![execution])
    }

    fn write_query(directory: &Path, graph: &Value, observation: &Observation) -> TestResult {
        std::fs::create_dir_all(directory)?;
        std::fs::write(directory.join("graph.json"), serde_json::to_vec(graph)?)?;
        std::fs::write(
            directory.join("cfg-7.json"),
            serde_json::to_vec(observation)?,
        )?;
        std::fs::write(
            directory.join("cargo-version.txt"),
            format!("{CARGO_COMMIT}\n"),
        )?;
        Ok(())
    }

    #[test]
    fn retained_query_restores_only_under_its_exact_key() -> TestResult {
        let fixture = CargoFixture::new()?;
        let root = fixture.root.clone();
        let metadata = cached_metadata(&root);
        let config = vec![root.join(".cargo/config.toml")];
        let key = |data: &Value| cache_key("context", data, &root, &root.join("target"), &config);
        let first = key(&metadata).ok_or("key refused")?;
        // Source contents are not part of the key; manifests and the lockfile are.
        std::fs::write(root.join("src/lib.rs"), "// edited")?;
        assert_eq!(key(&metadata).as_deref(), Some(first.as_str()));
        let mut with_sail = metadata.clone();
        with_sail["sail_build_environment"] = json!({"version":1});
        assert_eq!(key(&with_sail).as_deref(), Some(first.as_str()));
        assert_ne!(
            cache_key("other", &metadata, &root, &root.join("target"), &config),
            Some(first.clone())
        );
        let query = root.join("query");
        write_query(&query, &cached_graph(&root), &observation(None, b"unix\n"))?;
        let context = root.join("context");
        std::fs::create_dir_all(&context)?;
        retain(&context, &first, &query, &metadata);
        let restored = root.join("restored");
        std::fs::create_dir_all(&restored)?;
        assert!(!restore(&context, "other", &restored, &metadata));
        assert!(restore(&context, &first, &restored, &metadata));
        assert_eq!(load(&restored, &metadata), load(&query, &metadata));
        // A metadata target the unit graph does not name exactly refuses.
        let mut moved = metadata.clone();
        moved["packages"][0]["targets"][0]["src_path"] = json!(root.join("other.rs"));
        let again = root.join("again");
        std::fs::create_dir_all(&again)?;
        assert!(!restore(&context, &first, &again, &moved));
        std::fs::write(root.join("Cargo.lock"), "# changed")?;
        assert_ne!(key(&metadata), Some(first.clone()));
        std::fs::write(root.join("Cargo.lock"), "")?;
        let locked = key(&metadata).ok_or("key refused")?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[profile.dev]\ndebug-assertions=false\n",
        )?;
        assert_ne!(key(&metadata), Some(locked));
        std::fs::create_dir_all(root.join(".cargo"))?;
        std::fs::write(&config[0], "include = ['other.toml']\n")?;
        assert_eq!(key(&metadata), None);
        Ok(())
    }

    #[test]
    fn custom_target_specifications_and_incomplete_queries_are_not_retained() -> TestResult {
        let fixture = CargoFixture::new()?;
        let root = fixture.root.clone();
        let metadata = cached_metadata(&root);
        let context = root.join("context");
        std::fs::create_dir_all(&context)?;
        let query = root.join("custom");
        let mut execution = cached_graph(&root);
        execution["units"][0]["platform"] = json!("custom.json");
        write_query(
            &query,
            &execution,
            &observation(Some("custom.json"), b"unix\n"),
        )?;
        assert!(load(&query, &metadata).is_some());
        retain(&context, "key", &query, &metadata);
        assert!(!context.join(CACHE_FILE).exists());
        let incomplete = root.join("incomplete");
        write_query(
            &incomplete,
            &cached_graph(&root),
            &observation(None, b"unix\n"),
        )?;
        std::fs::remove_file(incomplete.join("cargo-version.txt"))?;
        retain(&context, "key", &incomplete, &metadata);
        assert!(!context.join(CACHE_FILE).exists());
        Ok(())
    }

    #[test]
    fn missing_or_malformed_cargo_observation_does_not_identify_an_old_toolchain() -> TestResult {
        let fixture = CargoFixture::new()?;
        assert!(unsupported_cargo(&fixture.root).is_none());
        for text in [
            "cargo 1.98.0\n",
            "cargo 1.98.0\ncommit-hash: unknown\n",
            "commit-hash: 0000000000000000000000000000000000000000\n",
        ] {
            std::fs::write(fixture.root.join("cargo-version.txt"), text)?;
            assert!(unsupported_cargo(&fixture.root).is_none());
        }
        Ok(())
    }
}

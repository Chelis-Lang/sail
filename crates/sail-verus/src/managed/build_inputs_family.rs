//! Transactional ordinary script observations for an exclusively owned target.
//!
//! This is not a proof receipt. A family export is consumed before any target
//! mutation; an interrupted writer cannot leave old declarations authorizing
//! newer generated output. Every context keeps its own fresh observation copy.

use super::{BuildInput, current};
use crate::managed::files::{digest, json};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::io::Read as _;
use std::io::Write;
use std::path::Path;

#[path = "build_inputs_sources.rs"]
mod sources;

const MAX_BYTES: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    version: u32,
    compiler: String,
    records: Vec<BuildInput>,
    sources: Option<sources::Inventory>,
    checksum: String,
}

struct Encoding(Vec<u8>);
impl Write for Encoding {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("compiler script observation limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize) -> Option<Vec<u8>> {
    let mut writer = Encoding(Vec::new());
    serde_json::to_writer(&mut writer, value).ok()?;
    Some(writer.0)
}
fn checksum(export: &Export) -> Option<String> {
    encode(&(
        export.version,
        &export.compiler,
        &export.records,
        &export.sources,
    ))
    .map(|v| digest(&v))
}

#[cfg(target_os = "linux")]
fn bytes(path: &Path) -> Option<Vec<u8>> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let file = std::fs::File::from(fd);
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(u64::try_from(MAX_BYTES).ok()?.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_BYTES {
        return None;
    }
    Some(bytes)
}
#[cfg(not(target_os = "linux"))]
fn bytes(_path: &Path) -> Option<Vec<u8>> {
    // Only the Linux nonblocking regular-file loader is audited. An unavailable
    // export always takes original script cleaning, including an older context.
    None
}

fn read(path: &Path) -> Option<Export> {
    serde_json::from_slice(&bytes(path)?).ok()
}

fn output_matches(record: &BuildInput, target: &Path) -> bool {
    let Some((path, sha)) = &record.output else {
        return false;
    };
    let Some(target) = target.canonicalize().ok() else {
        return false;
    };
    let Some(actual) = path.canonicalize().ok().filter(|p| p.starts_with(&target)) else {
        return false;
    };
    let Some(bytes) = bytes(&actual).filter(|bytes| digest(bytes) == *sha) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    let Some(directory) = record.manifest.parent() else {
        return false;
    };
    let mut paths = Vec::new();
    let mut keys = std::collections::BTreeSet::new();
    for line in text.lines() {
        let line = line
            .strip_prefix("cargo::")
            .or_else(|| line.strip_prefix("cargo:"))
            .unwrap_or(line);
        if let Some(path) = line.strip_prefix("rerun-if-changed=") {
            paths.push(directory.join(path));
        }
        if let Some(key) = line.strip_prefix("rerun-if-env-changed=") {
            keys.insert(key.to_owned());
        }
    }
    if paths.is_empty() && keys.is_empty() {
        paths.push(directory.to_owned());
    }
    paths == record.paths && keys == record.env.keys().cloned().collect()
}

fn matches(record: &BuildInput, data: &Value, target: &Path) -> bool {
    record.covered
        && output_matches(record, target)
        && record.env.keys().all(|key| allowed_environment(key))
        && package_matches(record, data)
        && current(record, None) == (record.digest.clone(), true)
}

// Parent values do not describe Cargo-injected or removed child variables.
// RUSTC_BOOTSTRAP is the sole pinned, explicitly invoked exception.
fn allowed_environment(key: &str) -> bool {
    if matches!(std::env::var(key), Err(std::env::VarError::NotUnicode(_))) {
        return false;
    }
    key == "RUSTC_BOOTSTRAP"
        || !(key.starts_with("SAIL_")
            || key.starts_with("VERUS_")
            || key.starts_with("CARGO_")
            || key.starts_with("RUSTUP_")
            || key.starts_with("DEP_")
            || [
                "CARGO",
                "RUSTC",
                "RUSTDOC",
                "RUSTC_LINKER",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "RUSTFLAGS",
                "OUT_DIR",
                "HOST",
                "TARGET",
                "PROFILE",
                "OPT_LEVEL",
                "DEBUG",
                "NUM_JOBS",
                "LD_LIBRARY_PATH",
                "DYLD_LIBRARY_PATH",
                "DYLD_FALLBACK_LIBRARY_PATH",
                "LIBPATH",
                "PATH",
            ]
            .contains(&key))
}

fn package_matches(record: &BuildInput, data: &Value) -> bool {
    data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|package| {
            package["id"] == record.package_id
                && package["name"] == record.package_name
                && package["manifest_path"].as_str().map(Path::new)
                    == Some(record.manifest.as_path())
                && package["targets"].as_array().is_some_and(|targets| {
                    targets.iter().any(|target| {
                        target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
                    })
                })
        })
}

fn eligible(records: Vec<BuildInput>, data: &Value, target: &Path) -> Vec<BuildInput> {
    let mut invalid = BTreeSet::new();
    let mut units = BTreeSet::new();
    for record in &records {
        let output = record.output.as_ref().map(|(path, _)| path.as_path());
        if !matches(record, data, target) || !units.insert((record.package_id.as_str(), output)) {
            invalid.insert(record.package_id.clone());
        }
    }
    // A single unchanged unit must not authorize other units of the package.
    records
        .into_iter()
        .filter(|r| !invalid.contains(&r.package_id))
        .collect()
}

/// Recheck after the original native guards start, before any cached output use.
/// Remove all observations of a package if even one unit lost its contract.
pub(in crate::managed) fn recheck(
    local: &Path,
    data: &Value,
    target: &Path,
) -> Result<Vec<String>> {
    let previous = super::read::<Vec<BuildInput>>(local).unwrap_or_default();
    let packages: std::collections::BTreeMap<_, _> = previous
        .iter()
        .map(|r| (r.package_id.clone(), r.package_name.clone()))
        .collect();
    let retained = eligible(previous, data, target);
    let retained_ids: BTreeSet<_> = retained.iter().map(|r| r.package_id.as_str()).collect();
    let clean: BTreeSet<_> = packages
        .iter()
        .filter(|(id, _)| !retained_ids.contains(id.as_str()))
        .map(|(_, name)| name.clone())
        .collect();
    let clean = clean.into_iter().collect();
    super::super::files::json(local, &retained)?;
    Ok(clean)
}

/// Must run under the existing family lock, before any clean or invocation.
/// Missing exports cannot fall back to an older context-local observation.
pub(in crate::managed) fn begin(
    path: &Path,
    compiler: &str,
    target: &Path,
    local: &Path,
    data: &Value,
) -> Result<bool> {
    let export = read(path).filter(|export| {
        export.version == 2
            && export.compiler == compiler
            && checksum(export).as_ref() == Some(&export.checksum)
    });
    // Remove even unreadable, stale or malformed exports. Refuse mutation if
    // removal fails: leaving an old certificate alongside new output is unsafe.
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::new("SAIL3102", error.to_string())),
    }
    let (mut records, available) = export.map_or_else(
        || (Vec::new(), false),
        |export| {
            let count = export.records.len();
            let mut records = eligible(export.records, data, target);
            let mut available = count != 0 && count == records.len();
            if let Some(sources) = export.sources {
                let source_state = local.with_file_name("source-inputs.json");
                let admitted = available
                    .then(|| sources.admit(data, &records, target))
                    .flatten()
                    .and_then(|inputs| sources::merge(&source_state, inputs));
                if let Some(inputs) = admitted {
                    // This is only an inventory. The caller subsequently reads
                    // every declared byte and acquires all original owners.
                    if super::super::files::json(&source_state, &inputs).is_err() {
                        available = false;
                    }
                } else {
                    available = false;
                }
                if !available {
                    records.clear();
                }
            }
            (records, available)
        },
    );
    let admitted: BTreeSet<_> = records.iter().map(|r| r.package_id.clone()).collect();
    // Preserve earlier declarations only as observation roots, never as reuse
    // authorization. This keeps the original declarations-establishment next
    // run's inventory while forcing regeneration of every uncertified package.
    for mut prior in super::read::<Vec<BuildInput>>(local).unwrap_or_default() {
        if !admitted.contains(&prior.package_id) && package_matches(&prior, data) {
            prior.covered = false;
            prior.output = None;
            records.push(prior);
        }
    }
    super::super::files::json(local, &records)?;
    Ok(available)
}

/// Optional publication after the original final Current transaction. The
/// runner admits either successful verification or its separately sealed full
/// negative outcome; neither this export nor a negative outcome is a proof.
/// Encoding/publication failure leaves no reusable family export.
pub(in crate::managed) fn publish(
    path: &Path,
    compiler: &str,
    target: &Path,
    local: &Path,
    data: &Value,
) {
    let records = eligible(
        super::read::<Vec<BuildInput>>(local).unwrap_or_default(),
        data,
        target,
    );
    let sources = sources::Inventory::capture(
        &local.with_file_name("source-inputs.json"),
        data,
        &records,
        target,
    );
    publish_inventory(path, compiler, records, sources);
}

fn publish_inventory(
    path: &Path,
    compiler: &str,
    records: Vec<BuildInput>,
    sources: Option<sources::Inventory>,
) {
    let mut export = Export {
        version: 2,
        compiler: compiler.to_owned(),
        records,
        sources,
        checksum: String::new(),
    };
    let Some(sum) = checksum(&export) else {
        return;
    };
    export.checksum = sum;
    // Check the complete deterministic envelope before the original atomic
    // writer allocates its identical document. This bounds the new encoding,
    // not existing Cargo loaders or overall managed invocation memory/RSS.
    if encode(&export).is_some() {
        let _ = json(path, &export);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use std::path::PathBuf;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    struct Fixture {
        root: PathBuf,
        target: PathBuf,
        input: PathBuf,
        local: PathBuf,
        export: PathBuf,
        data: Value,
    }
    impl Fixture {
        fn new(name: &str) -> std::io::Result<Self> {
            let root = std::env::temp_dir()
                .join(format!("sail-script-family-{name}-{}", std::process::id()));
            let target = root.join("target");
            std::fs::create_dir_all(&target)?;
            let input = root.join("external.txt");
            std::fs::write(&input, "good")?;
            let manifest = root.join("Cargo.toml");
            std::fs::write(&manifest, "[package]\nname='registry-script'\n")?;
            let data = serde_json::json!({"packages":[{
                "id":"registry-pkg", "name":"registry-script", "manifest_path":manifest,
                "source":"registry+https://example.invalid/index", "targets":[{"kind":["custom-build"]}]
            }]});
            Ok(Self {
                local: root.join("context-a.json"),
                export: root.join("family.json"),
                root,
                target,
                input,
                data,
            })
        }
        fn record(&self, unit: &str, input: &Path, keys: &[&str]) -> std::io::Result<BuildInput> {
            let output = self.target.join(unit).join("output");
            std::fs::create_dir_all(output.parent().unwrap_or(&self.target))?;
            let mut text = format!("cargo:rerun-if-changed={}\n", input.display());
            for key in keys {
                let _ = writeln!(text, "cargo:rerun-if-env-changed={key}");
            }
            std::fs::write(&output, &text)?;
            let mut record = BuildInput {
                package_id: "registry-pkg".to_owned(),
                package_name: "registry-script".to_owned(),
                manifest: self.root.join("Cargo.toml"),
                paths: vec![input.to_owned()],
                env: keys
                    .iter()
                    .map(|k| ((*k).to_owned(), super::super::effective_value(k, None)))
                    .collect(),
                covered: true,
                digest: String::new(),
                output: Some((output, digest(text.as_bytes()))),
            };
            record.digest = current(&record, None).0;
            Ok(record)
        }
        fn export(&self, records: &[BuildInput]) -> Result<()> {
            json(&self.local, &records)?;
            publish(
                &self.export,
                "compiler",
                &self.target,
                &self.local,
                &self.data,
            );
            Ok(())
        }
        fn begin(&self, local: &Path, compiler: &str) -> Result<()> {
            begin(&self.export, compiler, &self.target, local, &self.data).map(|_| ())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn selector_reuses_script_contract_but_consumes_generation_before_any_mutation() -> TestResult {
        let f = Fixture::new("selector")?;
        f.export(&[f.record("host", &f.input, &[])?])?;
        let other = f.root.join("context-b.json");
        f.begin(&other, "compiler")?;
        assert!(!f.export.exists());
        assert_eq!(super::super::stale(&other, &f.data).0, Vec::<String>::new());
        // B rewrites shared output then fails. Even restoring A's external input
        // and mtime cannot revive the consumed generation or old local A ledger.
        let file = std::fs::File::open(&f.input)?;
        let times = file.metadata()?.modified()?;
        std::fs::write(&f.input, "fail")?;
        std::fs::write(f.target.join("host/output"), "cargo:rustc-cfg=wrong\n")?;
        std::fs::write(&f.input, "good")?;
        file.set_times(std::fs::FileTimes::new().set_modified(times))?;
        f.begin(&f.local, "compiler")?;
        let (clean, _, complete) = super::super::stale(&f.local, &f.data);
        assert_eq!(clean, vec!["registry-script"]);
        assert!(!complete);
        Ok(())
    }
    #[test]
    fn post_watch_recheck_cleans_restored_mtime_change_before_cached_output_use() -> TestResult {
        let f = Fixture::new("post-watch")?;
        f.export(&[f.record("host", &f.input, &[])?])?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );
        let file = std::fs::File::open(&f.input)?;
        let mtime = file.metadata()?.modified()?;
        std::fs::write(&f.input, "fail")?;
        file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        // This is the precise acquisition gap: the original owner starts after
        // mutation and legitimately observes the new bytes as pristine.
        let mut observer =
            crate::managed::precise::Observer::begin(std::slice::from_ref(&f.input), &[])
                .ok_or("actual native observer unavailable")?;
        assert!(observer.clean());
        assert_eq!(
            recheck(&f.local, &f.data, &f.target)?,
            vec!["registry-script"]
        );
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        Ok(())
    }
    #[test]
    fn both_output_units_survive_update_and_an_earlier_input_change_refuses_package() -> TestResult
    {
        let f = Fixture::new("units")?;
        let second = f.root.join("target-input.txt");
        std::fs::write(&second, "also")?;
        let records = [
            f.record("host", &f.input, &[])?,
            f.record("target", &second, &[])?,
        ];
        let artifacts = f.root.join("artifacts.jsonl");
        let mut text = String::new();
        for record in &records {
            let output = record.output.as_ref().map_or(&f.target, |(path, _)| path);
            writeln!(
                text,
                "{}",
                serde_json::json!({"reason":"build-script-executed", "package_id":record.package_id,
                "out_dir":output.parent().unwrap_or(&f.target).join("out")})
            )?;
        }
        std::fs::write(&artifacts, text)?;
        super::super::update(&f.local, &artifacts, &f.data)?;
        let updated: Vec<BuildInput> = super::super::read(&f.local).ok_or("unit ledger missing")?;
        assert_eq!(updated.len(), 2);
        publish(&f.export, "compiler", &f.target, &f.local, &f.data);
        f.begin(&f.local, "compiler")?;
        let file = std::fs::File::open(&f.input)?;
        let mtime = file.metadata()?.modified()?;
        std::fs::write(&f.input, "fail")?;
        file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        assert_eq!(
            recheck(&f.local, &f.data, &f.target)?,
            vec!["registry-script"]
        );
        assert!(super::super::read::<Vec<BuildInput>>(&f.local).is_some_and(|v| v.is_empty()));
        Ok(())
    }
    #[test]
    fn content_contract_output_registry_and_compiler_changes_cannot_import() -> TestResult {
        let f = Fixture::new("bindings")?;
        let pristine = f.record("host", &f.input, &[])?;
        for field in [
            "input",
            "declarations",
            "output",
            "registry",
            "compiler",
            "incomplete",
            "duplicate",
            "old-version",
            "checksum",
        ] {
            std::fs::write(&f.input, "good")?;
            let record = f.record("host", &f.input, &[])?;
            let records = if field == "duplicate" {
                vec![record.clone(), record]
            } else {
                vec![record]
            };
            f.export(&records)?;
            let mut data = f.data.clone();
            match field {
                "input" => std::fs::write(&f.input, "fail")?,
                "declarations" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.records[0].paths.clear();
                    export.checksum = checksum(&export).ok_or("checksum failed")?;
                    json(&f.export, &export)?;
                }
                "output" => {
                    std::fs::write(f.target.join("host/output"), "cargo:rustc-cfg=changed\n")?;
                }
                "registry" => {
                    data["packages"][0]["id"] = Value::String("different-registry-pkg".to_owned());
                }
                "incomplete" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.records[0].covered = false;
                    export.checksum = checksum(&export).ok_or("checksum failed")?;
                    json(&f.export, &export)?;
                }
                "old-version" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.version = 1;
                    export.checksum = checksum(&export).ok_or("checksum failed")?;
                    json(&f.export, &export)?;
                }
                "checksum" => {
                    let mut export = read(&f.export).ok_or("export missing")?;
                    export.checksum = "invalid".to_owned();
                    json(&f.export, &export)?;
                }
                _ => {}
            }
            begin(
                &f.export,
                if field == "compiler" {
                    "different-tool-family"
                } else {
                    "compiler"
                },
                &f.target,
                &f.local,
                &data,
            )?;
            assert!(
                super::super::read::<Vec<BuildInput>>(&f.local)
                    .is_some_and(|v| v.iter().all(|r| !r.covered)),
                "refusal: {field}"
            );
        }
        assert!(pristine.covered);
        Ok(())
    }
    #[test]
    fn effective_bootstrap_is_bound_and_unaudited_injected_environment_refuses_reuse() -> TestResult
    {
        let f = Fixture::new("environment")?;
        let record = f.record("host", &f.input, &["RUSTC_BOOTSTRAP"])?;
        let parent =
            BTreeMap::from([("RUSTC_BOOTSTRAP".to_owned(), "different-parent".to_owned())]);
        assert_eq!(current(&record, Some(&parent)), current(&record, None));
        assert_eq!(record.env["RUSTC_BOOTSTRAP"].as_deref(), Some("1"));
        let ordinary = f.record("env", &f.input, &["LANG"])?;
        let changed =
            BTreeMap::from([("LANG".to_owned(), "sail-changed-language-input".to_owned())]);
        assert_ne!(current(&ordinary, Some(&changed)).0, ordinary.digest);
        f.export(&[record])?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );
        for key in [
            "OUT_DIR",
            "HOST",
            "TARGET",
            "PROFILE",
            "OPT_LEVEL",
            "DEBUG",
            "NUM_JOBS",
            "SAIL_MANAGED_RUNTIME",
            "CARGO",
            "CARGO_FEATURE_TEST",
            "VERUS_DRIVER_ARGS",
            "RUSTUP_TOOLCHAIN",
        ] {
            f.export(&[f.record("host", &f.input, &[key])?])?;
            f.begin(&f.local, "compiler")?;
            assert_eq!(
                super::super::stale(&f.local, &f.data).0,
                vec!["registry-script"],
                "declared {key}"
            );
        }
        let mut record = f.record("host", &f.input, &["SAIL_TEST_FAMILY_INPUT"])?;
        record.env.insert(
            "SAIL_TEST_FAMILY_INPUT".to_owned(),
            Some("stale-value".to_owned()),
        );
        assert!(!matches(&record, &f.data, &f.target));
        Ok(())
    }
    #[test]
    fn malformed_oversize_and_nonregular_exports_discard_older_local_observations() -> TestResult {
        let f = Fixture::new("invalid")?;
        for text in [b"{".to_vec(), vec![b'x'; MAX_BYTES + 1]] {
            json(&f.local, &vec![f.record("host", &f.input, &[])?])?;
            std::fs::write(&f.export, text)?;
            f.begin(&f.local, "compiler")?;
            assert_eq!(
                super::super::stale(&f.local, &f.data).0,
                vec!["registry-script"]
            );
        }
        std::os::unix::fs::symlink(&f.local, &f.export)?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &f.export,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )?;
        f.begin(&f.local, "compiler")?;
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            vec!["registry-script"]
        );
        Ok(())
    }
    #[test]
    fn distinct_package_ids_with_one_name_cannot_hide_post_watch_invalidation() -> TestResult {
        let f = Fixture::new("same-name")?;
        let mut data = f.data.clone();
        let mut package = data["packages"][0].clone();
        package["id"] = Value::String("registry-pkg-other-version".to_owned());
        data["packages"]
            .as_array_mut()
            .ok_or("packages absent")?
            .push(package);
        let other_input = f.root.join("other.txt");
        std::fs::write(&other_input, "also")?;
        let mut second = f.record("other-version", &other_input, &[])?;
        second.package_id = "registry-pkg-other-version".to_owned();
        json(&f.local, &vec![f.record("host", &f.input, &[])?, second])?;
        std::fs::write(&f.input, "fail")?;
        assert_eq!(
            recheck(&f.local, &data, &f.target)?,
            vec!["registry-script"]
        );
        let retained: Vec<BuildInput> = super::super::read(&f.local).ok_or("local absent")?;
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].package_id, "registry-pkg-other-version");
        Ok(())
    }
    #[test]
    fn missing_generation_keeps_declaration_watch_roots_but_forces_fresh_script() -> TestResult {
        let f = Fixture::new("declaration-transition")?;
        let record = f.record("host", &f.input, &[])?;
        json(&f.local, &vec![record])?;
        f.begin(&f.local, "compiler")?;
        let (clean, paths, covered) = super::super::stale(&f.local, &f.data);
        assert_eq!(clean, vec!["registry-script"]);
        assert_eq!(paths, vec![f.input.clone()]);
        assert!(!covered);
        let observation: Vec<BuildInput> =
            super::super::read(&f.local).ok_or("watch paths missing")?;
        assert!(observation.iter().all(|r| !r.covered && r.output.is_none()));
        assert_eq!(
            recheck(&f.local, &f.data, &f.target)?,
            vec!["registry-script"]
        );
        let fresh = f.record("host", &f.input, &[])?;
        let output = fresh
            .output
            .as_ref()
            .map(|(path, _)| path)
            .ok_or("output absent")?;
        let artifacts = f.root.join("artifacts.jsonl");
        std::fs::write(
            &artifacts,
            format!(
                "{}\n",
                serde_json::json!({
                    "reason":"build-script-executed", "package_id":fresh.package_id,
                    "out_dir":output.parent().ok_or("parent absent")?.join("out")
                })
            ),
        )?;
        super::super::update(&f.local, &artifacts, &f.data)?;
        let established: Vec<BuildInput> =
            super::super::read(&f.local).ok_or("fresh record absent")?;
        assert_eq!(established.len(), 1);
        assert!(established[0].covered && established[0].output.is_some());
        assert_eq!(
            super::super::stale(&f.local, &f.data).0,
            Vec::<String>::new()
        );
        publish(&f.export, "compiler", &f.target, &f.local, &f.data);
        assert!(f.export.is_file());
        f.begin(&f.local, "compiler")?;
        let (clean, paths, covered) = super::super::stale(&f.local, &f.data);
        assert_eq!(clean, Vec::<String>::new());
        assert_eq!(paths, vec![f.input.clone()]);
        assert!(covered);
        Ok(())
    }
    #[test]
    fn removed_dependency_does_not_force_cleaning_a_retired_package() -> TestResult {
        let f = Fixture::new("removed")?;
        json(&f.local, &vec![f.record("host", &f.input, &[])?])?;
        let data = serde_json::json!({"packages":[]});
        begin(&f.export, "compiler", &f.target, &f.local, &data)?;
        let (clean, paths, complete) = super::super::stale(&f.local, &data);
        assert!(clean.is_empty() && paths.is_empty() && complete);
        Ok(())
    }
}

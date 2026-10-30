//! A negative hint for a previously observed unqualified Cargo (#561).
//!
//! This can only choose the existing plain fallback. It establishes no proof,
//! source currentness, replay or build-script authority. Unknown selections,
//! custom wrappers, changed inputs and malformed hints keep the late check.

use super::{Config, input, receipts, tool_digests};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    key: String,
    reported: String,
    selection: String,
    programs: Vec<(PathBuf, String)>,
    metadata: BTreeSet<PathBuf>,
    metadata_stamp: String,
    checksum: String,
}

impl Record {
    fn body_digest(&self) -> Option<String> {
        let bytes = serde_json::to_vec(&(
            self.version,
            &self.key,
            &self.reported,
            &self.selection,
            &self.programs,
            &self.metadata,
            &self.metadata_stamp,
        ))
        .ok()?;
        Some(super::files::digest(&bytes))
    }
}

/// Selection observed before the genuine Cargo version query, retained only
/// if it still agrees afterwards. The hint is private scheduling data.
pub(super) struct Probe {
    file: PathBuf,
    key: String,
    paths: BTreeSet<PathBuf>,
    programs: Vec<PathBuf>,
    selection: String,
}

impl Probe {
    pub(super) fn begin(config: &Config, environment: &BTreeMap<Vec<u8>, Vec<u8>>) -> Option<Self> {
        if std::env::var_os("SAIL_MANAGED_UNQUALIFIED_CARGO_MEMO").is_some_and(|value| value == "0")
        {
            return None;
        }
        let bytes = serde_json::to_vec(&(
            "sail-unqualified-cargo",
            VERSION,
            &config.workspace,
            &config.args,
            &config.cache,
            &config.sail,
            &config.cargo_verus,
            &config.cargo,
            format!("{:?}", config.cargo_toolchain),
            environment.iter().collect::<Vec<_>>(),
        ))
        .ok()?;
        let key = super::files::digest(&bytes);
        let (paths, programs) = selection_paths(config)?;
        let selection = input::stamp_paths(&paths)?;
        // Even a script installed at a normal rustup path can delegate its
        // version to unobserved files. Keep opaque/interpreter wrappers late.
        programs
            .iter()
            .all(|path| native_program(path))
            .then_some(())?;
        (input::stamp_paths(&paths).as_deref() == Some(selection.as_str())).then_some(())?;
        Some(Self {
            file: config
                .cache
                .join("unqualified-cargo")
                .join(format!("{key}.json")),
            key,
            paths,
            programs,
            selection,
        })
    }

    pub(super) fn refusal(&self, config: &Config) -> Option<Error> {
        let record: Record = receipts::read(&self.file)?;
        (record.body_digest().as_deref() == Some(record.checksum.as_str())).then_some(())?;
        (record.version == VERSION && record.key == self.key && record.selection == self.selection)
            .then_some(())?;
        let version = super::build_environment::unsupported_version(&record.reported)?;
        let paths: Vec<_> = record
            .programs
            .iter()
            .map(|(path, _)| path.clone())
            .collect();
        (paths == self.programs).then_some(())?;
        (input::stamp_paths(&record.metadata).as_deref() == Some(record.metadata_stamp.as_str()))
            .then_some(())?;
        let tools = tool_digests::ToolDigests::open(&config.cache);
        record
            .programs
            .iter()
            .all(|(path, digest)| tools.matches(path, digest))
            .then_some(())?;
        tools.save();
        error(&version).ok()
    }

    pub(super) fn remember(&self, config: &Config, data: &serde_json::Value, query: &Path) {
        let remember = || -> Option<()> {
            let reported = std::fs::read_to_string(query.join("cargo-version.txt")).ok()?;
            super::build_environment::unsupported_version(&reported)?;
            (input::stamp_paths(&self.paths).as_deref() == Some(self.selection.as_str()))
                .then_some(())?;
            let guard = input::MetadataGuard::begin(data)?;
            let (metadata, metadata_stamp) = guard.parts();
            let tools = tool_digests::ToolDigests::open(&config.cache);
            let programs = self
                .programs
                .iter()
                .map(|path| Some((path.clone(), tools.identity(path).ok()?)))
                .collect::<Option<Vec<_>>>()?;
            tools.save();
            // Identification itself may race an install or toolchain switch.
            (input::stamp_paths(&self.paths).as_deref() == Some(self.selection.as_str()))
                .then_some(())?;
            guard.unchanged_for(data).then_some(())?;
            std::fs::create_dir_all(self.file.parent()?).ok()?;
            let mut record = Record {
                version: VERSION,
                key: self.key.clone(),
                reported,
                selection: self.selection.clone(),
                programs,
                metadata: metadata.clone(),
                metadata_stamp: metadata_stamp.to_owned(),
                checksum: String::new(),
            };
            record.checksum = record.body_digest()?;
            super::files::json(&self.file, &record).ok()?;
            Some(())
        };
        let _ = remember();
    }
}

/// Same code and message for an early hint and the original late refusal.
pub(super) fn error(version: &str) -> Result<Error> {
    let toolchain = crate::toolchain::Pins::embedded()?.verus.rust_toolchain;
    Ok(Error::new(
        "SAIL3139",
        format!(
            "Cargo {version} cannot track this project's build-script inputs for proof reuse; \
         its build-script protocol is qualified for Cargo {}. Select a qualified Rust \
         toolchain, for example `RUSTUP_TOOLCHAIN={toolchain} sail verify`, to enable reuse",
            super::build_environment::QUALIFIED_RANGE
        ),
    ))
}

fn named_channel(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn native_program(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0; 4];
    if std::io::Read::read_exact(&mut file, &mut magic).is_err() {
        return false;
    }
    // ELF and Mach-O (thin and universal), on the supported native hosts.
    // The genuine Cargo report and complete program digests remain required.
    matches!(
        magic,
        [0x7f, b'E', b'L', b'F']
            | [0xfe, 0xed, 0xfa, 0xce | 0xcf]
            | [0xce | 0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe | 0xbf]
            | [0xbe | 0xbf, 0xba, 0xfe, 0xca]
    )
}

/// Bind the actual Cargo bodies as well as the rustup proxy and every file
/// which can select one. External path toolchains and custom Cargo wrappers
/// deliberately retain the ordinary late check.
fn selection_paths(config: &Config) -> Option<(BTreeSet<PathBuf>, Vec<PathBuf>)> {
    config.cargo.is_absolute().then_some(())?;
    let root = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")))?;
    root.is_absolute().then_some(())?;
    if let Some(channel) = std::env::var_os(super::cargo_toolchain::ENV) {
        named_channel(channel.to_str()?).then_some(())?;
    }
    let settings = root.join("settings.toml");
    let table: toml::Table = std::fs::read_to_string(&settings).ok()?.parse().ok()?;
    if let Some(default) = table.get("default_toolchain") {
        named_channel(default.as_str()?).then_some(())?;
    }
    if let Some(overrides) = table.get("overrides") {
        for value in overrides.as_table()?.values() {
            named_channel(value.as_str()?).then_some(())?;
        }
    }
    let mut paths = BTreeSet::from([settings, root.join("toolchains"), config.cargo.clone()]);
    let mut programs = BTreeSet::new();
    for entry in std::fs::read_dir(root.join("toolchains")).ok()? {
        let cargo = entry.ok()?.path().join("bin/cargo");
        if cargo.is_file() {
            paths.insert(cargo.clone());
            programs.insert(cargo);
        }
    }
    let actual = config.cargo.canonicalize().ok()?;
    let native = programs
        .iter()
        .any(|path| path.canonicalize().ok().as_ref() == Some(&actual));
    let proxy = super::cargo_toolchain::observe(&config.workspace, &config.cargo, "").rustup_proxy;
    (native || proxy).then_some(())?;
    programs.insert(config.cargo.clone());
    for program in [&config.sail, &config.cargo_verus] {
        paths.insert(program.clone());
        programs.insert(program.clone());
    }
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    for path in input::configuration_paths(&config.workspace, home.as_deref()) {
        if path
            .file_name()
            .is_some_and(|name| name == "rust-toolchain" || name == "rust-toolchain.toml")
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    if let Ok(table) = text.parse::<toml::Table>() {
                        let toolchain = table.get("toolchain")?.as_table()?;
                        (!toolchain.contains_key("path")).then_some(())?;
                        named_channel(toolchain.get("channel")?.as_str()?).then_some(())?;
                    } else {
                        named_channel(text.trim()).then_some(())?;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return None,
            }
        }
        paths.insert(path);
    }
    Some((paths, programs.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn process_probe() -> TestResult {
        let Some(root) = std::env::var_os("SAIL_TEST_UNQUALIFIED_ROOT") else {
            return Ok(());
        };
        let root = PathBuf::from(root);
        let operation = std::fs::read_to_string(root.join("operation"))?;
        let cargo = if operation.starts_with("proxy-") {
            root.join("bin/cargo")
        } else if operation == "wrapper" {
            root.join("custom-cargo")
        } else {
            root.join("rustup/toolchains/1.92.0/bin/cargo")
        };
        let config = Config {
            workspace: root.join("project"),
            cache: root.join("cache"),
            sail: root.join("sail"),
            cargo_verus: root.join("cargo-verus"),
            cargo,
            cargo_toolchain: super::super::CargoToolchain::Caller,
            shim: root.join("sail-z3"),
            z3: root.join("z3"),
            socket: None,
            args: Vec::new(),
            force: false,
            attribution_logs: false,
        };
        let environment = BTreeMap::from([
            (b"RUSTUP_TOOLCHAIN".to_vec(), b"1.92.0".to_vec()),
            (b"RUSTFLAGS".to_vec(), vec![0xff, b'x']),
        ]);
        let probe = Probe::begin(&config, &environment);
        if matches!(
            operation.as_str(),
            "wrapper" | "disabled" | "external" | "installed-wrapper" | "broker-wrapper"
        ) {
            assert!(probe.is_none());
            return Ok(());
        }
        let probe = probe.ok_or("fixture selection refused")?;
        if operation.ends_with("prepare") {
            let data = serde_json::json!({
                "workspace_root":config.workspace,"sail_invocation_directory":config.workspace,
                "packages":[{"id":"app","manifest_path":config.workspace.join("Cargo.toml")}]
            });
            probe.remember(&config, &data, &root.join("query"));
            assert!(probe.file.is_file());
        } else if operation == "qualified" {
            let mut record: Record = receipts::read(&probe.file).ok_or("record unavailable")?;
            record.reported = format!(
                "cargo 1.99.1\ncommit-hash: {}\n",
                super::super::build_environment::QUALIFIED_CARGO[0].1
            );
            record.checksum = record.body_digest().ok_or("checksum")?;
            super::super::files::json(&probe.file, &record)?;
            assert!(
                probe.refusal(&config).is_none(),
                "a qualified report was refused"
            );
        } else if operation == "corrupt" {
            let mut record: Record = receipts::read(&probe.file).ok_or("record unavailable")?;
            record.reported.push('x');
            super::super::files::json(&probe.file, &record)?;
            assert!(probe.refusal(&config).is_none());
        } else {
            let refusal = probe.refusal(&config);
            if operation.ends_with("same") {
                assert_eq!(refusal.map(|error| error.code), Some("SAIL3139"));
            } else {
                assert!(
                    refusal.is_none(),
                    "changed Cargo/metadata/selection reused a hint"
                );
            }
        }
        Ok(())
    }

    fn fixture_programs(root: &Path) -> TestResult {
        for name in [
            "project",
            "query",
            "bin",
            "cargo-home",
            "rustup/toolchains/1.92.0/bin",
            "rustup/toolchains/1.99.1/bin",
        ] {
            std::fs::create_dir_all(root.join(name))?;
        }
        let program = root.join("rustup/toolchains/1.92.0/bin/cargo");
        std::fs::write(&program, "\x7fELFold Cargo")?;
        std::fs::write(
            root.join("rustup/toolchains/1.99.1/bin/cargo"),
            "\x7fELFqualified Cargo",
        )?;
        std::fs::write(root.join("bin/rustup"), "\x7fELFproxy")?;
        std::fs::hard_link(root.join("bin/rustup"), root.join("bin/cargo"))?;
        std::fs::write(root.join("custom-cargo"), "opaque wrapper")?;
        std::fs::write(root.join("sail"), "\x7fELFsail")?;
        std::fs::write(root.join("cargo-verus"), "\x7fELFcargo-verus")?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn native_and_proxy_hints_bind_programs_metadata_and_selection() -> TestResult {
        let root = std::env::temp_dir().join(format!("sail-negative-cargo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        fixture_programs(&root)?;
        let program = root.join("rustup/toolchains/1.92.0/bin/cargo");
        let settings = root.join("rustup/settings.toml");
        let setting = |channel: &str| {
            std::fs::write(
                &settings,
                format!("version='12'\ndefault_toolchain='{channel}'\n[overrides]\n"),
            )
        };
        setting("1.92.0")?;
        let manifest = root.join("project/Cargo.toml");
        std::fs::write(&manifest, "[package]\nname='app'\nversion='0.1.0'\n")?;
        // Real Anvil's toolchain file begins with a generated-file comment.
        std::fs::write(
            root.join("project/rust-toolchain.toml"),
            "# AUTO-GENERATED by tools/setup-verus.sh\n[toolchain]\nchannel='1.92.0'\n",
        )?;
        std::fs::write(
            root.join("query/cargo-version.txt"),
            format!("cargo 1.92.0\ncommit-hash: {}\n", "0".repeat(40)),
        )?;
        let probe = |operation: &str| -> TestResult {
            std::fs::write(root.join("operation"), operation)?;
            let output = std::process::Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "managed::unqualified_cargo::tests::process_probe",
                    "--nocapture",
                ])
                .env("SAIL_TEST_UNQUALIFIED_ROOT", &root)
                .env("RUSTUP_HOME", root.join("rustup"))
                .env("CARGO_HOME", root.join("cargo-home"))
                .env("RUSTUP_TOOLCHAIN", "1.92.0")
                .env(
                    "SAIL_MANAGED_UNQUALIFIED_CARGO_MEMO",
                    if operation == "disabled" { "0" } else { "1" },
                )
                .env(
                    "PATH",
                    std::env::join_paths(std::iter::once(root.join("bin")).chain(
                        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
                    ))?,
                )
                .output()?;
            assert!(
                output.status.success(),
                "{operation}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(())
        };
        for operation in ["prepare", "same", "proxy-prepare", "proxy-same"] {
            probe(operation)?;
        }
        let modified = std::fs::metadata(&program)?.modified()?;
        std::fs::write(&program, "\x7fELFnew Cargo")?;
        std::fs::File::options()
            .write(true)
            .open(&program)?
            .set_modified(modified)?;
        probe("reinstalled")?;
        probe("proxy-reinstalled")?;
        probe("prepare")?;
        std::fs::write(&manifest, "[package]\nname='changed'\nversion='0.1.0'\n")?;
        probe("manifest")?;
        probe("prepare")?;
        setting("1.99.1")?;
        probe("selection")?;
        probe("prepare")?;
        probe("qualified")?;
        probe("prepare")?;
        probe("corrupt")?;
        probe("wrapper")?;
        probe("disabled")?;
        setting("/external/toolchain")?;
        probe("external")?;
        setting("1.92.0")?;
        std::fs::write(&program, "#!/bin/sh\nexec other-cargo \"$@\"\n")?;
        probe("installed-wrapper")?;
        std::fs::write(&program, "\x7fELFnew Cargo")?;
        std::fs::write(
            root.join("cargo-verus"),
            "#!/bin/sh\nexec other-broker \"$@\"\n",
        )?;
        probe("broker-wrapper")?;
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}

//! Driver-boundary reuse of verified registry metadata (ADR 0045).
//!
//! Native dependencies always run through Cargo and its real compiler. Only
//! a successful metadata-only verifier invocation may be recorded or replayed.
//! Its exact extern bytes and complete source/compiler observations bracket
//! both publication and placement; uncertain coverage is an ordinary miss.

use super::dependency_inventory::{Inventory, Observation, Root};
use super::files::{digest, file_digest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const VERSION: u32 = 2;
const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;

/// The source closure of one eligible registry proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Package {
    manifest: PathBuf,
    key: String,
    sources: Vec<PathBuf>,
    extern_names: Vec<String>,
}

/// Private per-run plan. No native artifact is placed by this plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Plan {
    directory: PathBuf,
    target: PathBuf,
    compiler: Vec<(PathBuf, bool)>,
    packages: Vec<Package>,
}

/// Build a plan only for fully identified dependency closures.
pub(super) fn plan(
    cache: &Path,
    target: &Path,
    candidates: &[super::store::Candidate],
    data: &Value,
    compiler: &[(PathBuf, bool)],
) -> Option<Plan> {
    let packages = data["packages"].as_array()?;
    let mut eligible = Vec::new();
    for candidate in candidates {
        let package = packages
            .iter()
            .find(|p| p["id"].as_str() == Some(&candidate.root))?;
        let manifest = PathBuf::from(package["manifest_path"].as_str()?);
        let node = data["resolve"]["nodes"]
            .as_array()?
            .iter()
            .find(|n| n["id"] == package["id"])?;
        let mut extern_names = Vec::new();
        for dependency in node["deps"].as_array()? {
            let id = dependency["pkg"].as_str()?;
            let member = packages.iter().find(|p| p["id"].as_str() == Some(id))?;
            let proc_macro = member["targets"].as_array()?.iter().any(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|k| k == "proc-macro"))
            });
            // A transitive metadata lookup through -L needs a separate closure
            // inventory contract. Proc macros are actually rebuilt and link
            // their native closure into the exact dylib we bind below.
            if !proc_macro
                && !data["resolve"]["nodes"]
                    .as_array()?
                    .iter()
                    .find(|n| n["id"].as_str() == Some(id))?["deps"]
                    .as_array()?
                    .is_empty()
            {
                return None;
            }
            extern_names.push(dependency["name"].as_str()?.replace('-', "_"));
        }
        extern_names.sort();
        let mut sources = Vec::new();
        for id in &candidate.closure {
            let member = packages.iter().find(|p| p["id"].as_str() == Some(id))?;
            // Local input aliases and git checkouts need a separately qualified
            // inventory contract. Registry trees have one complete package root.
            if !member["source"].as_str()?.starts_with("registry+") {
                return None;
            }
            sources.push(
                Path::new(member["manifest_path"].as_str()?)
                    .parent()?
                    .to_owned(),
            );
        }
        sources.sort();
        eligible.push(Package {
            manifest,
            key: candidate.key.clone(),
            sources,
            extern_names,
        });
    }
    (!eligible.is_empty() && !compiler.is_empty()).then(|| Plan {
        directory: cache.join("dependency-store/v2"),
        target: target.to_owned(),
        compiler: compiler.to_vec(),
        packages: eligible,
    })
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    key: String,
    origin: String,
    report: sail_api::output::Report,
    inventory: Inventory,
    declared: BTreeMap<String, Option<String>>,
    files: Vec<File>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    streams_sha256: String,
    checksum: String,
}

#[derive(Serialize, Deserialize)]
struct File {
    name: String,
    sha256: String,
    bytes: Vec<u8>,
}

/// A proof's actual establishing run, retained across local receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Origin {
    /// Original successful verification run, not this materialization run.
    pub run: String,
    /// Exact driver entry used for this replay.
    pub key: String,
}

/// The result of an exact metadata replay.
pub(crate) struct Replay {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub origin: Origin,
    pub report: sail_api::output::Report,
}

/// Inputs captured before the actual compiler starts or any entry is served.
pub(crate) struct Call<'a> {
    plan: &'a Plan,
    key: String,
    observation: Observation,
    environment: BTreeMap<String, String>,
    out_dir: PathBuf,
    stem: String,
}

fn value(args: &[String], flag: &str) -> Option<String> {
    let mut found = None;
    for (index, argument) in args.iter().enumerate() {
        let value = if argument == flag {
            Some(args.get(index + 1)?.clone())
        } else {
            argument
                .strip_prefix(&format!("{flag}="))
                .map(str::to_owned)
        };
        if let Some(value) = value
            && found.replace(value).is_some()
        {
            return None;
        }
    }
    found
}

fn extra_filename(args: &[String]) -> Option<String> {
    let mut found = None;
    for (index, argument) in args.iter().enumerate() {
        let option = if argument == "-C" {
            args.get(index + 1)?.as_str()
        } else {
            argument.strip_prefix("-C").unwrap_or_default()
        };
        let option = option.strip_prefix('=').unwrap_or(option);
        if let Some(value) = option.strip_prefix("extra-filename=")
            && found.replace(value.to_owned()).is_some()
        {
            return None;
        }
    }
    found
}

fn metadata_outputs(args: &[String]) -> Option<(String, PathBuf, String)> {
    if !matches!(value(args, "--crate-type")?.as_str(), "lib" | "rlib") {
        return None;
    }
    let emit = value(args, "--emit")?;
    let emits: Vec<_> = emit.split(',').collect();
    if !emits.contains(&"metadata")
        || !emits.contains(&"dep-info")
        || emits.iter().any(|e| !matches!(*e, "metadata" | "dep-info"))
    {
        return None;
    }
    let out_dir = PathBuf::from(value(args, "--out-dir")?);
    let crate_name = value(args, "--crate-name")?;
    let stem = format!("{crate_name}{}", extra_filename(args)?);
    Some((crate_name, out_dir, stem))
}

fn environment(
    vars: impl Iterator<Item = (OsString, OsString)>,
) -> Option<BTreeMap<String, String>> {
    // Declared reads may name any variable, including Sail/Cargo routing
    // variables omitted from the key. Never discard their presence or value.
    vars.map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

impl<'a> Call<'a> {
    /// An invocation capable of producing only Rust metadata and proof data.
    #[allow(
        clippy::too_many_lines,
        reason = "The single fail-closed constructor assembles one exact compiler invocation and its complete pre-compilation observation."
    )]
    pub(crate) fn new(plan: &'a Plan, args: &[OsString]) -> Option<Self> {
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_str().map(str::to_owned))
            .collect::<Option<_>>()?;
        // Response files and compiler plugins introduce read roots outside
        // this initial contract; their contents cannot be replaced by a path key.
        if args.iter().any(|arg| {
            arg.starts_with('@')
                || arg.starts_with("codegen-backend=")
                || arg.starts_with("-Zcodegen-backend=")
                || arg.starts_with("profile-use=")
                || arg.starts_with("-Cprofile-use=")
                || arg.starts_with("--import")
                || arg.starts_with("import-dep-if-present")
                || arg.starts_with("llvm-args=")
                || arg.starts_with("-Cllvm-args=")
        }) {
            return None;
        }
        let (crate_name, out_dir, stem) = metadata_outputs(&args)?;
        let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?).join("Cargo.toml");
        let package = plan.packages.iter().find(|p| p.manifest == manifest)?;
        // Cargo executes registry invocations from that package's source root.
        // Bind the actual cwd too: relative paths and flags are never rebased.
        let cwd = std::env::current_dir().ok()?;
        if cwd.canonicalize().ok()? != manifest.parent()?.canonicalize().ok()? {
            return None;
        }
        if !plain_path(&out_dir) || !plain_path(&cwd) || !out_dir.starts_with(&plan.target) {
            return None;
        }
        let mut roots = Vec::new();
        for (index, path) in package.sources.iter().enumerate() {
            roots.push(Root {
                label: format!("source/{index}"),
                path: path.clone(),
                recursive: true,
            });
        }
        for (index, (path, recursive)) in plan.compiler.iter().enumerate() {
            roots.push(Root {
                label: format!("compiler/{index}"),
                path: path.clone(),
                recursive: *recursive,
            });
        }
        if let Some(path) = std::env::var_os("OUT_DIR") {
            roots.push(Root {
                label: "generated/out".to_owned(),
                path: PathBuf::from(path),
                recursive: true,
            });
        }
        // Only paths carrying artifact identities are relocatable. Arbitrary
        // path-looking flag values remain exact, as do all declared env values.
        let mut keyed_args = args.clone();
        let mut seen_externs = Vec::new();
        let mut externs = Vec::new();
        for (index, argument) in args.iter().enumerate() {
            let external = argument.strip_prefix("--extern=").or_else(|| {
                (index > 0 && args[index - 1] == "--extern").then_some(argument.as_str())
            });
            if let Some(external) = external {
                if let Some((name, path)) = external.split_once('=') {
                    seen_externs.push(name.to_owned());
                    let path = PathBuf::from(path);
                    let hash = file_digest(&path).ok()?;
                    let vir = path.with_extension("vir");
                    let vir_hash = match std::fs::symlink_metadata(&vir) {
                        Ok(_) => Some(file_digest(&vir).ok()?),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                        Err(_) => return None,
                    };
                    roots.push(Root {
                        label: format!("extern/{name}"),
                        path: path.clone(),
                        recursive: false,
                    });
                    if vir_hash.is_some() {
                        roots.push(Root {
                            label: format!("vir/{name}"),
                            path: vir,
                            recursive: false,
                        });
                    }
                    externs.push((name.to_owned(), hash, vir_hash));
                    let prefix = if argument.starts_with("--extern=") {
                        "--extern="
                    } else {
                        ""
                    };
                    keyed_args[index] = format!("{prefix}{name}=@@extern/{name}@@");
                } else if external != "proc_macro" {
                    return None;
                }
            }
            if index > 0 && args[index - 1] == "--out-dir" {
                "@@output@@".clone_into(&mut keyed_args[index]);
            }
            if argument.starts_with("--out-dir=") {
                "--out-dir=@@output@@".clone_into(&mut keyed_args[index]);
            }
            // Dependency search roots change lookup semantics, including absent
            // files. Only explicit externs are supported by the initial cache.
            if argument == "-L" || argument.starts_with("-L") {
                // Only explicit leaf metadata imports and already-linked
                // proc-macro artifacts are eligible (checked by the plan).
                let item = if argument == "-L" {
                    args.get(index + 1)?
                } else {
                    argument.strip_prefix("-L")?
                };
                let directory = item.strip_prefix("dependency=")?;
                if Path::new(directory) != out_dir {
                    return None;
                }
                let replacement = if argument == "-L" {
                    "dependency=@@output@@"
                } else {
                    "-Ldependency=@@output@@"
                };
                replacement
                    .clone_into(&mut keyed_args[if argument == "-L" { index + 1 } else { index }]);
            }
        }
        // Do not erase the spelling of a declared source file or compiler.
        seen_externs.sort();
        if seen_externs != package.extern_names {
            return None;
        }
        let environment = environment(std::env::vars_os())?;
        let cargo_id = format!(
            "{}-{}-{}",
            environment.get("CARGO_PKG_NAME")?,
            environment.get("CARGO_PKG_VERSION")?,
            digest(manifest.to_str()?.as_bytes()).get(..12)?
        );
        let mut effective = Vec::new();
        for name in [
            " __VERUS_DRIVER_ARGS__".to_owned(),
            format!(" __VERUS_DRIVER_ARGS_FOR_{cargo_id}"),
        ] {
            if let Some(value) = environment.get(&name) {
                effective.extend(value.split("__VERUS_DRIVER_ARGS_SEP__").skip(1));
            }
        }
        // The pinned source checks adjacent VIR only for these named imports.
        // Candidates have no verified descendant; the sole accepted unresolved
        // name is the current crate itself, which cannot be an extern import.
        if effective.iter().any(|arg| {
            arg.starts_with("--import")
                || arg
                    .strip_prefix("import-dep-if-present=")
                    .is_some_and(|name| name != crate_name)
        }) {
            return None;
        }
        // Follow the existing Cargo tracked-read contract: declared values are
        // checked exactly below, while Cargo's generated output root is not
        // itself an observed source input. Full generated tree bytes still bind.
        let mut keyed_environment: BTreeMap<_, _> = environment
            .iter()
            .filter(|(name, _)| {
                let own = [
                    " __VERUS_DRIVER_ARGS_FOR_",
                    "__VERUS_DRIVER_VERIFY_",
                    " __VERUS_DRIVER_IS_BUILTIN_",
                    " __VERUS_DRIVER_IS_BUILTIN_MACROS_",
                ]
                .iter()
                .find_map(|prefix| name.strip_prefix(prefix).map(|id| id == cargo_id));
                name.as_str() == " __VERUS_DRIVER_ARGS__"
                    || own.unwrap_or_else(|| {
                        super::environment::input_environment_key(std::ffi::OsStr::new(
                            name.as_str(),
                        ))
                    })
                    || matches!(
                        name.as_str(),
                        "PATH" | "HOME" | "SDKROOT" | "MACOSX_DEPLOYMENT_TARGET"
                    )
            })
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        // These identify this already-running Sail proxy, not a compiler
        // selected by the dependency. The plan binds the actual proxy/runtime
        // bytes; a declared read of either still requires its exact raw value.
        keyed_environment.remove("CARGO_MAKEFLAGS");
        keyed_environment.remove("MAKEFLAGS");
        keyed_environment.remove("RUSTC_WRAPPER");
        keyed_environment.remove("RUSTC_WORKSPACE_WRAPPER");
        if let Some(out) = keyed_environment.get_mut("OUT_DIR") {
            if !Path::new(out).starts_with(&plan.target) {
                return None;
            }
            "@@generated-out@@".clone_into(out);
        }
        let observation = Observation::capture(&roots)?;
        let key = digest(
            &serde_json::to_vec(&(
                VERSION,
                &package.key,
                &cwd,
                &keyed_args,
                &externs,
                &keyed_environment,
                observation.inventory(),
            ))
            .ok()?,
        );
        Some(Self {
            plan,
            key,
            observation,
            environment,
            out_dir,
            stem,
        })
    }

    fn entry(&self) -> PathBuf {
        self.plan.directory.join(&self.key).join("entry.json")
    }

    /// Validate every stream and payload before any output becomes visible.
    pub(crate) fn serve(&self, err: &mut dyn Write) -> Option<Replay> {
        let entry = self.entry();
        if std::fs::metadata(&entry).is_ok_and(|metadata| metadata.len() > MAX_ENTRY_BYTES) {
            let _ = writeln!(
                err,
                "warning[SAIL3131]: ignoring an oversized verified dependency entry; compiling it normally"
            );
            return None;
        }
        let bytes = read_limited(&entry, MAX_ENTRY_BYTES)?;
        let result = self.restore(&bytes);
        match result {
            Ok(replay) => Some(replay),
            Err(false) => None,
            Err(true) => {
                let _ = writeln!(
                    err,
                    "warning[SAIL3131]: ignoring a damaged verified dependency entry; compiling it normally"
                );
                None
            }
        }
    }

    fn restore(&self, bytes: &[u8]) -> std::result::Result<Replay, bool> {
        let mut stored: Stored = serde_json::from_slice(bytes).map_err(|_| true)?;
        let checksum = std::mem::take(&mut stored.checksum);
        if digest(&serde_json::to_vec(&stored).map_err(|_| true)?) != checksum {
            return Err(true);
        }
        if stored.version != VERSION
            || stored.key != self.key
            || !super::receipts::complete(&stored.report)
        {
            return Err(true);
        }
        if &stored.inventory != self.observation.inventory()
            || !self.observation.unchanged()
            || !stored
                .declared
                .iter()
                .all(|(name, value)| self.environment.get(name) == value.as_ref())
        {
            return Err(false);
        }
        if digest(&serde_json::to_vec(&(&stored.stdout, &stored.stderr)).map_err(|_| true)?)
            != stored.streams_sha256
            || !self.complete_payload(&stored.files)
        {
            return Err(true);
        }
        let output = self.out_dir.to_str().ok_or(false)?;
        let stderr = self.artifacts(&stored.stderr, false).ok_or(true)?;
        // Decode and prepare every stream/payload before creating the first
        // output, so no late parsing error can strand a partial materialization.
        let files = stored
            .files
            .iter()
            .map(|file| {
                let bytes = if file.name == format!("{}.d", self.stem) {
                    dep_info_paths(
                        std::str::from_utf8(&file.bytes).map_err(|_| true)?,
                        "@@output@@",
                        output,
                    )
                    .ok_or(true)?
                    .into_bytes()
                } else {
                    file.bytes.clone()
                };
                Ok((self.out_dir.join(&file.name), bytes))
            })
            .collect::<std::result::Result<Vec<_>, bool>>()?;
        let mut placed = Vec::new();
        for (destination, bytes) in files {
            let opened = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination);
            let written = match opened {
                Ok(mut out) => {
                    // Ownership begins as soon as create_new succeeds, even
                    // when a subsequent write fills the disk or is interrupted.
                    placed.push(destination);
                    out.write_all(&bytes)
                }
                Err(error) => Err(error),
            };
            if written.is_err() {
                for path in placed {
                    let _ = std::fs::remove_file(path);
                }
                return Err(false);
            }
        }
        if !self.observation.unchanged() {
            for path in placed {
                let _ = std::fs::remove_file(path);
            }
            return Err(false);
        }
        Ok(Replay {
            stdout: stored.stdout,
            stderr,
            report: stored.report,
            origin: Origin {
                run: stored.origin,
                key: self.key.clone(),
            },
        })
    }

    fn complete_payload(&self, files: &[File]) -> bool {
        files.len() == 3
            && [
                format!("{}.d", self.stem),
                format!("lib{}.rmeta", self.stem),
                format!("lib{}.vir", self.stem),
            ]
            .iter()
            .all(|name| {
                files
                    .iter()
                    .filter(|file| &file.name == name && digest(&file.bytes) == file.sha256)
                    .count()
                    == 1
            })
    }

    fn allowed(&self, name: &str) -> bool {
        name == format!("{}.d", self.stem)
            || name == format!("lib{}.rmeta", self.stem)
            || name == format!("lib{}.vir", self.stem)
    }

    /// Publish only after the current invocation established a complete proof.
    pub(crate) fn record(
        &self,
        stdout: &[u8],
        stderr: &[u8],
        runtime: &super::receipts::Runtime,
        identity: &super::receipts::TargetIdentity,
    ) {
        let Some(call) = super::receipts::read::<super::receipts::Invocation>(
            &runtime.invocations.join(format!("{}.json", identity.key())),
        ) else {
            return;
        };
        if !call.success
            || !super::receipts::complete(&call.report)
            || !self.observation.unchanged()
        {
            return;
        }
        let Some(mut stored) = self.stage(stdout, stderr, &runtime.run_id, &call.report) else {
            return;
        };
        let Ok(unsealed) = serde_json::to_vec(&stored) else {
            return;
        };
        stored.checksum = digest(&unsealed);
        if !self.observation.unchanged() {
            return;
        }
        let Ok(bytes) = serde_json::to_vec(&stored) else {
            return;
        };
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            return;
        }
        let entry = self.entry();
        let Some(directory) = entry.parent() else {
            return;
        };
        if std::fs::create_dir_all(directory).is_err() {
            return;
        }
        let temporary = directory.join(format!(".record-{}", std::process::id()));
        if std::fs::write(&temporary, bytes).is_ok()
            && self.observation.unchanged()
            && std::fs::rename(&temporary, &entry).is_ok()
        {
            self.prune();
        }
        let _ = std::fs::remove_file(temporary);
    }

    fn stage(
        &self,
        stdout: &[u8],
        stderr: &[u8],
        origin: &str,
        report: &sail_api::output::Report,
    ) -> Option<Stored> {
        if !self.observation.unchanged() {
            return None;
        }
        let replay_stderr = self.artifacts(stderr, true)?;
        if stdout
            .windows(self.plan.target.as_os_str().as_encoded_bytes().len())
            .any(|w| w == self.plan.target.as_os_str().as_encoded_bytes())
        {
            return None;
        }
        let dep_name = format!("{}.d", self.stem);
        let text = std::fs::read_to_string(self.out_dir.join(&dep_name)).ok()?;
        for path in text
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.strip_suffix(':'))
            .filter(|line| !line.is_empty() && !line.contains(": "))
        {
            let path = PathBuf::from(path.replace("\\ ", " "));
            if !self.observation.covers(&path, &file_digest(&path).ok()?) {
                return None;
            }
        }
        let mut declared = BTreeMap::new();
        for line in text
            .lines()
            .filter_map(|line| line.strip_prefix("# env-dep:"))
        {
            let (name, value) = line
                .split_once('=')
                .map_or((line, None), |(name, value)| (name, Some(value.to_owned())));
            // Escaped or path-dependent env values are exact; unsupported escapes
            // conservatively miss until a qualified parser is shared.
            if value.as_ref().is_some_and(|v| v.contains('\\')) {
                return None;
            }
            if self.environment.get(name) != value.as_ref() {
                return None;
            }
            declared.insert(name.to_owned(), value);
        }
        let mut files = Vec::new();
        let mut payload_bytes = 0u64;
        for name in [
            &dep_name,
            &format!("lib{}.rmeta", self.stem),
            &format!("lib{}.vir", self.stem),
        ] {
            let path = self.out_dir.join(name);
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            if !metadata.is_file() {
                return None;
            }
            payload_bytes = payload_bytes.checked_add(metadata.len())?;
            if payload_bytes > MAX_PAYLOAD_BYTES {
                return None;
            }
            let mut bytes = read_limited(&path, MAX_PAYLOAD_BYTES)?;
            if bytes.len() as u64 != metadata.len() {
                return None;
            }
            if *name == dep_name {
                let text = String::from_utf8(bytes).ok()?;
                if text.contains("@@output@@") {
                    return None;
                }
                bytes = dep_info_paths(&text, self.out_dir.to_str()?, "@@output@@")?.into_bytes();
            } else if bytes
                .windows(self.plan.target.as_os_str().as_encoded_bytes().len())
                .any(|w| w == self.plan.target.as_os_str().as_encoded_bytes())
            {
                return None;
            }
            files.push(File {
                name: name.to_owned(),
                sha256: digest(&bytes),
                bytes,
            });
        }
        Some(Stored {
            version: VERSION,
            key: self.key.clone(),
            origin: origin.to_owned(),
            report: report.clone(),
            inventory: self.observation.inventory().clone(),
            declared,
            files,
            stdout: stdout.to_vec(),
            stderr: replay_stderr.clone(),
            streams_sha256: digest(&serde_json::to_vec(&(stdout, &replay_stderr)).ok()?),
            checksum: String::new(),
        })
    }
    fn artifacts(&self, stderr: &[u8], recording: bool) -> Option<Vec<u8>> {
        let mut result = Vec::new();
        for line in stderr.split_inclusive(|b| *b == b'\n') {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let value: Value = serde_json::from_slice(line).ok()?;
            if recording && super::store::long_running_note(&value) {
                continue;
            }
            if value["$message_type"] != "artifact"
                || !value
                    .as_object()?
                    .keys()
                    .all(|k| matches!(k.as_str(), "$message_type" | "artifact" | "emit"))
            {
                return None;
            }
            let artifact = value["artifact"].as_str()?;
            let from = if recording {
                self.out_dir.as_path()
            } else {
                Path::new("@@output@@")
            };
            let name = Path::new(artifact).strip_prefix(from).ok()?.to_str()?;
            if !self.allowed(name) {
                return None;
            }
            let to = if recording {
                Path::new("@@output@@").join(name)
            } else {
                self.out_dir.join(name)
            };
            let source = serde_json::to_string(artifact).ok()?;
            let destination = serde_json::to_string(to.to_str()?).ok()?;
            let line = std::str::from_utf8(line).ok()?;
            // Replace only this exact JSON string value; ambiguous duplicate
            // occurrences decline rather than rewriting another field.
            if line.matches(&source).count() != 1 {
                return None;
            }
            result.extend_from_slice(line.replacen(&source, &destination, 1).as_bytes());
        }
        Some(result)
    }

    fn prune(&self) {
        let mut entries: Vec<_> = std::fs::read_dir(&self.plan.directory)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
            .collect();
        entries.sort_by_key(|(modified, _)| *modified);
        let remove = entries.len().saturating_sub(super::store::capacity());
        for (_, path) in entries.into_iter().take(remove) {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn read_limited(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(limit.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

fn plain_path(path: &Path) -> bool {
    path.to_str().is_some_and(|path| {
        !path.contains("@@")
            && !path
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '\\' | ':' | '"' | '#'))
    })
}

/// Rebase only complete dep-info path tokens, retaining line layout. Declared
/// env values are never rewritten; their exact originals are checked separately.
fn dep_info_paths(text: &str, from: &str, to: &str) -> Option<String> {
    if text.contains('\\') {
        return None;
    }
    let prefix = format!("{from}/");
    let mut result = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with('#') {
            result.push_str(line);
            continue;
        }
        let mut start = 0;
        for token in line.split_ascii_whitespace() {
            let offset = line[start..].find(token)?.checked_add(start)?;
            result.push_str(&line[start..offset]);
            if let Some(relative) = token.strip_prefix(&prefix) {
                result.push_str(to);
                result.push('/');
                result.push_str(relative);
            } else {
                if token.contains(from) {
                    return None;
                }
                result.push_str(token);
            }
            start = offset.checked_add(token.len())?;
        }
        result.push_str(&line[start..]);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        plan: Plan,
    }

    impl Fixture {
        fn report() -> sail_api::output::Report {
            let mut report = crate::report::setup_failure(&[], "SAIL3100", "fixture");
            report.error = None;
            report.exit_code = Some(0);
            report.crates.push(sail_api::output::CrateReport {
                name: Some("demo".to_owned()),
                verus: sail_api::output::VerusResults {
                    verified: Some(0),
                    errors: Some(0),
                    success: Some(true),
                    encountered_vir_error: Some(false),
                },
                functions: Vec::new(),
            });
            report
        }
        fn new(name: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
            let root = std::env::temp_dir()
                .join(format!("sail-driver-store-{name}-{}", std::process::id()));
            if root.exists() {
                std::fs::remove_dir_all(&root)?;
            }
            std::fs::create_dir_all(root.join("source"))?;
            std::fs::create_dir_all(root.join("target/deps"))?;
            std::fs::write(root.join("source/lib.rs"), "mod leaf; pub use leaf::*;\n")?;
            std::fs::write(root.join("source/leaf.rs"), "pub fn value() -> u32 { 7 }\n")?;
            let plan = Plan {
                directory: root.join("store"),
                target: root.join("target"),
                compiler: Vec::new(),
                packages: Vec::new(),
            };
            Ok(Self { root, plan })
        }

        fn call(&self) -> Option<Call<'_>> {
            let roots = [Root {
                label: "source".to_owned(),
                path: self.root.join("source"),
                recursive: true,
            }];
            Some(Call {
                plan: &self.plan,
                key: "key".to_owned(),
                observation: Observation::capture(&roots)?,
                environment: BTreeMap::new(),
                out_dir: self.root.join("target/deps"),
                stem: "demo-hash".to_owned(),
            })
        }

        fn compile(&self) -> std::result::Result<std::process::Output, Box<dyn std::error::Error>> {
            Ok(std::process::Command::new("rustc")
                .args([
                    "--crate-name",
                    "demo",
                    "--crate-type",
                    "lib",
                    "--emit=dep-info,metadata",
                    "-C",
                    "extra-filename=-hash",
                    "--out-dir",
                ])
                .arg(self.root.join("target/deps"))
                .arg(self.root.join("source/lib.rs"))
                .env_remove("SAIL_MANAGED_USER_VALUE")
                .output()?)
        }

        fn payload(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
            let output = self.compile()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            std::fs::write(
                self.root.join("target/deps/libdemo-hash.vir"),
                "fixture verified VIR",
            )?;
            Ok(())
        }

        fn seal(mut stored: Stored) -> std::result::Result<Vec<u8>, serde_json::Error> {
            stored.checksum = digest(&serde_json::to_vec(&stored)?);
            serde_json::to_vec(&stored)
        }

        fn clear(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
            for entry in std::fs::read_dir(self.root.join("target/deps"))? {
                std::fs::remove_file(entry?.path())?;
            }
            Ok(())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn compiler_payload_replays_with_original_origin_and_no_native_files()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("origin")?;
        let Some(call) = fixture.call() else {
            return Err("missing observation".into());
        };
        fixture.payload()?;
        let Some(stored) = call.stage(b"verified", b"", "original-proof", &Fixture::report())
        else {
            return Err("not staged".into());
        };
        assert_eq!(stored.files.len(), 3);
        assert!(stored.files.iter().all(|file| call.allowed(&file.name)));
        let bytes = Fixture::seal(stored)?;
        fixture.clear()?;
        let replay = call.restore(&bytes).map_err(|_| "not restored")?;
        assert_eq!(replay.origin.run, "original-proof");
        assert_eq!(replay.report, Fixture::report());
        assert_eq!(replay.stdout, b"verified");
        assert_eq!(replay.stderr, [] as [u8; 0]);
        Ok(())
    }

    #[test]
    fn real_compiler_ambiguous_module_and_record_race_refuse()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("topology")?;
        let Some(call) = fixture.call() else {
            return Err("missing observation".into());
        };
        fixture.payload()?;
        let Some(stored) = call.stage(b"verified", b"", "origin", &Fixture::report()) else {
            return Err("not staged".into());
        };
        let bytes = Fixture::seal(stored)?;
        fixture.clear()?;
        std::fs::create_dir_all(fixture.root.join("source/leaf"))?;
        std::fs::write(
            fixture.root.join("source/leaf/mod.rs"),
            "pub fn value() -> u32 { 8 }",
        )?;
        assert!(!fixture.compile()?.status.success());
        fixture.clear()?;
        let Some(after) = fixture.call() else {
            return Err("missing new observation".into());
        };
        assert!(after.restore(&bytes).is_err());
        assert!(!call.observation.unchanged());
        assert!(
            call.stage(b"verified", b"", "origin", &Fixture::report())
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn source_modified_after_successful_compiler_cannot_publish()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("race")?;
        let Some(call) = fixture.call() else {
            return Err("missing observation".into());
        };
        fixture.payload()?;
        std::fs::write(fixture.root.join("source/leaf.rs"), "this is not Rust")?;
        assert!(!call.observation.unchanged());
        assert!(
            call.stage(b"verified", b"", "origin", &Fixture::report())
                .is_none()
        );
        assert!(!fixture.compile()?.status.success());
        Ok(())
    }

    #[test]
    fn damaged_stream_or_payload_places_nothing()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("corrupt")?;
        let Some(call) = fixture.call() else {
            return Err("missing observation".into());
        };
        fixture.payload()?;
        let Some(mut stored) = call.stage(b"verified", b"", "origin", &Fixture::report()) else {
            return Err("not staged".into());
        };
        fixture.clear()?;
        stored.stdout.push(b'x');
        let bytes = Fixture::seal(stored)?;
        assert!(call.restore(&bytes).is_err());
        assert_eq!(std::fs::read_dir(&call.out_dir)?.count(), 0);
        Ok(())
    }
    #[test]
    fn repeated_output_controls_and_raw_imports_are_ineligible()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("repeated-output")?;
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "demo".to_owned(),
            "--crate-type".to_owned(),
            "lib".to_owned(),
            "--emit=dep-info,metadata".to_owned(),
            "-C".to_owned(),
            "extra-filename=-hash".to_owned(),
            "--out-dir".to_owned(),
            fixture
                .root
                .join("target/deps")
                .to_string_lossy()
                .into_owned(),
            fixture
                .root
                .join("source/lib.rs")
                .to_string_lossy()
                .into_owned(),
        ];
        assert!(metadata_outputs(&args).is_some());
        let mut repeated = args.clone();
        repeated.push("--emit=link".to_owned());
        let output = std::process::Command::new("rustc")
            .args(&repeated[1..])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(fixture.root.join("target/deps/libdemo-hash.rlib").is_file());
        assert!(metadata_outputs(&repeated).is_none());
        for tail in [
            vec!["--crate-type=proc-macro"],
            vec!["--out-dir=other"],
            vec!["-Cextra-filename=-other"],
            vec!["-C", "extra-filename=-other"],
        ] {
            let mut repeated = args.clone();
            repeated.extend(tail.into_iter().map(str::to_owned));
            assert!(metadata_outputs(&repeated).is_none());
        }
        let run = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "managed::dependency_build::tests::constructor_probe",
                "--nocapture",
            ])
            .env("SAIL_TEST_DEPENDENCY_CONSTRUCTOR", &fixture.root)
            .env("CARGO_MANIFEST_DIR", fixture.root.join("source"))
            .env("CARGO_PKG_NAME", "demo")
            .env("CARGO_PKG_VERSION", "0.1.0")
            .current_dir(fixture.root.join("source"))
            .output()?;
        assert!(
            run.status.success(),
            "{} {}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        Ok(())
    }

    #[test]
    fn excluded_declared_variable_absence_to_presence_refuses_replay()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("declared-routing")?;
        std::fs::write(
            fixture.root.join("source/lib.rs"),
            "pub const FLAG: Option<&str> = option_env!(\"SAIL_MANAGED_USER_VALUE\");",
        )?;
        let Some(mut call) = fixture.call() else {
            return Err("missing observation".into());
        };
        fixture.payload()?;
        let Some(stored) = call.stage(b"verified", b"", "origin", &Fixture::report()) else {
            return Err("not staged".into());
        };
        assert_eq!(stored.declared.get("SAIL_MANAGED_USER_VALUE"), Some(&None));
        let bytes = Fixture::seal(stored)?;
        fixture.clear()?;
        call.environment = environment(std::iter::once((
            OsString::from("SAIL_MANAGED_USER_VALUE"),
            OsString::from("present"),
        )))
        .ok_or("environment")?;
        assert_eq!(
            call.environment
                .get("SAIL_MANAGED_USER_VALUE")
                .map(String::as_str),
            Some("present")
        );
        assert!(call.restore(&bytes).is_err());
        assert_eq!(std::fs::read_dir(&call.out_dir)?.count(), 0);
        Ok(())
    }
    #[test]
    fn constructor_probe() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let Some(root) = std::env::var_os("SAIL_TEST_DEPENDENCY_CONSTRUCTOR") else {
            return Ok(());
        };
        let root = PathBuf::from(root);
        std::fs::write(
            root.join("source/Cargo.toml"),
            "[package]\nname=\"demo\"\nversion=\"0.1.0\"\n",
        )?;
        std::fs::write(root.join("compiler"), "observed compiler fixture")?;
        let plan = Plan {
            directory: root.join("store"),
            target: root.join("target"),
            compiler: vec![(root.join("compiler"), false)],
            packages: vec![Package {
                manifest: root.join("source/Cargo.toml"),
                key: "candidate".to_owned(),
                sources: vec![root.join("source")],
                extern_names: Vec::new(),
            }],
        };
        let args: Vec<OsString> = [
            "rustc",
            "--crate-name",
            "demo",
            "--crate-type",
            "lib",
            "--emit=dep-info,metadata",
            "-C",
            "extra-filename=-hash",
            "--out-dir",
        ]
        .into_iter()
        .map(OsString::from)
        .chain([
            root.join("target/deps").into_os_string(),
            root.join("source/lib.rs").into_os_string(),
        ])
        .collect();
        assert!(Call::new(&plan, &args).is_some());
        for tail in [
            vec!["--emit=link"],
            vec!["--crate-type=proc-macro"],
            vec!["--out-dir=other"],
            vec!["-Cextra-filename=-other"],
            vec!["--import=foreign=/tmp/foreign.vir"],
            vec!["--import", "foreign=/tmp/foreign.vir"],
            vec!["-V", "import-dep-if-present=foreign"],
        ] {
            let mut variant = args.clone();
            variant.extend(tail.into_iter().map(OsString::from));
            assert!(Call::new(&plan, &variant).is_none(), "{variant:?}");
        }
        Ok(())
    }
}

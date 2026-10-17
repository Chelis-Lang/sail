//! Explicit managed-driver join for the named, default-off content experiment.
//!
//! The normal compiler remains the miss path. Native producer admission is
//! independent of the verifier's final source snapshot and proof receipt.

use super::super::{dependency_snapshot, files, input, receipts};
use super::{archive, compiler, eligibility};
use dependency_snapshot::{Content, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A per-compiler-family request, selected only by the explicit research switch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Plan {
    pub(super) root: PathBuf,
    pub(super) target: PathBuf,
    metadata: Value,
    roots: BTreeSet<PathBuf>,
    pub(super) closure: eligibility::Closure,
    launcher: PathBuf,
    launcher_sha256: String,
    compiler_sha256: String,
}

impl Plan {
    pub(crate) fn prepare(
        metadata: &Value,
        target: &Path,
        selected: &BTreeSet<(String, String, String)>,
        launcher: &Path,
    ) -> Option<Self> {
        if std::env::var("SAIL_RETAINED_CONTENT_RESEARCH").as_deref() != Ok("1") {
            return None;
        }
        let packages = metadata["packages"].as_array()?;
        let provider = |name: &str| {
            packages.iter().find(|p| p["name"] == name)?["manifest_path"]
                .as_str()
                .map(PathBuf::from)
        };
        let vstd = provider("vstd")?;
        let source = vstd.parent()?.parent()?.parent()?;
        let registry_manifest = provider("proc-macro2")?;
        let registry = registry_manifest.parent()?.parent()?;
        let mut roots = BTreeSet::new();
        let mut closure = None;
        for (id, _, kind) in selected {
            if kind != "lib" {
                return None;
            }
            let package = packages.iter().find(|p| p["id"] == *id)?;
            let manifest = PathBuf::from(package["manifest_path"].as_str()?);
            let admitted = eligibility::Closure::admit(metadata, &manifest, source, registry)?;
            roots.insert(manifest);
            closure = Some(admitted);
        }
        let root = target.parent()?.join("retained-content-v1");
        std::fs::create_dir_all(root.join("graphs")).ok()?;
        std::fs::create_dir_all(root.join("live-graphs")).ok()?;
        let launcher = launcher.canonicalize().ok()?;
        let compiler = launcher.with_file_name("rust_verify");
        Some(Self {
            launcher_sha256: files::file_digest(&launcher).ok()?,
            compiler_sha256: files::file_digest(&compiler).ok()?,
            launcher,
            root: root.canonicalize().ok()?,
            target: target.canonicalize().ok()?,
            metadata: metadata.clone(),
            roots,
            closure: closure?,
        })
    }

    pub(super) fn native_edges(&self, args: &[OsString], manifest: &Path) -> bool {
        self.metadata["packages"]
            .as_array()
            .and_then(|packages| {
                packages.iter().find(|package| {
                    package["manifest_path"].as_str().map(Path::new) == Some(manifest)
                })
            })
            .and_then(|package| package["id"].as_str())
            .is_some_and(|id| direct_externs(&self.metadata, id, args, true))
    }

    pub(super) fn compiler_matches(&self, wrapper: &Path) -> bool {
        wrapper.canonicalize().ok().as_ref() == Some(&self.launcher)
            && files::file_digest(wrapper).ok().as_ref() == Some(&self.launcher_sha256)
            && files::file_digest(&self.launcher.with_file_name("rust_verify"))
                .ok()
                .as_ref()
                == Some(&self.compiler_sha256)
    }

    pub(super) fn compiler_identity(&self) -> &str {
        &self.compiler_sha256
    }

    /// Called only after ordinary whole-run Current and receipt commit.
    pub(crate) fn publish(&self, runtime: &receipts::Runtime) -> Option<()> {
        let snapshot = self.snapshot()?;
        let live = std::fs::read_dir(self.root.join("live-graphs"))
            .ok()?
            .map(|entry| receipts::read::<String>(&entry.ok()?.path()))
            .collect::<Option<BTreeSet<_>>>()?;
        for entry in std::fs::read_dir(&runtime.receipts).ok()? {
            let receipt: receipts::Receipt = receipts::read(&entry.ok()?.path())?;
            if receipt.invocation.run_id != runtime.run_id
                || receipt.invocation.retained_from.is_some()
                || !self.roots.contains(&receipt.invocation.identity.manifest)
            {
                continue;
            }
            let directory = runtime.sidecars.join(receipt.invocation.identity.key());
            let Some(candidate): Option<Candidate> =
                receipts::read(&directory.join("retained-candidate.json"))
            else {
                continue;
            };
            if candidate.basis.context != runtime.context_sha256
                || candidate.basis.identity != receipt.invocation.identity
                || candidate.basis.source_revision != snapshot.revision
                || !receipt.valid(&runtime.context_sha256)
                || candidate.outputs.iter().any(|output| {
                    dependency_snapshot::read_content(&output.path, 256 * 1024 * 1024).as_ref()
                        != Some(&output.content)
                })
                || dependency_snapshot::read_content(&candidate.reads_path, 8 * 1024 * 1024)
                    .as_ref()
                    != Some(&candidate.compiler_reads)
            {
                continue;
            }
            let entry = archive::Entry::new(
                candidate.basis,
                receipt,
                candidate.compiler_reads,
                candidate.outputs,
                candidate.stdout,
                candidate.stderr,
            )?;
            let _ = archive::retain(
                &self.root,
                &entry,
                [
                    &directory.join("verus.stdout"),
                    &directory.join("verus.stderr"),
                    &candidate.reads_path,
                ],
                &live,
            );
        }
        Some(())
    }

    fn snapshot(&self) -> Option<input::InputSnapshot> {
        // Declared graph files are separately bound immutable inputs. Do not
        // make the source-content key depend on the ever-growing historical
        // dep-info selection; unknown extra reads still fail compiler admission.
        let mut metadata = self.metadata.clone();
        metadata["sail_source_inputs"] = serde_json::json!([]);
        let mut snapshot = input::snapshot_metadata(&metadata, &[]);
        if !snapshot.complete || !self.closure.check_sources() {
            return None;
        }
        super::namespace::admit(&snapshot.digests, &self.closure.source_roots())?;
        snapshot.revision =
            files::digest(&serde_json::to_vec(&(&snapshot.revision, namespace(&snapshot)?)).ok()?);
        Some(snapshot)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "One admission binds source, Cargo edges, private graph and actual compiler request before spawn."
    )]
    pub(crate) fn begin(
        &self,
        runtime: &receipts::Runtime,
        identity: &receipts::TargetIdentity,
        args: &[OsString],
        directory: &Path,
    ) -> Option<Attempt> {
        if runtime.engine.is_some() || identity.crate_type != "lib" && identity.crate_type != "rlib"
        {
            return None;
        }
        if !closed_verus_options(&crate::adapter::effective_verus_arguments()?) {
            return None;
        }
        let variables = super::producers::environment()?;
        if !self.roots.contains(&identity.manifest) && !self.closure.unit(&variables, args) {
            return None;
        }
        let source_argument = ordinary_source(args, &self.target)?;
        output_paths(args, &self.target)?;
        let cwd = std::env::current_dir().ok()?;
        let package = self.metadata["packages"]
            .as_array()?
            .iter()
            .find(|package| {
                package["manifest_path"].as_str().map(Path::new)
                    == Some(identity.manifest.as_path())
            })?;
        if !direct_externs(&self.metadata, package["id"].as_str()?, args, false) {
            return None;
        }
        let actual_source = package["targets"].as_array()?.iter().find_map(|target| {
            (target["name"].as_str()?.replace('-', "_") == identity.name)
                .then(|| target["src_path"].as_str().map(PathBuf::from))?
        })?;
        if cwd.join(source_argument).canonicalize().ok()? != actual_source.canonicalize().ok()? {
            return None;
        }
        let sources = self.snapshot()?;
        let environment = std::env::vars_os()
            .map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect::<Option<BTreeMap<_, _>>>()?;
        // Cargo may invoke this verifier as soon as dependency metadata is
        // ready. Wait for those actual successful producers, discard the first
        // view, then capture the complete final directory (including rlibs).
        let first =
            Snapshot::capture_ready(&self.target, &directory.join("dependency-first"), args)?;
        if !super::producers::ready(self, &first) {
            return None;
        }
        drop(first);
        let graph =
            Snapshot::reuse(&self.target, &self.root.join("graphs"), args).or_else(|| {
                Snapshot::capture_ready(
                    &self.target,
                    &self
                        .root
                        .join(format!("copy-{}-{}", runtime.run_id, std::process::id())),
                    args,
                )?
                .publish(&self.root.join("graphs"), args)
            })?;
        if !super::producers::ready(self, &graph)
            || !super::producers::established(self, &runtime.context_sha256, &graph)
            || !super::producers::named_externs(self, args)
        {
            return None;
        }
        let graph_key = graph.key()?;
        // Register before any archive eviction can collect a graph named by
        // this compiler generation's future dep-info. Publication is not a
        // temporary driver lease: the generation owns it after the child exits.
        files::json(&self.root.join("live-graphs").join(&graph_key), &graph_key).ok()?;
        let basis = archive::Basis {
            identity: identity.clone(),
            context: runtime.context_sha256.clone(),
            source_revision: sources.revision.clone(),
            dependency_inventory: graph_key,
            directory: cwd.clone(),
            environment: environment
                .iter()
                .filter(|(name, _)| keyed_environment(name))
                .map(|(name, value)| {
                    (
                        name.clone(),
                        Some(files::digest(value.as_bytes()).into_bytes()),
                    )
                })
                .collect(),
        };
        // Whole-directory source coverage, not a dep-info-only foreign leaf:
        // adding a sibling module must change the namespace/content key too.
        let whole_roots: Vec<_> = sources.roots.iter().filter(|root| root.is_dir()).collect();
        let excluded: Vec<_> = sources
            .excluded
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .collect();
        let source_contents = sources
            .digests
            .iter()
            .filter(|(path, _)| {
                whole_roots.iter().any(|root| path.starts_with(root))
                    && !excluded.iter().any(|root| path.starts_with(root))
            })
            .map(|(path, sha)| {
                let content = dependency_snapshot::read_content(path, 256 * 1024 * 1024)?;
                (content.sha256 == *sha).then(|| (path.clone(), content))
            })
            .collect::<Option<_>>()?;
        let observation = compiler::Observation::begin(
            &directory.join("compiler-inputs"),
            &runtime.run_id,
            &identity.key(),
        )?;
        Some(Attempt {
            plan: self.clone(),
            basis,
            source_contents,
            cwd,
            environment,
            graph,
            observation,
            original_arguments: args.to_vec(),
            directory: directory.to_owned(),
        })
    }
}

/// A checked older proof; compiler invocations remain zero for this restore.
pub(crate) struct Restored {
    pub(crate) establishing_run: String,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// One real workspace compiler invocation and its immutable dependency inputs.
pub(crate) struct Attempt {
    plan: Plan,
    basis: archive::Basis,
    source_contents: BTreeMap<PathBuf, Content>,
    cwd: PathBuf,
    environment: BTreeMap<String, String>,
    graph: Snapshot,
    original_arguments: Vec<OsString>,
    observation: compiler::Observation,
    directory: PathBuf,
}

impl Attempt {
    pub(crate) fn restore(&self) -> crate::Result<Option<Restored>> {
        if !self.plan.roots.contains(&self.basis.identity.manifest) {
            return Ok(None);
        }
        let Some(paths) = output_paths(&self.original_arguments, &self.plan.target) else {
            return Ok(None);
        };
        let Some(directory) = paths.first().and_then(|path| path.parent()) else {
            return Ok(None);
        };
        let names = paths
            .iter()
            .filter_map(|path| path.file_name().map(std::ffi::OsStr::to_owned))
            .collect();
        archive::restore(&self.plan.root, &self.basis, directory, &names, || {
            self.unchanged()
        })
        .map(|restored| {
            restored.map(|restored| Restored {
                establishing_run: restored.receipt.invocation.establishing_run().to_owned(),
                stdout: restored.stdout,
                stderr: restored.stderr,
            })
        })
        .map_err(|failure| {
            crate::Error::new(
                "SAIL3134",
                format!(
                    "retained proof rollback failed; refusing this compiler attempt: {:?}",
                    failure.paths
                ),
            )
        })
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        self.graph.arguments()
    }

    pub(crate) fn configure(&self, command: &mut std::process::Command) {
        self.observation.configure(command);
    }

    pub(crate) fn started(&mut self, pid: u32) {
        self.observation.started(pid);
    }

    fn unchanged(&self) -> bool {
        self.graph.intact()
            && self
                .plan
                .snapshot()
                .is_some_and(|current| current.revision == self.basis.source_revision)
            && self
                .environment
                .iter()
                .filter(|(name, _)| keyed_environment(name))
                .all(|(name, value)| std::env::var(name).as_ref() == Ok(value))
            && Snapshot::reuse(
                &self.plan.target,
                &self.plan.root.join("graphs"),
                &self.original_arguments,
            )
            .is_some_and(|graph| graph.key().as_ref() == Some(&self.basis.dependency_inventory))
    }

    /// Stores only an admission witness. Actual archive publication is later,
    /// after the ordinary whole-run Current result and committed receipt.
    pub(crate) fn finish(&self, success: bool) -> Option<()> {
        let accepted = self.observation.finish(
            success,
            &self.cwd,
            &self.source_contents,
            &self.graph.copied_inputs()?,
            &self.environment,
        )?;
        if !self.unchanged()
            || accepted
                .tracked_environment
                .keys()
                .any(|name| !keyed_environment(name))
        {
            return None;
        }
        let reads =
            dependency_snapshot::read_content(&self.observation.document(), 8 * 1024 * 1024)?;
        if reads.sha256 != accepted.document_sha256 {
            return None;
        }
        let invocation: receipts::Invocation = receipts::read(
            &self
                .directory
                .parent()?
                .parent()?
                .join("invocations")
                .join(format!("{}.json", self.basis.identity.key())),
        )?;
        if !invocation.success || !receipts::complete(&invocation.report) {
            return None;
        }
        let outputs = output_paths(&self.original_arguments, &self.plan.target)?
            .into_iter()
            .map(|path| {
                Some(archive::Output {
                    content: dependency_snapshot::read_content(&path, 256 * 1024 * 1024)?,
                    path,
                    executable: false,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let streams = ["verus.stdout", "verus.stderr"].map(|name| {
            dependency_snapshot::read_content(&self.directory.join(name), 16 * 1024 * 1024)
        });
        let [Some(stdout), Some(stderr)] = streams else {
            return None;
        };
        super::producers::publish_verified(
            &self.plan,
            &self.basis.context,
            &invocation.run_id,
            &self.basis.identity.key(),
            &self.observation.document(),
            &reads,
            &outputs,
            &self.environment,
            &self.original_arguments,
        )?;
        files::json(
            &self.directory.join("retained-candidate.json"),
            &Candidate {
                basis: self.basis.clone(),
                compiler_reads: reads,
                reads_path: self.observation.document(),
                original_inputs: self.graph.original_inputs().clone(),
                outputs,
                stdout,
                stderr,
            },
        )
        .ok()
    }
}

#[derive(Serialize, Deserialize)]
struct Candidate {
    basis: archive::Basis,
    compiler_reads: Content,
    reads_path: PathBuf,
    original_inputs: BTreeMap<PathBuf, Content>,
    outputs: Vec<archive::Output>,
    stdout: Content,
    stderr: Content,
}

/// Content keys include namespace facts, never inode/mtime freshness. The
/// compiler documents separately prove what was actually consumed.
fn namespace(snapshot: &input::InputSnapshot) -> Option<String> {
    let mut paths = snapshot.observation_paths.clone();
    paths.extend(snapshot.roots.iter().cloned());
    if paths.len() > 131_072 {
        return None;
    }
    let mut hash = Sha256::new();
    let mut entries = 0usize;
    let mut bytes = 0usize;
    let mut add = |value: &[u8]| -> Option<()> {
        bytes = bytes.checked_add(value.len())?;
        if bytes > 64 * 1024 * 1024 {
            return None;
        }
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
        Some(())
    };
    for path in paths {
        add(path.as_os_str().as_encoded_bytes())?;
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        if metadata.file_type().is_symlink() {
            add(b"link")?;
            add(std::fs::read_link(&path)
                .ok()?
                .as_os_str()
                .as_encoded_bytes())?;
        } else if metadata.is_dir() {
            add(b"directory")?;
            let mut names = BTreeMap::new();
            for entry in std::fs::read_dir(&path).ok()? {
                let entry = entry.ok()?;
                entries = entries.checked_add(1)?;
                if entries > 262_144 {
                    return None;
                }
                let kind = entry.file_type().ok()?;
                let kind = if kind.is_symlink() {
                    1u8
                } else if kind.is_dir() {
                    2
                } else if kind.is_file() {
                    3
                } else {
                    return None;
                };
                names.insert(entry.file_name(), kind);
            }
            for (name, kind) in names {
                add(name.as_encoded_bytes())?;
                add(&[kind])?;
            }
        } else if metadata.is_file() {
            add(b"file")?;
        } else {
            return None;
        }
    }
    Some(format!("{:x}", hash.finalize()))
}

/// Actual extern names must be precisely Cargo's admitted normal edges, with
/// no duplicate, renamed-by-rustflags or injected self/import edge.
fn direct_externs(
    metadata: &Value,
    package: &str,
    args: &[OsString],
    system_proc_macro: bool,
) -> bool {
    let expected = (|| -> Option<BTreeSet<String>> {
        let node = metadata["resolve"]["nodes"]
            .as_array()?
            .iter()
            .find(|node| node["id"] == package)?;
        node["deps"]
            .as_array()?
            .iter()
            .filter_map(|dependency| {
                let kinds = dependency["dep_kinds"].as_array();
                match kinds {
                    Some(kinds) if !kinds.is_empty() => kinds
                        .iter()
                        .any(|kind| kind["kind"].is_null())
                        .then(|| dependency["name"].as_str().map(str::to_owned)),
                    _ => Some(None),
                }
            })
            .collect()
    })();
    let Some(expected) = expected else {
        return false;
    };
    let mut actual = BTreeSet::new();
    for (index, argument) in args.iter().enumerate() {
        let Some(text) = argument.to_str() else {
            return false;
        };
        let external = text
            .strip_prefix("--extern=")
            .or_else(|| (index > 0 && args[index - 1] == "--extern").then_some(text));
        if let Some(external) = external {
            if system_proc_macro && external == "proc_macro" {
                continue;
            }
            let Some((name, _)) = external.split_once('=') else {
                return false;
            };
            if !actual.insert(name.to_owned()) {
                return false;
            }
        }
    }
    actual == expected
}

/// A custom import, skipped proof mode or hidden Rust tail cannot borrow the
/// normal Cargo dependency-DAG assumption. Literal tokens remain in the key.
fn closed_verus_options(arguments: &[String]) -> bool {
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--output-json" | "--time" | "--time-expanded" => index += 1,
            "--rlimit" | "--multiple-errors" | "--num-threads" => {
                if arguments
                    .get(index + 1)
                    .is_none_or(|value| value.parse::<u32>().is_err())
                {
                    return false;
                }
                index += 2;
            }
            _ => return false,
        }
    }
    true
}

fn keyed_environment(name: &str) -> bool {
    !name.starts_with("SAIL_MANAGED_")
        && !matches!(
            name,
            "CARGO_MAKEFLAGS"
                | "SAIL_PROVENANCE_DIR"
                | "SAIL_COMPILER_INPUTS"
                | "SAIL_COMPILER_INPUTS_BINDING"
        )
}

/// Custom loaders/sysroots, imports, alternate backends, response files and
/// unknown codegen switches cannot borrow the closed Cargo graph assumption.
pub(super) fn ordinary_source<'a>(args: &'a [OsString], target: &Path) -> Option<&'a str> {
    let args = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()?;
    let mut index = 1; // Cargo's rustc hint is not rust_verify's actual executable.
    let mut source = None;
    while let Some(arg) = args.get(index).copied() {
        match arg {
            "--crate-name" | "--crate-type" | "--edition" | "--emit" | "--out-dir"
            | "--error-format" | "--json" | "--cap-lints" | "--cfg" | "--check-cfg"
            | "--extern" => {
                args.get(index + 1)?;
                index += 2;
            }
            "-L" => {
                if !args
                    .get(index + 1)
                    .is_some_and(|value| value.starts_with("dependency="))
                {
                    return None;
                }
                index += 2;
            }
            "-C" => {
                if !args
                    .get(index + 1)
                    .is_some_and(|value| codegen(value, target))
                {
                    return None;
                }
                index += 2;
            }
            "-Z" => {
                if args.get(index + 1).copied() != Some("checksum-hash-algorithm=blake3") {
                    return None;
                }
                index += 2;
            }
            value if value.starts_with("-C") => {
                if !codegen(&value[2..], target) {
                    return None;
                }
                index += 1;
            }
            value if value.starts_with("-Ldependency=") => {
                index += 1;
            }
            value if value.starts_with("--") => {
                let (name, _) = value.split_once('=')?;
                if ![
                    "--crate-name",
                    "--crate-type",
                    "--edition",
                    "--emit",
                    "--out-dir",
                    "--error-format",
                    "--json",
                    "--cap-lints",
                    "--cfg",
                    "--check-cfg",
                    "--extern",
                ]
                .contains(&name)
                {
                    return None;
                }
                index += 1;
            }
            value if !value.starts_with('-') && !value.starts_with('@') => {
                if source.replace(value).is_some() {
                    return None;
                }
                index += 1;
            }
            _ => return None,
        }
    }
    source
}

fn codegen(value: &str, target: &Path) -> bool {
    if let Some(mode) = value.strip_prefix("split-debuginfo=") {
        return matches!(mode, "off" | "packed" | "unpacked");
    }
    if value == "prefer-dynamic" {
        return true;
    }
    value.split_once('=').is_some_and(|(name, argument)| {
        if name == "incremental" {
            return Path::new(argument).starts_with(target)
                && Path::new(argument)
                    .components()
                    .all(|part| !matches!(part, std::path::Component::ParentDir));
        }
        [
            "metadata",
            "extra-filename",
            "embed-bitcode",
            "debuginfo",
            "opt-level",
            "debug-assertions",
            "overflow-checks",
            "panic",
            "prefer-dynamic",
            "strip",
            "lto",
            "codegen-units",
        ]
        .contains(&name)
    })
}

pub(super) fn value<'a>(args: &'a [OsString], flag: &str) -> Option<&'a str> {
    args.iter().enumerate().find_map(|(index, argument)| {
        let text = argument.to_str()?;
        text.strip_prefix(&format!("{flag}=")).or_else(|| {
            (text == flag)
                .then(|| args.get(index + 1)?.to_str())
                .flatten()
        })
    })
}

/// Only actual normal Cargo library outputs, never a broad name-prefix scan.
fn output_paths(args: &[OsString], target: &Path) -> Option<Vec<PathBuf>> {
    let directory = PathBuf::from(value(args, "--out-dir")?);
    let name = value(args, "--crate-name")?;
    let extra = args.iter().enumerate().find_map(|(index, arg)| {
        let text = arg.to_str()?;
        text.strip_prefix("-Cextra-filename=").or_else(|| {
            (text == "-C")
                .then(|| {
                    args.get(index + 1)?
                        .to_str()?
                        .strip_prefix("extra-filename=")
                })
                .flatten()
        })
    })?;
    if directory.canonicalize().ok()? != directory
        || !directory.starts_with(target)
        || name.is_empty()
        || extra.is_empty()
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        || !extra
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || !matches!(value(args, "--crate-type")?, "lib" | "rlib")
    {
        return None;
    }
    let stem = format!("{name}{extra}");
    let mut paths = BTreeSet::new();
    for emission in value(args, "--emit")?.split(',') {
        paths.insert(directory.join(match emission {
            "dep-info" => format!("{stem}.d"),
            "metadata" => format!("lib{stem}.rmeta"),
            "link" => format!("lib{stem}.rlib"),
            _ => return None,
        }));
    }
    paths.insert(directory.join(format!("lib{stem}.vir")));
    Some(paths.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_proof_mode_overrides_cannot_hide_in_cargo_effective_options() {
        assert!(closed_verus_options(&[]));
        assert!(closed_verus_options(&[
            "--output-json".into(),
            "--rlimit".into(),
            "10".into()
        ]));
        for option in [
            "--import",
            "--no-verify",
            "--verify-function",
            "--no-vstd",
            "--",
            "--compile",
        ] {
            assert!(!closed_verus_options(&[option.into()]));
        }
        let metadata = serde_json::json!({"resolve":{"nodes":[{"id":"root","deps":[{
            "name":"vstd","dep_kinds":[{"kind":null}]}]}]}});
        let args: Vec<OsString> = ["hint", "--extern", "vstd=/owned/libvstd.rmeta"]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert!(direct_externs(&metadata, "root", &args, false));
        let mut extra = args.clone();
        extra.extend(["--extern".into(), "foreign=/owned/libforeign.rmeta".into()]);
        assert!(!direct_externs(&metadata, "root", &extra, false));
        let mut duplicate = args.clone();
        duplicate.extend(args.into_iter().skip(1));
        assert!(!direct_externs(&metadata, "root", &duplicate, false));
    }

    #[test]
    fn normal_library_output_family_is_exact_and_custom_compiler_routes_decline()
    -> Result<(), Box<dyn std::error::Error>> {
        let target =
            std::env::temp_dir().join(format!("sail-retained-command-{}", std::process::id()));
        std::fs::create_dir(&target)?;
        let target = target.canonicalize()?;
        {
            let args: Vec<OsString> = [
                "/rustc-hint",
                "--crate-name",
                "vest_lib",
                "--crate-type",
                "lib",
                "--emit=dep-info,metadata",
                "-C",
                "extra-filename=-abcd",
                "--out-dir",
            ]
            .into_iter()
            .map(OsString::from)
            .chain([target.clone().into(), "src/lib.rs".into()])
            .collect();
            assert!(ordinary_source(&args, &target).is_some());
            assert_eq!(
                output_paths(&args, &target),
                Some(vec![
                    target.join("libvest_lib-abcd.rmeta"),
                    target.join("libvest_lib-abcd.vir"),
                    target.join("vest_lib-abcd.d"),
                ])
            );
            for route in [
                vec!["--sysroot", "/foreign"],
                vec!["--import", "foreign.vir"],
                vec!["-C", "linker=foreign"],
                vec!["@response"],
                vec!["--cfg"],
            ] {
                let mut changed = args.clone();
                changed.extend(route.into_iter().map(OsString::from));
                assert!(ordinary_source(&changed, &target).is_none());
            }
            let mut changed = args.clone();
            changed[5] = "--emit=metadata=/foreign/output".into();
            assert!(output_paths(&changed, &target).is_none());
            let mut changed = args;
            changed[7] = "extra-filename=-abcd/escape".into();
            assert!(output_paths(&changed, &target).is_none());
        }
        std::fs::remove_dir_all(target)?;
        Ok(())
    }
}

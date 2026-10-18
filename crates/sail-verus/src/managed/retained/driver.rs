//! Managed-driver join for content restoration with closed source authority.
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

#[cfg(test)]
#[path = "late_guard_tests.rs"]
mod late_guard_tests;
#[cfg(test)]
pub(crate) use late_guard_tests::run as actual_late_guard;

/// A per-compiler-family request with explicit source and runtime admission.
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
    caller_environment: archive::CallerEnvironment,
}

impl Plan {
    pub(crate) fn requested(&self, identity: &receipts::TargetIdentity) -> bool {
        self.roots.contains(&identity.manifest)
    }

    pub(crate) fn prepare(
        metadata: &Value,
        target: &Path,
        selected: &BTreeSet<(String, String, String)>,
        launcher: &Path,
        z3: &Path,
        run: &str,
    ) -> Option<Self> {
        let mode = super::super::research::selected().ok()?;
        if !super::selection::requested(mode) {
            return None;
        }
        if matches!(mode, super::super::research::Mode::Off)
            && !eligibility::Closure::automatic_profile_candidate(metadata)
        {
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
        let launcher = launcher.canonicalize().ok()?;
        if !super::selection::admitted(mode, &launcher, z3) {
            return None;
        }
        let root = target.parent()?.join("retained-content-v1");
        std::fs::create_dir_all(root.join("graphs")).ok()?;
        std::fs::create_dir_all(root.join("live-graphs")).ok()?;
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
            caller_environment: archive::CallerEnvironment::capture(run)?,
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
        if !self.caller_environment.for_run(&runtime.run_id) {
            return None;
        }
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
        super::namespace::admit(
            &super::namespace::closed_inventory(&snapshot.digests, &self.compile_source_roots()?),
            &self.closure.source_roots(),
        )?;
        snapshot.revision =
            files::digest(&serde_json::to_vec(&(&snapshot.revision, namespace(&snapshot)?)).ok()?);
        Some(snapshot)
    }

    fn compile_source_roots(&self) -> Option<BTreeSet<PathBuf>> {
        let mut roots = self.closure.source_roots();
        for manifest in &self.roots {
            roots.insert(manifest.parent()?.canonicalize().ok()?);
        }
        Some(roots)
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
        if !self.caller_environment.for_run(&runtime.run_id)
            || runtime.engine.is_some()
            || identity.crate_type != "lib" && identity.crate_type != "rlib"
        {
            return None;
        }
        let variables = super::producers::environment()?;
        let official = self.closure.unit(&variables, args);
        let primary = self.roots.contains(&identity.manifest);
        if (!primary && !official)
            || variables.get("CARGO_PRIMARY_PACKAGE").map(String::as_str) != primary.then_some("1")
        {
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
        let is_vstd = official
            && package["name"] == "vstd"
            && package
                .pointer("/metadata/verus/is-vstd")
                .and_then(Value::as_bool)
                == Some(true);
        if !closed_verus_options(
            &crate::adapter::effective_verus_arguments()?,
            is_vstd,
            &cargo_imports(&self.metadata, package)?,
        ) {
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
        let graph = if let Some(graph) =
            Snapshot::reuse(&self.target, &self.root.join("graphs"), args)
        {
            publish_graph(&self.root, graph, args)?
        } else {
            publish_graph(
                &self.root,
                Snapshot::capture_ready(&self.target, &directory.join("dependency-final"), args)?,
                args,
            )?
        };
        if !super::producers::ready(self, &graph)
            || !super::producers::established(self, &runtime.context_sha256, &graph)
            || !super::producers::named_externs(self, args)
        {
            return None;
        }
        let graph_key = graph.key()?;
        // publish_graph already registered the generation's durable paths.
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
        let compile_roots = self.compile_source_roots()?;
        let excluded: Vec<_> = sources
            .excluded
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .collect();
        let source_contents = sources
            .digests
            .iter()
            .filter(|(path, _)| {
                compile_roots.iter().any(|root| path.starts_with(root))
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

/// Publish one complete graph and its deduplicated lifetime marker together.
/// The caller owns the temporary snapshot outside the durable arena.
pub(super) fn publish_graph(root: &Path, graph: Snapshot, args: &[OsString]) -> Option<Snapshot> {
    let key = graph.key()?;
    let incoming = graph.storage()?;
    // Reserve the entire graph even when an identical one already exists.
    // This deliberately conservative peak remains valid across publication/GC.
    let _admission = super::budget::admit(
        root,
        incoming.0.checked_add(66)?,
        incoming.1.checked_add(2)?,
    )?;
    let graph = graph.publish(&root.join("graphs"), args)?;
    if !graph.intact() {
        return None;
    }
    files::json(&root.join("live-graphs").join(key.clone()), &key).ok()?;
    Some(graph)
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
    pub(crate) fn restore(&self, runtime: &receipts::Runtime) -> crate::Result<Option<Restored>> {
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
        archive::restore(
            &self.plan.root,
            &self.basis,
            directory,
            &names,
            &self.plan.caller_environment,
            &runtime.run_id,
            || self.unchanged(),
        )
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
            && super::producers::established(&self.plan, &self.basis.context, &self.graph)
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
fn closed_verus_options(arguments: &[String], is_vstd: bool, imports: &BTreeSet<String>) -> bool {
    let mut index = 0;
    let mut compile_dependencies = false;
    let mut vstd = false;
    let mut actual_imports = BTreeSet::new();
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--is-vstd" if is_vstd && !vstd => {
                vstd = true;
                index += 1;
            }
            "--VIA-CARGO" => {
                let Some(mode) = arguments.get(index + 1) else {
                    return false;
                };
                if mode == "compile-when-not-primary-package" && !compile_dependencies {
                    // This pinned global mode is present for primary units too;
                    // Verus applies it only when Cargo's bound primary flag is absent.
                    compile_dependencies = true;
                } else if let Some(name) = mode.strip_prefix("import-dep-if-present=") {
                    if !imports.contains(name) || !actual_imports.insert(name.to_owned()) {
                        return false;
                    }
                } else {
                    return false;
                }
                index += 2;
            }
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
    compile_dependencies && vstd == is_vstd && actual_imports == *imports
}

/// The initial closed graph has only vstd as a verified dependency. Cargo also
/// names a package's own lib when it has auxiliary targets; the lib invocation
/// has no self extern, so the pinned if-present route cannot import itself.
fn cargo_imports(metadata: &Value, package: &Value) -> Option<BTreeSet<String>> {
    let node = metadata["resolve"]["nodes"]
        .as_array()?
        .iter()
        .find(|node| node["id"] == package["id"])?;
    let mut imports = BTreeSet::new();
    for dependency in node["deps"].as_array()? {
        if !dependency["dep_kinds"]
            .as_array()?
            .iter()
            .any(|kind| kind["kind"].is_null())
        {
            continue;
        }
        let target = metadata["packages"]
            .as_array()?
            .iter()
            .find(|target| target["id"] == dependency["pkg"])?;
        if target
            .pointer("/metadata/verus/verify")
            .and_then(Value::as_bool)
            == Some(true)
        {
            imports.insert(dependency["name"].as_str()?.to_owned());
        }
    }
    let targets = package["targets"].as_array()?;
    if targets
        .iter()
        .any(|target| target["kind"] != serde_json::json!(["lib"]))
    {
        let library = targets
            .iter()
            .find(|target| target["kind"] == serde_json::json!(["lib"]))?;
        imports.insert(library["name"].as_str()?.to_owned());
    }
    Some(imports)
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
            value if cargo_diagnostic_lint(value) => {
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

// Cargo emits these long forms for manifest lints, including inherited Clippy
// lints during ordinary rustc builds. They neither select inputs nor change
// lookup. Every literal argument remains in the compiler/retained identity.
fn cargo_diagnostic_lint(argument: &str) -> bool {
    let Some(name) = ["--allow=", "--warn=", "--deny=", "--forbid="]
        .iter()
        .find_map(|prefix| argument.strip_prefix(prefix))
    else {
        return false;
    };
    let identifier = |part: &str| {
        part.as_bytes().split_first().is_some_and(|(first, rest)| {
            (first.is_ascii_alphabetic() || *first == b'_')
                && rest
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'-'))
        })
    };
    let mut parts = name.split("::");
    let Some(first) = parts.next() else {
        return false;
    };
    match parts.next() {
        None => identifier(first),
        Some(second) => first == "clippy" && identifier(second) && parts.next().is_none(),
    }
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
    fn shipped_coexistence_is_limited_to_the_selected_requested_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let caller = archive::CallerEnvironment::capture("run").ok_or("capture")?;
        let plan: Plan = serde_json::from_value(serde_json::json!({
            "root":"/owned", "target":"/owned/target", "metadata":null,
            "roots":["/project/Cargo.toml"], "closure":{"directories":{}},
            "launcher":"/pinned/verus", "launcher_sha256":"launcher",
            "compiler_sha256":"compiler", "caller_environment":caller
        }))?;
        let identity = |manifest: &str, name: &str| receipts::TargetIdentity {
            manifest: PathBuf::from(manifest),
            name: name.into(),
            crate_type: "lib".into(),
            rustc_args_sha256: "actual arguments".into(),
        };
        assert!(plan.requested(&identity("/project/Cargo.toml", "project")));
        assert!(!plan.requested(&identity("/registry/vstd/Cargo.toml", "vstd")));
        assert!(!plan.requested(&identity("/foreign/Cargo.toml", "project")));
        Ok(())
    }

    #[test]
    fn caller_environment_is_required_and_bound_to_the_actual_run()
    -> Result<(), Box<dyn std::error::Error>> {
        // Deserialization alone is not source admission; this isolates the
        // required per-run wire field from filesystem/provider eligibility.
        let caller = archive::CallerEnvironment::capture("current-run").ok_or("capture")?;
        let mut wire = serde_json::json!({"root":"/owned","target":"/owned/target",
            "metadata":null,"roots":[],"closure":{"directories":{}},
            "launcher":"/pinned/verus","launcher_sha256":"literal-launcher",
            "compiler_sha256":"literal-compiler","caller_environment":caller});
        let plan: Plan = serde_json::from_value(wire.clone())?;
        assert!(plan.caller_environment.for_run("current-run"));
        assert!(!plan.caller_environment.for_run("different-run"));
        wire.as_object_mut()
            .ok_or("object")?
            .remove("caller_environment");
        assert!(serde_json::from_value::<Plan>(wire).is_err());
        Ok(())
    }

    #[test]
    fn actual_cargo_manifest_lints_are_admitted_without_hiding_other_arguments()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/retained-cargo-lints.json"
        ))?;
        let mut args: Vec<OsString> = [
            "/rustc-hint",
            "--crate-name",
            "sample",
            "--crate-type",
            "lib",
            "src/lib.rs",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        for value in fixture["lint_arguments"]
            .as_array()
            .ok_or("lint arguments")?
        {
            let value = value.as_str().ok_or("lint argument")?;
            assert!(cargo_diagnostic_lint(value));
            args.push(value.into());
        }
        let target = Path::new("/owned/target");
        assert_eq!(ordinary_source(&args, target), Some("src/lib.rs"));
        // Repeated valid Rust flags are preserved, not removed or reordered.
        args.push("--warn=unexpected_cfgs".into());
        assert_eq!(ordinary_source(&args, target), Some("src/lib.rs"));
        for argument in [
            "--warn=",
            "--warn=clippy::",
            "--allow=foreign::lint",
            "--warn=clippy::a::b",
            "--warn=foo,bar",
            "--warn=foo bar",
            "--warn=@response",
            "--warn=--sysroot=x",
            "--force-warn=unexpected_cfgs",
            "-Aunexpected_cfgs",
            "--sysroot=/foreign",
        ] {
            assert!(!cargo_diagnostic_lint(argument));
            let mut changed = args.clone();
            changed.push(argument.into());
            assert!(ordinary_source(&changed, target).is_none(), "{argument}");
        }
        for flag in [
            "--allow=dead_code",
            "--warn=unexpected_cfgs",
            "--deny=unsafe_code",
            "--forbid=clippy::all",
        ] {
            assert!(cargo_diagnostic_lint(flag));
        }
        Ok(())
    }

    #[test]
    fn source_and_proof_mode_overrides_cannot_hide_in_cargo_effective_options() {
        assert!(!closed_verus_options(&[], false, &BTreeSet::new()));
        assert!(closed_verus_options(
            &[
                "--VIA-CARGO".into(),
                "compile-when-not-primary-package".into(),
                "--output-json".into(),
                "--rlimit".into(),
                "10".into()
            ],
            false,
            &BTreeSet::new()
        ));
        for option in [
            "--import",
            "--no-verify",
            "--verify-function",
            "--no-vstd",
            "--",
            "--compile",
        ] {
            assert!(!closed_verus_options(
                &[option.into()],
                false,
                &BTreeSet::new()
            ));
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
    fn pinned_cargo_routing_is_exact_and_tied_to_real_metadata_imports()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/retained-cargo-routing.json"
        ))?;
        for row in fixture["rows"].as_array().ok_or("rows")? {
            let tokens: Vec<String> = serde_json::from_value(row["tokens"].clone())?;
            let imports: BTreeSet<String> = serde_json::from_value(row["imports"].clone())?;
            let vstd = row["is_vstd"].as_bool().ok_or("vstd")?;
            assert!(closed_verus_options(&tokens, vstd, &imports));
            assert!(!closed_verus_options(&tokens, !vstd, &imports));
            for suffix in [
                vec!["--VIA-CARGO", "compile-when-not-primary-package"],
                vec!["--VIA-CARGO", "compile-when-primary-package"],
                vec!["--VIA-CARGO", "import-dep-if-present=foreign"],
                vec!["--VIA-CARGO", "import-dep-if-present=vstd"],
                vec!["--VIA-CARGO"],
                vec!["--"],
                vec!["--is-core"],
            ] {
                let mut changed = tokens.clone();
                changed.extend(suffix.into_iter().map(str::to_owned));
                assert!(!closed_verus_options(&changed, vstd, &imports));
            }
        }
        let metadata = &fixture["metadata"];
        for package in metadata["packages"].as_array().ok_or("packages")? {
            let expected: Option<BTreeSet<String>> = match package["name"].as_str() {
                Some("vest_lib") => Some(["vest_lib".into(), "vstd".into()].into()),
                Some("vstd") => Some(["vstd".into()].into()),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(cargo_imports(metadata, package), Some(expected));
            }
        }
        Ok(())
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

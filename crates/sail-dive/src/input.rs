//! Private immutable module closure and pre-binding host-capability admission.
mod pure_tlc;

use crate::{Model, Outcome};
use dive_api::dive_sema::{self, Origin};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) type Failure = (Outcome, String);

pub(crate) struct Frozen {
    directory: PathBuf,
    forbidden_labels: BTreeSet<String>,
    pub(crate) root: PathBuf,
    pub(crate) config: PathBuf,
    pub(crate) libraries: Vec<PathBuf>,
}

impl Drop for Frozen {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn rejected(message: impl std::fmt::Display) -> Failure {
    (Outcome::Rejected, format!("SAIL4201: {message}"))
}

fn relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./-".contains(&byte))
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

impl Frozen {
    pub(crate) fn create(model: &Model) -> Result<Self, Failure> {
        Self::prepare(model).map(|(frozen, _)| frozen)
    }

    fn prepare(model: &Model) -> Result<(Self, dive_sema::Analysis), Failure> {
        if model.files.is_empty() || model.files.len() > 1024 {
            return Err(rejected("model closure requires 1..1024 files"));
        }
        let mut names = BTreeSet::new();
        let mut modules = BTreeSet::new();
        let mut total = 0_usize;
        for (path, source) in &model.files {
            total = total.saturating_add(source.len());
            if !relative(path)
                || !names.insert(path.to_ascii_lowercase())
                || !(Path::new(&path).extension() == Some(std::ffi::OsStr::new("tla"))
                    || Path::new(&path).extension() == Some(std::ffi::OsStr::new("cfg")))
                || total > 8 * 1024 * 1024
            {
                return Err(rejected("invalid, conflicting or oversized source closure"));
            }
            if Path::new(&path).extension() == Some(std::ffi::OsStr::new("tla")) {
                let stem = Path::new(path).file_stem().and_then(|p| p.to_str());
                if !modules.insert(stem.map(str::to_ascii_lowercase)) {
                    return Err(rejected(
                        "module file names must be unique across the closure",
                    ));
                }
            }
        }
        if !model.files.contains_key(&model.root)
            || Path::new(&model.root).extension() != Some(std::ffi::OsStr::new("tla"))
            || !model.files.contains_key(&model.config)
            || Path::new(&model.config).extension() != Some(std::ffi::OsStr::new("cfg"))
        {
            return Err(rejected("root and configuration must name closure files"));
        }

        let directory = std::env::temp_dir().join(format!(
            "sail-dive-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(rejected)?;
        let mut frozen = Self {
            root: directory.join(&model.root),
            config: directory.join(&model.config),
            libraries: Vec::new(),
            forbidden_labels: BTreeSet::new(),
            directory,
        };
        let mut parents = BTreeSet::new();
        for (path, source) in &model.files {
            let full = frozen.directory.join(path);
            let parent = full
                .parent()
                .ok_or_else(|| rejected("missing source parent"))?;
            std::fs::create_dir_all(parent).map_err(rejected)?;
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&full)
                .map_err(rejected)?;
            std::io::Write::write_all(&mut output, source.as_bytes()).map_err(rejected)?;
            if Path::new(&path).extension() == Some(std::ffi::OsStr::new("tla")) {
                parents.insert(parent.to_path_buf());
            }
        }
        frozen.libraries = parents.into_iter().collect();
        let analysis = frozen.admit(model)?;
        Ok((frozen, analysis))
    }

    fn analysis(&self) -> Result<dive_sema::Analysis, Failure> {
        dive_sema::analyze_file(
            &self.root,
            &dive_sema::Options {
                search: dive_sema::SearchPath {
                    library_paths: self.libraries.clone(),
                    after_standard: Vec::new(),
                },
                ..Default::default()
            },
        )
        .map_err(rejected)
    }

    pub(crate) fn loaded_sources(model: &Model) -> Option<Vec<String>> {
        let (frozen, analysis) = Self::prepare(model).ok()?;
        analysis
            .files()
            .iter()
            .map(|file| match &file.origin {
                Origin::File(path) => Some(
                    path.strip_prefix(&frozen.directory)
                        .ok()?
                        .to_str()?
                        .to_owned(),
                ),
                Origin::Standard => Some(file.display.clone()),
                // An extracted monolith has a distinct logical source even when
                // the origin points at the same outer file.
                _ => None,
            })
            .collect()
    }

    pub(crate) fn admit_labels(&self, trace: &dive_api::trace::TraceDoc) -> Result<(), Failure> {
        if trace
            .labels
            .iter()
            .flatten()
            .any(|label| self.forbidden_labels.contains(&label.name))
        {
            return Err((
                Outcome::Unsupported,
                "SAIL4202: unqualified native trace label".into(),
            ));
        }
        Ok(())
    }

    fn admit(&mut self, model: &Model) -> Result<dive_sema::Analysis, Failure> {
        let analysis = self.analysis()?;
        let mut qualified_imports = false;
        for file in analysis.files() {
            match &file.origin {
                Origin::File(path) | Origin::Monolith(path) => {
                    let relative = path.strip_prefix(&self.directory).map_err(rejected)?;
                    let key = relative
                        .to_str()
                        .ok_or_else(|| rejected("non-UTF-8 input"))?;
                    if !model.files.contains_key(key)
                        || std::fs::read(path).map_err(rejected)? != model.files[key].as_bytes()
                    {
                        return Err(rejected("loaded module is outside the exact closure"));
                    }
                }
                Origin::Standard if file.display == "<standard>/TLC.tla" => {
                    qualified_imports = true
                }
                Origin::Standard => {
                    let stem = Path::new(&file.display)
                        .file_stem()
                        .and_then(|p| p.to_str());
                    if !matches!(
                        stem,
                        Some("Naturals" | "Integers" | "Sequences" | "FiniteSets" | "Bags")
                    ) {
                        return Err((
                            Outcome::Unsupported,
                            format!(
                                "SAIL4202: host-capable or unqualified standard module {}",
                                file.display
                            ),
                        ));
                    }
                }
                Origin::Bundled if pure_tlc::qualified_bundle(&file.display) => {
                    qualified_imports = true
                }
                Origin::Bundled => {
                    return Err((
                        Outcome::Unsupported,
                        format!("SAIL4202: unqualified bundled module {}", file.display),
                    ));
                }
                Origin::Text => return Err(rejected("unattributed module source")),
                // A module origin this build does not know is not admitted.
                _ => {
                    return Err((
                        Outcome::Unsupported,
                        format!("SAIL4202: unqualified module origin {}", file.display),
                    ));
                }
            }
        }
        if !analysis.accepted() {
            return Err(rejected(
                "module syntax, resolution or semantic analysis failed",
            ));
        }
        // Native overrides match module names even for caller-supplied files.
        // Origin checks alone therefore cannot exclude a user TLC module that
        // defines PrintT. Inspect the actual lowered overrides before binding.
        let ir = dive_ir::lower(&analysis)
            .map_err(|e| (Outcome::Unsupported, format!("SAIL4202: {e}")))?;
        if qualified_imports {
            let config = dive_api::parse_config_file(&self.config).map_err(rejected)?;
            self.forbidden_labels = pure_tlc::admit(&ir, &config)?;
        } else if ir.defs().iter().filter_map(|d| d.std).any(|op| {
            op.impure()
                || matches!(
                    op.support(),
                    dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                )
        }) || ir.nodes().iter().any(|node| {
            node.effects
                .intersects(dive_ir::Effects::IMPURE.union(dive_ir::Effects::LIVE_TLCGET))
        }) {
            return Err((
                Outcome::Unsupported,
                "SAIL4202: native host capabilities are not admitted".into(),
            ));
        }
        Ok(analysis)
    }
}

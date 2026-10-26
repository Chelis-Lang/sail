//! Private immutable module closure and pre-binding host-capability admission.
use crate::{Model, Outcome};
use dive_api::dive_sema::{self, Origin};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) type Failure = (Outcome, String);

pub(crate) struct Frozen {
    directory: PathBuf,
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
        frozen.admit(model)?;
        Ok(frozen)
    }

    fn admit(&self, model: &Model) -> Result<(), Failure> {
        let analysis = dive_sema::analyze_file(
            &self.root,
            &dive_sema::Options {
                search: dive_sema::SearchPath {
                    library_paths: self.libraries.clone(),
                    after_standard: Vec::new(),
                },
                ..Default::default()
            },
        )
        .map_err(rejected)?;
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
                Origin::Text => return Err(rejected("unattributed module source")),
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
        if ir.defs().iter().filter_map(|d| d.std).any(|op| {
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
        Ok(())
    }
}

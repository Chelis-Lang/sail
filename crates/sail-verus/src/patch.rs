//! Pointing a Cargo project's Verus dependencies at the pinned Verus source
//! (`patch_verus_deps`).
//!
//! Projects depend on `vstd`, `verus_builtin` and friends from crates.io (often
//! with an exact `=0.0.0-<date>` pin) or from Verus's git repository. To verify
//! them with the pinned release, those dependencies must resolve to the pinned
//! commit's sources instead. This mirrors verita's `inject_verus_patches`
//! (`tools/verita/src/dependencies.rs` at the pinned Verus commit):
//!
//! 1. map every package under `<verus>/source/*/Cargo.toml` to its directory;
//! 2. find the workspace root: the highest ancestor of the target, up to the
//!    repository root, whose `Cargo.toml` has a `[workspace]` table (else the
//!    target itself);
//! 3. collect the dependency names declared by the workspace root and the
//!    target manifests (`[dependencies]`, `[dev-dependencies]`,
//!    `[build-dependencies]`, `[workspace.dependencies]`);
//! 4. for each Verus package among them, add a path entry to `[patch.crates-io]`
//!    and to `[patch."https://github.com/verus-lang/verus.git"]` in the
//!    workspace root manifest;
//! 5. rewrite exact version pins (`=X`) on those packages in both manifests
//!    to the local version, so that Cargo actually applies the patches.
//!
//! Edits go through `toml_edit`, so everything else in the files is left
//! byte for byte. This only ever runs on a scratch copy of a project, never on
//! the fetched source.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The git source verita patches, besides crates.io.
pub const VERUS_GIT_URL: &str = "https://github.com/verus-lang/verus.git";

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn parse(text: &str) -> Option<toml::Table> {
    text.parse::<toml::Table>().ok()
}

/// Packages under `<verus>/source/`, by package name.
fn verus_crates(verus_repo: &Path) -> BTreeMap<String, PathBuf> {
    let mut map = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(verus_repo.join("source")) else {
        return map;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let name = read(&dir.join("Cargo.toml"))
            .and_then(|t| parse(&t))
            .and_then(|m| {
                m.get("package")?
                    .get("name")?
                    .as_str()
                    .map(ToOwned::to_owned)
            });
        if let Some(name) = name {
            map.insert(name, dir);
        }
    }
    map
}

/// The highest directory from `repo_root` down to `target` whose manifest has
/// a `[workspace]` table, else `target`.
fn workspace_root(target: &Path, repo_root: &Path) -> PathBuf {
    let mut chain = Vec::new();
    let mut current = Some(target);
    while let Some(dir) = current {
        chain.push(dir);
        if dir == repo_root {
            break;
        }
        current = dir.parent();
    }
    chain
        .iter()
        .rev()
        .find(|dir| {
            read(&dir.join("Cargo.toml"))
                .and_then(|t| parse(&t))
                .is_some_and(|m| m.contains_key("workspace"))
        })
        .map_or_else(|| target.to_path_buf(), |d| d.to_path_buf())
}

fn dependency_names(manifest: &toml::Table) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = manifest.get(section).and_then(toml::Value::as_table) {
            names.extend(deps.keys().cloned());
        }
    }
    if let Some(deps) = manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        names.extend(deps.keys().cloned());
    }
    names
}

fn package_version(dir: &Path) -> Option<String> {
    let manifest = parse(&read(&dir.join("Cargo.toml"))?)?;
    manifest
        .get("package")?
        .get("version")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// Rewrites `dep` if it is an exact pin (`"=X"` or `{ version = "=X" }`) to
/// another version. Returns whether it changed.
fn relax_item(dep: &mut toml_edit::Item, local: &str) -> bool {
    let item = if dep.is_str() {
        Some(dep)
    } else {
        dep.as_table_like_mut().and_then(|t| t.get_mut("version"))
    };
    let Some(value) = item.and_then(toml_edit::Item::as_value_mut) else {
        return false;
    };
    let exact = value
        .as_str()
        .and_then(|s| s.strip_prefix('='))
        .is_some_and(|v| v.trim_start_matches('=') != local);
    if !exact {
        return false;
    }
    // Keep surrounding whitespace and comments.
    let decor = value.decor().clone();
    *value = toml_edit::Value::from(format!("={local}"));
    *value.decor_mut() = decor;
    true
}

fn relax_pins(doc: &mut toml_edit::DocumentMut, local: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    let mut relax_in = |deps: &mut toml_edit::Table| {
        for (name, version) in local {
            if let Some(dep) = deps.get_mut(name) {
                changed |= relax_item(dep, version);
            }
        }
    };
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = doc.get_mut(section).and_then(toml_edit::Item::as_table_mut) {
            relax_in(deps);
        }
    }
    if let Some(deps) = doc
        .get_mut("workspace")
        .and_then(toml_edit::Item::as_table_mut)
        .and_then(|w| w.get_mut("dependencies"))
        .and_then(toml_edit::Item::as_table_mut)
    {
        relax_in(deps);
    }
    changed
}

fn load_doc(path: &Path) -> Result<toml_edit::DocumentMut> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Error::new("SAIL3016", format!("cannot read {}: {e}", path.display())))?;
    text.parse::<toml_edit::DocumentMut>()
        .map_err(|e| Error::new("SAIL3016", format!("cannot parse {}: {e}", path.display())))
}

fn write_doc(path: &Path, doc: &toml_edit::DocumentMut) -> Result<()> {
    std::fs::write(path, doc.to_string())
        .map_err(|e| Error::new("SAIL3016", format!("cannot write {}: {e}", path.display())))
}

/// What [`patch_verus_deps`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patched {
    /// The manifest that received the `[patch]` tables.
    pub manifest: Option<PathBuf>,
    /// The Verus packages patched, by name.
    pub crates: Vec<String>,
}

/// Points the Verus dependencies of the Cargo package in `target` (inside
/// the scratch repository `repo_root`) at the packages of `verus_repo`.
/// Does nothing if the project references no Verus package.
///
/// # Errors
///
/// `SAIL3016` if the workspace manifest cannot be read, parsed or written,
/// or its `[patch]` is not a table.
pub fn patch_verus_deps(target: &Path, repo_root: &Path, verus_repo: &Path) -> Result<Patched> {
    let crates = verus_crates(verus_repo);
    let root = workspace_root(target, repo_root);
    let root_manifest = root.join("Cargo.toml");
    let target_manifest = target.join("Cargo.toml");
    let mut names = BTreeSet::new();
    for path in [&root_manifest, &target_manifest] {
        if let Some(m) = read(path).and_then(|t| parse(&t)) {
            names.extend(dependency_names(&m));
        }
    }
    let patches: BTreeMap<&String, &PathBuf> =
        crates.iter().filter(|(n, _)| names.contains(*n)).collect();
    if patches.is_empty() {
        return Ok(Patched::default());
    }

    let mut doc = load_doc(&root_manifest)?;
    let not_table = || {
        Error::new(
            "SAIL3016",
            format!("[patch] in {} is not a table", root_manifest.display()),
        )
    };
    let patch = doc
        .entry("patch")
        .or_insert_with(|| {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            toml_edit::Item::Table(t)
        })
        .as_table_mut()
        .ok_or_else(not_table)?;
    for source in ["crates-io", VERUS_GIT_URL] {
        let table = patch
            .entry(source)
            .or_insert_with(toml_edit::table)
            .as_table_mut()
            .ok_or_else(not_table)?;
        for (name, path) in &patches {
            let mut entry = toml_edit::InlineTable::new();
            entry.insert(
                "path",
                toml_edit::Value::from(path.to_string_lossy().replace('\\', "/")),
            );
            table.insert(name, toml_edit::Item::Value(entry.into()));
        }
    }
    write_doc(&root_manifest, &doc)?;

    let local: BTreeMap<String, String> = patches
        .iter()
        .filter_map(|(n, p)| package_version(p).map(|v| ((*n).clone(), v)))
        .collect();
    if !local.is_empty() {
        let mut manifests = vec![root_manifest.clone()];
        if target_manifest != root_manifest && target_manifest.is_file() {
            manifests.push(target_manifest);
        }
        for path in manifests {
            let mut doc = load_doc(&path)?;
            if relax_pins(&mut doc, &local) {
                write_doc(&path, &doc)?;
            }
        }
    }
    Ok(Patched {
        manifest: Some(root_manifest),
        crates: patches.keys().map(|n| (*n).clone()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("sail-patch-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.0.join(rel);
            if let Some(parent) = path.parent() {
                assert!(std::fs::create_dir_all(parent).is_ok());
            }
            assert!(std::fs::write(path, text).is_ok());
        }

        fn read(&self, rel: &str) -> String {
            std::fs::read_to_string(self.0.join(rel)).unwrap_or_default()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn verus(s: &Scratch) -> PathBuf {
        s.write(
            "verus/source/vstd/Cargo.toml",
            "[package]\nname = \"vstd\"\nversion = \"0.0.0-new\"\n",
        );
        s.write(
            "verus/source/builtin/Cargo.toml",
            "[package]\nname = \"verus_builtin\"\nversion = \"0.0.0-b\"\n",
        );
        s.write("verus/source/air/Cargo.toml", "[package]\nname = \"air\"\n");
        s.0.join("verus")
    }

    #[test]
    fn patches_a_single_crate_and_relaxes_exact_pins() {
        let s = Scratch::new("single");
        let verus = verus(&s);
        let original = "# keep me\n[package]\nname = \"p\"\n\n[dependencies]\nvstd = \"=0.0.0-old\" # pinned\nserde = \"1\"\n";
        s.write("repo/p/Cargo.toml", original);
        let repo = s.0.join("repo");
        let got = patch_verus_deps(&repo.join("p"), &repo, &verus);
        assert_eq!(
            got.map(|p| p.crates),
            Ok(vec!["vstd".to_owned()]),
            "only referenced Verus crates are patched"
        );
        let text = s.read("repo/p/Cargo.toml");
        assert!(
            text.starts_with("# keep me\n[package]\nname = \"p\"\n"),
            "{text}"
        );
        assert!(text.contains("vstd = \"=0.0.0-new\" # pinned"), "{text}");
        assert!(text.contains("serde = \"1\""), "{text}");
        let vstd = verus.join("source/vstd").to_string_lossy().into_owned();
        assert!(
            text.contains(&format!(
                "[patch.crates-io]\nvstd = {{ path = \"{vstd}\" }}"
            )),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "[patch.\"https://github.com/verus-lang/verus.git\"]\nvstd = {{ path = \"{vstd}\" }}"
            )),
            "{text}"
        );
        assert!(!text.contains("air"), "{text}");
    }

    #[test]
    fn patches_the_outermost_workspace_and_both_manifests() {
        let s = Scratch::new("workspace");
        let verus = verus(&s);
        s.write(
            "repo/Cargo.toml",
            "[workspace]\nmembers = [\"a\"]\n\n[workspace.dependencies]\nverus_builtin = { version = \"=0.0.0-old\" }\n\n[patch.crates-io]\nfoo = { path = \"x\" }\n",
        );
        s.write(
            "repo/a/Cargo.toml",
            "[package]\nname = \"a\"\n\n[dependencies]\nvstd = { git = \"https://github.com/verus-lang/verus.git\" }\nverus_builtin = { workspace = true }\n",
        );
        let repo = s.0.join("repo");
        let got = patch_verus_deps(&repo.join("a"), &repo, &verus);
        assert_eq!(
            got.map(|p| p.crates),
            Ok(vec!["verus_builtin".to_owned(), "vstd".to_owned()])
        );
        let root = s.read("repo/Cargo.toml");
        assert!(root.contains("foo = { path = \"x\" }"), "{root}");
        assert!(
            root.contains("verus_builtin = { version = \"=0.0.0-b\" }"),
            "{root}"
        );
        assert_eq!(root.matches("[patch.crates-io]").count(), 1, "{root}");
        let member = s.read("repo/a/Cargo.toml");
        assert!(!member.contains("[patch"), "{member}");
    }

    #[test]
    fn leaves_projects_without_verus_dependencies_alone() {
        let s = Scratch::new("none");
        let verus = verus(&s);
        let original = "[package]\nname = \"p\"\n[dependencies]\nserde = \"1\"\n";
        s.write("repo/Cargo.toml", original);
        let repo = s.0.join("repo");
        assert_eq!(
            patch_verus_deps(&repo, &repo, &verus),
            Ok(Patched::default())
        );
        assert_eq!(s.read("repo/Cargo.toml"), original);
    }

    #[test]
    fn caret_and_matching_pins_are_not_rewritten() {
        let mut doc: toml_edit::DocumentMut =
            "[dependencies]\nvstd = \"0.0.0-old\"\nverus_builtin = \"=0.0.0-b\"\n"
                .parse()
                .unwrap_or_default();
        let local = BTreeMap::from([
            ("vstd".to_owned(), "0.0.0-new".to_owned()),
            ("verus_builtin".to_owned(), "0.0.0-b".to_owned()),
        ]);
        assert!(!relax_pins(&mut doc, &local));
    }
}

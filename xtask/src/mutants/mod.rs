//! `cargo xtask gen-mutants`: a seeded mutation catalogue for one corpus
//! unit (ADR 0063).
//!
//! The generator only reads source text: it finds every candidate site of
//! the nine classes in the files the crate root reaches through `mod`
//! declarations, orders the candidates by a SHA-256 hash of the seed and the
//! site, and takes up to `--per-class` of each class, at most one per
//! function first. It writes the catalogue in the `corpus/edits.toml` format
//! with `expected = "pending"`, and a JSON sidecar with each mutant's class,
//! function and location. The same seed, generator version and base commit
//! give byte-identical files. `sail-bench mutants oracle` then runs stock
//! Verus three times per mutant and settles `expected`.

mod lexer;
mod sites;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::Result;
use sites::{CLASSES, Site};

/// The generator's version, recorded in every sidecar. Bump it whenever
/// the same seed could produce different mutants.
pub const GENERATOR: &str = "gen-mutants 2";

const USAGE: &str =
    "usage: cargo xtask gen-mutants --project NAME [--unit UNIT] --seed N --per-class N
                       [--source DIR] [--name SHORT] [--out DIR]
       cargo xtask gen-mutants --file CRATE_ROOT --name SHORT --seed N --per-class N [--out DIR]

  --project   corpus project (corpus/corpus.toml)
  --unit      the unit to mutate when the project has several (its id, e.g.
              verified-storage/capybaraKV/capybarakv)
  --seed      the seed (an unsigned integer)
  --per-class mutants to take from each of the nine classes
  --source    the fetched source (default: the corpus cache, as
              `sail-conformance fetch` leaves it)
  --name      short name for ids and files (default: the project name)
  --out       output directory (default: corpus/mutants)
  --file      instead of a corpus unit, the crate rooted at this local file
              (its directory is the source; ids, unit and files use --name)";

struct Args {
    project: String,
    unit: Option<String>,
    seed: u64,
    per_class: usize,
    source: Option<PathBuf>,
    name: Option<String>,
    out: PathBuf,
    file: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut a = Args {
        project: String::new(),
        unit: None,
        seed: 0,
        per_class: 0,
        source: None,
        name: None,
        out: PathBuf::from("corpus/mutants"),
        file: None,
    };
    let mut seed = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--project" => a.project = value()?,
            "--unit" => a.unit = Some(value()?),
            "--seed" => {
                seed = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--seed needs an unsigned integer".to_owned())?,
                );
            }
            "--per-class" => {
                a.per_class = value()?
                    .parse()
                    .map_err(|_| "--per-class needs a number".to_owned())?;
            }
            "--source" => a.source = Some(value()?.into()),
            "--name" => a.name = Some(value()?),
            "--out" => a.out = value()?.into(),
            "--file" => a.file = Some(value()?.into()),
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    a.seed = seed.ok_or_else(|| format!("--seed is required\n{USAGE}"))?;
    let local_ok = a.file.is_some() && a.name.is_some() && a.project.is_empty();
    if a.per_class == 0 || !(local_ok || (a.file.is_none() && !a.project.is_empty())) {
        return Err(USAGE.to_owned());
    }
    Ok(a)
}

/// One selected mutant.
#[derive(Debug, Clone)]
pub struct Mutant {
    /// Catalogue id.
    pub id: String,
    /// File, relative to the project's source directory.
    pub file: String,
    /// 1-based line and column of the change.
    pub line: usize,
    /// Column.
    pub column: usize,
    /// The site.
    pub site: Site,
    /// Unique context in the original file.
    pub find: String,
    /// The same context, mutated.
    pub replace: String,
}

/// A deterministic 64-bit hash of the seed and `parts`.
fn hash(seed: u64, parts: &[&str]) -> u64 {
    let mut h = Sha256::new();
    h.update(seed.to_le_bytes());
    for p in parts {
        h.update(p.as_bytes());
        h.update([0]);
    }
    let d = h.finalize();
    let mut b = [0u8; 8];
    b.copy_from_slice(d.get(..8).unwrap_or(&[0; 8]));
    u64::from_le_bytes(b)
}

/// The files the crate root reaches through `mod name;` declarations,
/// relative to `source`, sorted.
fn crate_files(source: &Path, root: &Path) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(file) = todo.pop() {
        let Ok(rel) = file.strip_prefix(source) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if !seen.insert(rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned());
        let child_dir = match stem.as_deref() {
            Some("mod" | "lib" | "main") | None => dir.clone(),
            Some(s) => dir.join(s),
        };
        let toks = lexer::lex(&text);
        let tok_text = |i: usize| {
            toks.get(i)
                .and_then(|t| text.get(t.start..t.end))
                .unwrap_or_default()
        };
        for i in 0..toks.len() {
            if tok_text(i) != "mod" || tok_text(i + 2) != ";" {
                continue;
            }
            // `#[path = "..."]` directly before (possibly before `pub`).
            let mut j = i;
            if j > 0 && tok_text(j - 1) == "pub" {
                j -= 1;
            }
            let path_attr = (j >= 6
                && tok_text(j - 6) == "#"
                && tok_text(j - 4) == "path"
                && tok_text(j - 3) == "=")
                .then(|| tok_text(j - 2).trim_matches('"').to_owned());
            if let Some(p) = path_attr {
                todo.push(dir.join(p));
                continue;
            }
            let name = tok_text(i + 1).trim_start_matches("r#");
            let flat = child_dir.join(format!("{name}.rs"));
            let nested = child_dir.join(name).join("mod.rs");
            if flat.is_file() {
                todo.push(flat);
            } else if nested.is_file() {
                todo.push(nested);
            }
        }
    }
    seen.into_iter().collect()
}

/// Line and column (1-based) of byte `p`.
fn line_col(src: &str, p: usize) -> (usize, usize) {
    let before = src.get(..p).unwrap_or_default();
    let line = before.matches('\n').count() + 1;
    let col = before.rfind('\n').map_or(p, |q| p - q - 1) + 1;
    (line, col)
}

/// The smallest window of whole lines around `[s, e)` that occurs exactly
/// once in `src`, as `(window start, window end)`.
#[allow(
    clippy::many_single_char_names,
    reason = "s/e: the site, a/b: the window's first and last lines"
)]
fn unique_window(src: &str, s: usize, e: usize) -> Option<(usize, usize)> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(src.match_indices('\n').map(|(i, _)| i + 1))
        .filter(|&i| i <= src.len())
        .collect();
    let line_of = |p: usize| starts.partition_point(|&x| x <= p).saturating_sub(1);
    let (mut a, mut b) = (line_of(s), line_of(e.saturating_sub(1).max(s)));
    let end_of = |l: usize| starts.get(l + 1).copied().unwrap_or(src.len());
    for step in 0..120 {
        let ws = starts.get(a).copied().unwrap_or(0);
        let we = end_of(b);
        let w = src.get(ws..we).unwrap_or_default();
        if !w.trim().is_empty() && src.matches(w).count() == 1 {
            return Some((ws, we));
        }
        if step % 2 == 0 && b + 1 < starts.len() {
            b += 1;
        } else if a > 0 {
            a -= 1;
        } else if b + 1 < starts.len() {
            b += 1;
        } else {
            return None;
        }
    }
    None
}

/// Generates the mutants of one unit's files.
///
/// # Errors
///
/// A message if a file cannot be read.
pub fn generate(
    source: &Path,
    files: &[String],
    project: &str,
    name: &str,
    seed: u64,
    per_class: usize,
) -> Result<(Vec<Mutant>, BTreeMap<&'static str, usize>)> {
    let mut texts = BTreeMap::new();
    let mut candidates: Vec<(u64, String, Site)> = Vec::new();
    for file in files {
        let path = source.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let key =
            |label: &str, offset: usize| hash(seed, &[project, file, label, &offset.to_string()]);
        for site in sites::find(&text, &key) {
            let order = hash(
                seed,
                &[
                    project,
                    file,
                    site.class,
                    &site.start.to_string(),
                    &site.replacement,
                ],
            );
            candidates.push((order, file.clone(), site));
        }
        texts.insert(file.clone(), text);
    }
    let mut counts = BTreeMap::new();
    for (_, _, s) in &candidates {
        *counts.entry(s.class).or_insert(0) += 1;
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut out = Vec::new();
    for class in CLASSES {
        let mut taken: Vec<Mutant> = Vec::new();
        let mut used: BTreeSet<(String, String)> = BTreeSet::new();
        let mut results: BTreeSet<(String, String)> = BTreeSet::new();
        for spread in [true, false] {
            for (_, file, site) in candidates.iter().filter(|c| c.2.class == *class) {
                if taken.len() >= per_class {
                    break;
                }
                let fkey = (file.clone(), site.function.clone());
                if spread && used.contains(&fkey) {
                    continue;
                }
                let Some(text) = texts.get(file) else {
                    continue;
                };
                let Some((ws, we)) = unique_window(text, site.start, site.end) else {
                    continue;
                };
                let find = text.get(ws..we).unwrap_or_default().to_owned();
                let replace = format!(
                    "{}{}{}",
                    text.get(ws..site.start).unwrap_or_default(),
                    site.replacement,
                    text.get(site.end..we).unwrap_or_default()
                );
                if find == replace || !results.insert((file.clone(), replace.clone())) {
                    continue;
                }
                let (line, column) = line_col(text, site.start);
                used.insert(fkey);
                taken.push(Mutant {
                    id: String::new(),
                    file: file.clone(),
                    line,
                    column,
                    site: site.clone(),
                    find,
                    replace,
                });
            }
        }
        for (n, mut m) in taken.into_iter().enumerate() {
            m.id = format!("{name}-{class}-{:03}", n + 1);
            out.push(m);
        }
    }
    Ok((out, counts))
}

/// A TOML string: a multi-line literal string where possible.
fn toml_string(s: &str) -> String {
    if s.contains('\n') && !s.contains("'''") && !s.contains('\r') {
        format!("'''\n{s}'''")
    } else {
        serde_json::to_string(s).unwrap_or_default()
    }
}

/// The catalogue text.
#[must_use]
pub fn catalogue(mutants: &[Mutant], unit: &str, header: &str) -> String {
    let mut t = String::new();
    t.push_str(header);
    t.push_str("schema_version = 1\n");
    for m in mutants {
        let _ = write!(
            t,
            "\n[[edit]]\nid = {}\nunit = {}\nkind = {}\nfile = {}\ndescription = {}\nexpected = \"pending\"\nfind = {}\nreplace = {}\n",
            toml_string(&m.id),
            toml_string(unit),
            toml_string(m.site.kind),
            toml_string(&m.file),
            toml_string(&format!(
                "{}: {} in `{}`",
                m.site.class, m.site.what, m.site.function
            )),
            toml_string(&m.find),
            toml_string(&m.replace),
        );
    }
    t
}

/// What to mutate: a corpus unit, or a local crate root (`--file`).
struct Target {
    /// The directory the catalogue's file paths are relative to.
    source: PathBuf,
    /// The crate root.
    crate_root: PathBuf,
    /// The name hashed into the seed order.
    project: String,
    /// The catalogue's unit.
    unit: String,
    /// The pinned commit, or `local source`.
    commit: String,
    /// The arguments naming this target, for the header.
    invocation: String,
}

/// A corpus unit, as `sail-conformance fetch` leaves it (or `--source`).
fn corpus_target(a: &Args) -> Result<Target> {
    let root = crate::workspace_root();
    let corpus = sail_verus::corpus::Corpus::load(&root.join("corpus/corpus.toml"))
        .map_err(|e| e.to_string())?;
    let project = corpus
        .projects
        .iter()
        .find(|p| p.name == a.project)
        .ok_or_else(|| format!("no project `{}` in corpus/corpus.toml", a.project))?;
    let source = match &a.source {
        Some(s) => s.clone(),
        None => project.source_dir(&sail_verus::cache_dir().map_err(|e| e.to_string())?),
    };
    if !source.is_dir() {
        return Err(format!(
            "{} is not fetched (run `sail-conformance fetch --tier {} --project {}`)",
            source.display(),
            format!("{:?}", project.tier).to_lowercase(),
            project.name
        ));
    }
    let units = project.units(&source).map_err(|e| e.to_string())?;
    let unit = match &a.unit {
        Some(id) => units.iter().find(|u| &u.id == id),
        None if units.len() == 1 => units.first(),
        None => None,
    }
    .ok_or_else(|| {
        format!(
            "choose a unit with --unit: {}",
            units
                .iter()
                .map(|u| u.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let crate_root = if unit.build == sail_verus::corpus::Build::CargoVerus {
        let manifest = source.join(&unit.root).join("Cargo.toml");
        let lib = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|m| m.get("lib")?.get("path")?.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "src/lib.rs".to_owned());
        source.join(&unit.root).join(lib)
    } else {
        source.join(&unit.root)
    };
    Ok(Target {
        crate_root,
        project: a.project.clone(),
        unit: unit.id.clone(),
        commit: project.commit.clone(),
        invocation: format!(
            "--project {}{}",
            a.project,
            a.unit
                .as_ref()
                .map(|u| format!(" --unit {u}"))
                .unwrap_or_default()
        ),
        source,
    })
}

/// A local crate root outside the corpus (`--file`), named `name`.
fn local_target(file: &Path, name: &str) -> Result<Target> {
    if !file.is_file() {
        return Err(format!("{} is not a file", file.display()));
    }
    let source = file
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    Ok(Target {
        crate_root: file.to_path_buf(),
        source,
        project: name.to_owned(),
        unit: name.to_owned(),
        commit: "local source".to_owned(),
        invocation: format!("--file {}", file.display()),
    })
}

/// Entry point.
///
/// # Errors
///
/// A message for bad arguments, a missing source tree or unwritable output.
pub fn run(args: &[String]) -> Result<()> {
    let a = parse(args)?;
    let name = a.name.clone().unwrap_or_else(|| a.project.clone());
    let t = match &a.file {
        Some(file) => local_target(file, &name)?,
        None => corpus_target(&a)?,
    };
    let files = crate_files(&t.source, &t.crate_root);
    if files.is_empty() {
        return Err(format!(
            "no source files reached from {}",
            t.crate_root.display()
        ));
    }
    let (mutants, counts) = generate(&t.source, &files, &t.project, &name, a.seed, a.per_class)?;
    let oracle_note = if a.file.is_some() {
        "# `expected` is \"pending\" until its consumer settles it from stock Verus.\n"
    } else {
        "# `expected` is \"pending\" until `sail-bench mutants oracle` settles it from\n\
         # three stock Verus runs per mutant on the reference platform.\n"
    };
    let header = format!(
        "# Mutation catalogue for {} (ADR 0063), generated by `cargo xtask gen-mutants\n\
         # {} --name {name} --seed {} --per-class {}` ({GENERATOR}) at {}.\n{oracle_note}",
        t.unit, t.invocation, a.seed, a.per_class, t.commit,
    );
    let text = catalogue(&mutants, &t.unit, &header);
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let mut per_mutant = serde_json::Map::new();
    for m in &mutants {
        per_mutant.insert(
            m.id.clone(),
            serde_json::json!({
                "class": m.site.class,
                "function": m.site.function,
                "mode": m.site.mode,
                "kind": m.site.kind,
                "file": m.file,
                "line": m.line,
                "column": m.column,
            }),
        );
    }
    let mut taken = BTreeMap::new();
    for m in &mutants {
        *taken.entry(m.site.class).or_insert(0usize) += 1;
    }
    let meta = serde_json::json!({
        "schema_version": 1,
        "generator": GENERATOR,
        "project": t.project,
        "unit": t.unit,
        "base_commit": t.commit,
        "seed": a.seed,
        "per_class": a.per_class,
        "files": files.len(),
        "candidate_sites": counts,
        "generated": taken,
        "candidates_sha256": digest,
        "mutants": per_mutant,
        "oracle": serde_json::Value::Null,
    });
    let out = if a.out.is_absolute() {
        a.out.clone()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(&a.out)
    };
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let cat = out.join(format!("{name}.toml"));
    let side = out.join(format!("{name}.meta.json"));
    std::fs::write(&cat, &text).map_err(|e| format!("cannot write {}: {e}", cat.display()))?;
    let mut meta_text = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    meta_text.push('\n');
    std::fs::write(&side, meta_text)
        .map_err(|e| format!("cannot write {}: {e}", side.display()))?;
    println!(
        "{}: {} files, {} mutants ({}), sha256 {digest}\nwrote {} and {}",
        t.unit,
        files.len(),
        mutants.len(),
        CLASSES
            .iter()
            .map(|c| format!(
                "{c} {}/{}",
                taken.get(c).unwrap_or(&0),
                counts.get(c).unwrap_or(&0)
            ))
            .collect::<Vec<_>>()
            .join(", "),
        cat.display(),
        side.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_file_needs_a_name_and_no_project() {
        let args = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let ok = parse(&args("--file a/lib.rs --name smoke --seed 1 --per-class 1"));
        assert!(ok.is_ok_and(|a| a.file.is_some() && a.project.is_empty()));
        assert!(parse(&args("--file a/lib.rs --seed 1 --per-class 1")).is_err());
        assert!(
            parse(&args(
                "--file a/lib.rs --project p --name s --seed 1 --per-class 1"
            ))
            .is_err()
        );
        assert!(parse(&args("--project p --seed 1 --per-class 1")).is_ok());
    }

    #[test]
    fn windows_are_unique() {
        let src = "a\nb\nx\nb\nc\n";
        let s = src.find('x').unwrap_or(0);
        // "x\n" alone is unique.
        assert_eq!(unique_window(src, s, s + 1), Some((4, 6)));
        // The second "b\n" needs context.
        let second = src.rfind('b').unwrap_or(0);
        let w = unique_window(src, second, second + 1).unwrap_or((0, 0));
        let text = src.get(w.0..w.1).unwrap_or_default();
        assert_eq!(src.matches(text).count(), 1);
        assert!(text.contains("b\n"));
    }

    #[test]
    fn generation_is_deterministic() {
        let dir = std::env::temp_dir().join(format!("sail-gen-mutants-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("src"));
        let lib = "verus! {\nmod m;\nfn f(a: u64) -> (r: u64)\n    requires\n        a < 10,\n    ensures\n        r == a + 1,\n{\n    let x = a + 1;\n    assert(x > a);\n    x\n}\n}\n";
        let _ = std::fs::write(dir.join("src/lib.rs"), lib);
        let _ = std::fs::write(
            dir.join("src/m.rs"),
            "verus! {\nproof fn p(x: int) ensures x + 0 == x {}\n}\n",
        );
        let files = crate_files(&dir, &dir.join("src/lib.rs"));
        assert_eq!(files, ["src/lib.rs", "src/m.rs"]);
        let run = |seed| {
            generate(&dir, &files, "p", "p", seed, 3)
                .map(|(m, _)| catalogue(&m, "p/u", ""))
                .unwrap_or_default()
        };
        assert_eq!(run(7), run(7));
        assert!(run(7).contains("[[edit]]"));
        let parsed: std::result::Result<toml::Table, _> = run(7).parse();
        assert!(parsed.is_ok(), "{parsed:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

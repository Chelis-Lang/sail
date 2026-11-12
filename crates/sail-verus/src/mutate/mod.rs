//! Seeded source mutation of Verus crates (ADR 0063).
//!
//! This module is internal to the Sail workspace: `cargo xtask gen-mutants`
//! uses it to write the mutation catalogues in `corpus/mutants/`, and the
//! planned `sail mutate` campaigns will use its exec-only planner. Its API
//! carries no stability promise for other crates.
//!
//! The generator only reads source text. [`generate`] finds every candidate
//! site of the nine catalogue classes ([`CLASSES`]) in the files the crate
//! root reaches through `mod` declarations ([`crate_files`]), orders the
//! candidates by a SHA-256 hash of the seed and the site, and takes up to
//! `per_class` of each class, at most one per function first. [`render`]
//! writes the catalogue in the `corpus/edits.toml` format with
//! `expected = "pending"`, and a JSON sidecar with each mutant's class,
//! function and location. The same seed, generator version and base commit
//! give byte-identical files. `sail-bench mutants oracle` then runs stock
//! Verus three times per mutant and settles `expected`; [`settle`] replays
//! that rewrite from the recorded oracle so a committed catalogue can be
//! checked byte for byte against a fresh generation.
//!
//! [`exec::plan`] is the exec-only planner: the body classes restricted to
//! tokens the fail-closed exec mask admits, plus the
//! exec operators. Spec-mutating classes cannot be expressed in its class
//! type ([`exec::ExecClass`]).

pub mod exec;
mod lexer;
pub mod owned;
mod sites;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

pub use sites::{CLASSES, Site};

/// The generator's version, recorded in every sidecar. Bump it whenever
/// the same seed could produce different mutants.
///
/// Version 3 reads `spec(checked)`/`proof(axiom)` and mode attributes, and
/// places a function body by what follows its brace group, so a brace in a
/// `requires`/`ensures` clause (`match r { .. },`) is no longer taken for
/// the body.
pub const GENERATOR: &str = "gen-mutants 3";

/// One selected mutant.
#[derive(Debug, Clone)]
pub struct Mutant {
    /// Catalogue id.
    pub id: String,
    /// File, relative to the project's source directory.
    pub file: String,
    /// 1-based line of the change.
    pub line: usize,
    /// 1-based column of the change.
    pub column: usize,
    /// The site.
    pub site: Site,
    /// Unique context in the original file.
    pub find: String,
    /// The same context, mutated.
    pub replace: String,
}

/// A deterministic 64-bit hash of the seed and `parts`.
#[must_use]
pub fn hash(seed: u64, parts: &[&str]) -> u64 {
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
#[must_use]
pub fn crate_files(source: &Path, root: &Path) -> Vec<String> {
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
#[must_use]
pub fn line_col(src: &str, p: usize) -> (usize, usize) {
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
#[must_use]
pub fn unique_window(src: &str, s: usize, e: usize) -> Option<(usize, usize)> {
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

/// Generates the mutants of one unit's files, with the candidate count of
/// every class.
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
) -> Result<(Vec<Mutant>, BTreeMap<&'static str, usize>), String> {
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

/// One catalogue entry's text, `expected` as given.
fn entry(m: &Mutant, unit: &str, expected: &str) -> String {
    format!(
        "\n[[edit]]\nid = {}\nunit = {}\nkind = {}\nfile = {}\ndescription = {}\nexpected = {}\nfind = {}\nreplace = {}\n",
        toml_string(&m.id),
        toml_string(unit),
        toml_string(m.site.kind),
        toml_string(&m.file),
        toml_string(&format!(
            "{}: {} in `{}`",
            m.site.class, m.site.what, m.site.function
        )),
        toml_string(expected),
        toml_string(&m.find),
        toml_string(&m.replace),
    )
}

/// The catalogue text, every entry `expected = "pending"`.
#[must_use]
pub fn catalogue(mutants: &[Mutant], unit: &str, header: &str) -> String {
    let mut t = String::new();
    t.push_str(header);
    t.push_str("schema_version = 1\n");
    for m in mutants {
        t.push_str(&entry(m, unit, "pending"));
    }
    t
}

/// What one catalogue is generated from.
#[derive(Debug, Clone)]
pub struct Request {
    /// The directory the catalogue's file paths are relative to.
    pub source: PathBuf,
    /// The crate root.
    pub crate_root: PathBuf,
    /// The name hashed into the seed order (the corpus project's name).
    pub project: String,
    /// The catalogue's unit.
    pub unit: String,
    /// The pinned commit, or `local source`.
    pub commit: String,
    /// The `gen-mutants` arguments naming the target, for the header.
    pub invocation: String,
    /// Short name for ids and files.
    pub name: String,
    /// The seed.
    pub seed: u64,
    /// Mutants taken from each class.
    pub per_class: usize,
    /// A local crate (`--file`) rather than a corpus unit: its oracle is
    /// its consumer, not `sail-bench mutants oracle`.
    pub local: bool,
}

/// A generated catalogue and its sidecar.
#[derive(Debug, Clone)]
pub struct Generated {
    /// The catalogue text (`<name>.toml`).
    pub text: String,
    /// The sidecar (`<name>.meta.json`), oracle unset.
    pub meta: serde_json::Value,
    /// The selected mutants.
    pub mutants: Vec<Mutant>,
    /// Candidate sites per class.
    pub counts: BTreeMap<&'static str, usize>,
    /// Mutants taken per class.
    pub taken: BTreeMap<&'static str, usize>,
    /// The files read.
    pub files: Vec<String>,
    /// SHA-256 of `text`, hex.
    pub digest: String,
}

/// The sidecar's text for `meta`: pretty JSON with a final newline.
///
/// # Errors
///
/// A message if `meta` cannot be serialized.
pub fn meta_text(meta: &serde_json::Value) -> Result<String, String> {
    let mut t = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    t.push('\n');
    Ok(t)
}

/// Generates one catalogue and its sidecar.
///
/// # Errors
///
/// A message if no source file is reached from the crate root or a file
/// cannot be read.
pub fn render(r: &Request) -> Result<Generated, String> {
    let files = crate_files(&r.source, &r.crate_root);
    if files.is_empty() {
        return Err(format!(
            "no source files reached from {}",
            r.crate_root.display()
        ));
    }
    let (mutants, counts) = generate(&r.source, &files, &r.project, &r.name, r.seed, r.per_class)?;
    let oracle_note = if r.local {
        "# `expected` is \"pending\" until its consumer settles it from stock Verus.\n"
    } else {
        "# `expected` is \"pending\" until `sail-bench mutants oracle` settles it from\n\
         # three stock Verus runs per mutant on the reference platform.\n"
    };
    let header = format!(
        "# Mutation catalogue for {} (ADR 0063), generated by `cargo xtask gen-mutants\n\
         # {} --name {} --seed {} --per-class {}` ({GENERATOR}) at {}.\n{oracle_note}",
        r.unit, r.invocation, r.name, r.seed, r.per_class, r.commit,
    );
    let text = catalogue(&mutants, &r.unit, &header);
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
        "project": r.project,
        "unit": r.unit,
        "base_commit": r.commit,
        "seed": r.seed,
        "per_class": r.per_class,
        "files": files.len(),
        "candidate_sites": counts,
        "generated": taken,
        "candidates_sha256": digest,
        "mutants": per_mutant,
        "oracle": serde_json::Value::Null,
    });
    Ok(Generated {
        text,
        meta,
        mutants,
        counts,
        taken,
        files,
        digest,
    })
}

/// The header lines `sail-bench mutants oracle` writes: the generator's
/// comment lines without the "pending" note, then the oracle's summary.
#[must_use]
pub fn oracle_header(
    generated_header: &str,
    runs: usize,
    platform: &str,
    stock_verus: &str,
    kept: usize,
    dropped: usize,
) -> String {
    let mut header = generated_header.to_owned();
    let _ = write!(
        header,
        "# Oracle: {runs} stock runs per mutant on {platform} (Verus {stock_verus}); {kept} kept, {dropped} dropped\n# (reasons in the sidecar).\n",
    );
    let mut out = String::new();
    for l in header.lines().filter(|l| {
        !l.contains("is \"pending\" until")
            && !l.contains("three stock Verus runs per mutant on the reference")
    }) {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// The comment lines at the top of a catalogue.
#[must_use]
pub fn header_of(text: &str) -> String {
    let mut out = String::new();
    for l in text.lines().take_while(|l| l.starts_with('#')) {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// The catalogue `sail-bench mutants oracle` leaves after settling `g`
/// with the oracle recorded in `meta` (a committed sidecar): dropped
/// mutants removed, every other `expected` set to its first stock outcome.
///
/// # Errors
///
/// A message if `meta` has no oracle, or the oracle does not cover exactly
/// the generated mutants.
pub fn settle(g: &Generated, unit: &str, meta: &serde_json::Value) -> Result<String, String> {
    let oracle = meta
        .get("oracle")
        .filter(|o| o.is_object())
        .ok_or("the sidecar records no oracle")?;
    let results = oracle
        .get("results")
        .and_then(serde_json::Value::as_object)
        .ok_or("the oracle records no results")?;
    let dropped = oracle
        .get("dropped")
        .and_then(serde_json::Value::as_object)
        .ok_or("the oracle records no dropped list")?;
    let ids: BTreeSet<&str> = g.mutants.iter().map(|m| m.id.as_str()).collect();
    let covered: BTreeSet<&str> = results
        .keys()
        .map(String::as_str)
        .chain(dropped.keys().map(String::as_str))
        .collect();
    if ids != covered {
        let missing: Vec<&&str> = ids.difference(&covered).collect();
        let extra: Vec<&&str> = covered.difference(&ids).collect();
        return Err(format!(
            "the oracle does not cover the generated mutants (not settled: {missing:?}; not generated: {extra:?})"
        ));
    }
    let str_of = |k: &str| oracle.get(k).and_then(serde_json::Value::as_str);
    let runs = oracle
        .get("runs")
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("the oracle records no run count")?;
    let mut body = String::new();
    let mut kept = 0usize;
    for m in &g.mutants {
        if dropped.contains_key(&m.id) {
            continue;
        }
        let expected = results
            .get(&m.id)
            .and_then(|r| r.get("outcomes"))
            .and_then(|o| o.get(0))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("no stock outcome recorded for {}", m.id))?;
        body.push_str(&entry(m, unit, expected));
        kept += 1;
    }
    let mut t = oracle_header(
        &header_of(&g.text),
        runs,
        str_of("platform").ok_or("the oracle records no platform")?,
        str_of("stock_verus").ok_or("the oracle records no Verus release")?,
        kept,
        dropped.len(),
    );
    t.push_str("schema_version = 1\n");
    t.push_str(&body);
    Ok(t)
}

#[cfg(test)]
mod tests;

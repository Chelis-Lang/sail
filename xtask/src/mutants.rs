//! `cargo xtask gen-mutants`: a seeded mutation catalogue for one corpus
//! unit (ADR 0063), and `--check`, which regenerates every committed
//! catalogue and compares it byte for byte.
//!
//! The generator itself is `sail_verus::mutate`; this module parses the
//! command line, finds the unit's source and writes the files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sail_verus::mutate::{self, CLASSES, Request};

use crate::Result;

const USAGE: &str =
    "usage: cargo xtask gen-mutants --project NAME [--unit UNIT] --seed N --per-class N
                       [--source DIR] [--name SHORT] [--out DIR]
       cargo xtask gen-mutants --file CRATE_ROOT --name SHORT --seed N --per-class N [--out DIR]
       cargo xtask gen-mutants --check [--dir DIR] [--source NAME=DIR ...]

  --project   corpus project (corpus/corpus.toml)
  --unit      the unit to mutate when the project has several (its id, e.g.
              verified-storage/capybaraKV/capybarakv)
  --seed      the seed (an unsigned integer)
  --per-class mutants to take from each of the nine classes
  --source    the fetched source (default: the corpus cache, as
              `sail-conformance fetch` leaves it); with --check, NAME=DIR
              for the catalogue NAME
  --name      short name for ids and files (default: the project name)
  --out       output directory (default: corpus/mutants)
  --file      instead of a corpus unit, the crate rooted at this local file
              (its directory is the source; ids, unit and files use --name)
  --check     regenerate every catalogue in --dir (default: corpus/mutants)
              from its sidecar, replay its recorded oracle, and fail unless
              the result equals the committed catalogue byte for byte";

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

/// A corpus unit, as `sail-conformance fetch` leaves it (or `source`).
/// `unit` names the unit; without it the project must have one. The
/// header's invocation names the unit when `unit` is given and either
/// `always_name` is set (it was on the command line) or the project has
/// several units. The request's name, seed and count are left for the
/// caller.
fn corpus_request(
    project_name: &str,
    unit: Option<&str>,
    always_name: bool,
    source: Option<&Path>,
) -> Result<Request> {
    let root = crate::workspace_root();
    let corpus = sail_verus::corpus::Corpus::load(&root.join("corpus/corpus.toml"))
        .map_err(|e| e.to_string())?;
    let project = corpus
        .projects
        .iter()
        .find(|p| p.name == project_name)
        .ok_or_else(|| format!("no project `{project_name}` in corpus/corpus.toml"))?;
    let source = match source {
        Some(s) => s.to_path_buf(),
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
    let found = match unit {
        Some(id) => units.iter().find(|u| u.id == id),
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
    let crate_root = if found.build == sail_verus::corpus::Build::CargoVerus {
        let manifest = source.join(&found.root).join("Cargo.toml");
        let lib = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|m| m.get("lib")?.get("path")?.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "src/lib.rs".to_owned());
        source.join(&found.root).join(lib)
    } else {
        source.join(&found.root)
    };
    let named = always_name || units.len() > 1;
    Ok(Request {
        crate_root,
        project: project_name.to_owned(),
        unit: found.id.clone(),
        commit: project.commit.clone(),
        invocation: format!(
            "--project {project_name}{}",
            unit.filter(|_| named)
                .map(|u| format!(" --unit {u}"))
                .unwrap_or_default()
        ),
        source,
        name: String::new(),
        seed: 0,
        per_class: 0,
        local: false,
    })
}

/// A local crate root outside the corpus (`--file`), named `name`.
fn local_request(file: &Path, name: &str) -> Result<Request> {
    if !file.is_file() {
        return Err(format!("{} is not a file", file.display()));
    }
    let source = file
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    Ok(Request {
        crate_root: file.to_path_buf(),
        source,
        project: name.to_owned(),
        unit: name.to_owned(),
        commit: "local source".to_owned(),
        invocation: format!("--file {}", file.display()),
        name: name.to_owned(),
        seed: 0,
        per_class: 0,
        local: true,
    })
}

/// An absolute form of `p`, relative to the current directory.
fn absolute(p: &Path) -> Result<PathBuf> {
    if p.is_absolute() {
        Ok(p.to_path_buf())
    } else {
        Ok(std::env::current_dir().map_err(|e| e.to_string())?.join(p))
    }
}

/// Entry point.
///
/// # Errors
///
/// A message for bad arguments, a missing source tree, unwritable output,
/// or (with `--check`) a catalogue that does not regenerate identically.
pub fn run(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) == Some("--check") {
        return check(args.get(1..).unwrap_or_default());
    }
    let a = parse(args)?;
    let name = a.name.clone().unwrap_or_else(|| a.project.clone());
    let mut req = match &a.file {
        Some(file) => local_request(file, &name)?,
        None => corpus_request(&a.project, a.unit.as_deref(), true, a.source.as_deref())?,
    };
    req.name.clone_from(&name);
    req.seed = a.seed;
    req.per_class = a.per_class;
    let g = mutate::render(&req)?;
    let out = absolute(&a.out)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let cat = out.join(format!("{name}.toml"));
    let side = out.join(format!("{name}.meta.json"));
    std::fs::write(&cat, &g.text).map_err(|e| format!("cannot write {}: {e}", cat.display()))?;
    std::fs::write(&side, mutate::meta_text(&g.meta)?)
        .map_err(|e| format!("cannot write {}: {e}", side.display()))?;
    println!(
        "{}: {} files, {} mutants ({}), sha256 {}\nwrote {} and {}",
        req.unit,
        g.files.len(),
        g.mutants.len(),
        CLASSES
            .iter()
            .map(|c| format!(
                "{c} {}/{}",
                g.taken.get(c).unwrap_or(&0),
                g.counts.get(c).unwrap_or(&0)
            ))
            .collect::<Vec<_>>()
            .join(", "),
        g.digest,
        cat.display(),
        side.display()
    );
    Ok(())
}

/// The sidecar fields the generator writes (everything but the oracle).
fn generated_fields(meta: &serde_json::Value) -> serde_json::Value {
    let mut m = meta.clone();
    if let Some(o) = m.as_object_mut() {
        o.remove("oracle");
    }
    m
}

/// `--check`: regenerates every catalogue in the directory from its
/// sidecar's project, unit, seed and count, replays the recorded oracle,
/// and requires the committed catalogue's bytes and the sidecar's
/// generator fields to match.
fn check(args: &[String]) -> Result<()> {
    let mut dir = crate::workspace_root().join("corpus/mutants");
    let mut sources: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--dir" => dir = absolute(Path::new(&value()?))?,
            "--source" => {
                let v = value()?;
                let (n, d) = v
                    .split_once('=')
                    .ok_or_else(|| format!("--source with --check takes NAME=DIR\n{USAGE}"))?;
                sources.insert(n.to_owned(), absolute(Path::new(d))?);
            }
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .filter_map(|e| {
            let n = e.ok()?.file_name().to_string_lossy().into_owned();
            n.strip_suffix(".meta.json").map(ToOwned::to_owned)
        })
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("no catalogue sidecars in {}", dir.display()));
    }
    let mut failures = Vec::new();
    for name in &names {
        match check_one(&dir, name, sources.get(name).map(PathBuf::as_path)) {
            Ok(()) => println!("{name}: identical"),
            Err(e) => {
                println!("{name}: DIFFERENT: {e}");
                failures.push(name.clone());
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} of {} catalogues do not regenerate identically: {}",
            failures.len(),
            names.len(),
            failures.join(", ")
        ))
    }
}

fn check_one(dir: &Path, name: &str, source: Option<&Path>) -> Result<()> {
    let side = dir.join(format!("{name}.meta.json"));
    let cat = dir.join(format!("{name}.toml"));
    let meta: serde_json::Value = std::fs::read_to_string(&side)
        .map_err(|e| format!("cannot read {}: {e}", side.display()))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| format!("{}: {e}", side.display())))?;
    let committed =
        std::fs::read_to_string(&cat).map_err(|e| format!("cannot read {}: {e}", cat.display()))?;
    let field = |k: &str| {
        meta.get(k)
            .ok_or_else(|| format!("{} has no `{k}`", side.display()))
    };
    let project = field("project")?.as_str().unwrap_or_default().to_owned();
    let unit = field("unit")?.as_str().unwrap_or_default().to_owned();
    let seed = field("seed")?.as_u64().ok_or("`seed` is not a number")?;
    let per_class = field("per_class")?
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("`per_class` is not a number")?;
    let mut req = corpus_request(&project, Some(&unit), false, source)?;
    if field("base_commit")?.as_str() != Some(req.commit.as_str()) {
        return Err(format!(
            "the sidecar's base commit is not the corpus pin {}",
            req.commit
        ));
    }
    name.clone_into(&mut req.name);
    req.seed = seed;
    req.per_class = per_class;
    let g = mutate::render(&req)?;
    if generated_fields(&g.meta) != generated_fields(&meta) {
        return Err(format!(
            "the sidecar's generator fields differ from a fresh generation (sha256 {} vs {})",
            g.digest,
            meta.get("candidates_sha256")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?")
        ));
    }
    let settled = mutate::settle(&g, &req.unit, &meta)?;
    if settled != committed {
        let line = settled
            .lines()
            .zip(committed.lines())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| settled.lines().count().min(committed.lines().count()));
        return Err(format!(
            "{} differs from the regenerated, settled catalogue from line {}",
            cat.display(),
            line + 1
        ));
    }
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
}

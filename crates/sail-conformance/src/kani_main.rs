//! `sail-conformance kani`: Kani conformance and the fault arm (ADR 0183).
use std::process::ExitCode;

pub(crate) const USAGE: &str = "usage:
  sail-conformance kani run   [--tier fast|standard] [--project NAME]... [--filter SUBSTR]
                              [--rounds N] [--direct-runs N] [--jobs N] [--out DIR]
                              [--cache DIR] [--sail PATH] [--timeout-ms N] [--keep-targets]
                              [--corpus PATH]
  sail-conformance kani fault [--fixture DIR] [--memory-max SIZE] [--out DIR]
                              [--sail PATH] [--corpus PATH]

  run    compares `sail verify --backend kani` with the original Kani driver,
         run without Sail at least twice per harness (default 2), on the
         pinned corpus. Exits 0 when no harness is `different` or an
         infrastructure error, 1 otherwise, 2 on usage or setup errors.
  fault  runs the fault arm on the fixture crate (default
         crates/sail-conformance/tests/fixtures/kani-fault; memory cap default
         1536M). Exits 0 when the control row matches direct Kani and no fault
         gave a Sail claim or a state entry, 1 otherwise.

  --cache DIR   project checkouts and the prepared Cargo home
                (default $SAIL_CACHE_DIR/kani-corpus, else ~/.cache/sail/kani-corpus)
  --sail PATH   the `sail` binary (default: next to this binary)
  Linux x86-64 only.";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod imp {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;
    use std::time::Duration;

    use sail_conformance::kani::corpus::{Project, Tier};
    use sail_conformance::kani::runner::{self, Cli, Direct, Options, Slot};
    use sail_conformance::kani::{Error, fault};

    struct Args {
        command: String,
        corpus: PathBuf,
        tier: Tier,
        projects: Vec<String>,
        filter: Option<String>,
        fixture_dir: Option<String>,
        rounds: usize,
        direct_runs: usize,
        jobs: usize,
        out: PathBuf,
        cache: Option<PathBuf>,
        sail: Option<PathBuf>,
        timeout_ms: u64,
        keep_targets: bool,
        memory_max: String,
    }

    fn parse(args: &[String]) -> Result<Args, String> {
        let mut it = args.iter();
        let mut a = Args {
            command: it.next().cloned().ok_or(super::USAGE)?,
            corpus: PathBuf::from("corpus/kani-corpus.toml"),
            tier: Tier::Fast,
            projects: Vec::new(),
            filter: None,
            fixture_dir: None,
            rounds: 1,
            direct_runs: 2,
            jobs: 1,
            out: PathBuf::from("kani-conformance-out"),
            cache: None,
            sail: None,
            timeout_ms: 600_000,
            keep_targets: false,
            memory_max: "1536M".into(),
        };
        while let Some(flag) = it.next() {
            let mut value = || {
                it.next()
                    .cloned()
                    .ok_or(format!("{flag} needs a value\n{}", super::USAGE))
            };
            let number = |v: String| v.parse::<usize>().map_err(|e| format!("{flag}: {e}"));
            match flag.as_str() {
                "--corpus" => a.corpus = value()?.into(),
                "--tier" => a.tier = value()?.parse().map_err(|e: Error| e.to_string())?,
                "--project" => a.projects.push(value()?),
                "--filter" => a.filter = Some(value()?),
                "--fixture" => a.fixture_dir = Some(value()?),
                "--rounds" => a.rounds = number(value()?)?.max(1),
                "--direct-runs" => a.direct_runs = number(value()?)?,
                "--jobs" => a.jobs = number(value()?)?.max(1),
                "--out" => a.out = value()?.into(),
                "--cache" => a.cache = Some(value()?.into()),
                "--sail" => a.sail = Some(value()?.into()),
                "--timeout-ms" => {
                    a.timeout_ms = value()?.parse().map_err(|e| format!("--timeout-ms: {e}"))?;
                }
                "--memory-max" => a.memory_max = value()?,
                "--keep-targets" => a.keep_targets = true,
                _ => return Err(format!("unknown option {flag}\n{}", super::USAGE)),
            }
        }
        if a.direct_runs < 2 {
            return Err("--direct-runs must be at least 2 (ADR 0181)".into());
        }
        Ok(a)
    }

    fn sail_cache() -> Result<PathBuf, String> {
        sail_verus::cache_dir().map_err(|e| e.to_string())
    }

    fn setup(a: &Args) -> Result<(Direct, Cli, PathBuf, PathBuf), String> {
        let cache = sail_cache()?;
        let kani_root = cache.join("kani");
        let sail = match &a.sail {
            Some(p) => p.clone(),
            None => std::env::current_exe()
                .map_err(|e| e.to_string())?
                .with_file_name("sail"),
        };
        let stage_path = kani_root.join("stage-0.68.0-linux-x86_64");
        if !stage_path.exists() {
            bootstrap_stage(&sail, &cache)?;
        }
        let stage = stage_path.canonicalize().map_err(|e| {
            format!("Sail's Kani stage: {e}; run `sail engine install --backend kani` first")
        })?;
        let mut environment = BTreeMap::new();
        for key in ["HOME", "PATH", "SAIL_CACHE_DIR", "XDG_CACHE_HOME"] {
            if let Ok(v) = std::env::var(key) {
                environment.insert(key.to_owned(), v);
            }
        }
        let corpus_cache = a.cache.clone().unwrap_or_else(|| cache.join("kani-corpus"));
        std::fs::create_dir_all(&corpus_cache).map_err(|e| e.to_string())?;
        Ok((
            Direct {
                stage,
                deadline: Duration::from_millis(a.timeout_ms.saturating_mul(2).max(60_000)),
            },
            Cli {
                sail,
                kani_root: kani_root.canonicalize().map_err(|e| e.to_string())?,
                environment,
                timeout_ms: a.timeout_ms,
            },
            corpus_cache.canonicalize().map_err(|e| e.to_string())?,
            cache,
        ))
    }

    /// Sail stages its tools on its first check. A check of an empty crate
    /// with an empty Cargo home stages them and then fails, as intended.
    fn bootstrap_stage(sail: &Path, cache: &Path) -> Result<(), String> {
        let dir = cache.join("kani-corpus/stage-bootstrap");
        std::fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(dir.join("cargo-home")).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"bootstrap\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("src/lib.rs"), "").map_err(|e| e.to_string())?;
        let status = std::process::Command::new(sail)
            .args([
                "verify",
                "--backend",
                "kani",
                "--harness",
                "none",
                "--unwind",
                "1",
                "--project",
            ])
            .arg(&dir)
            .arg("--cargo-home")
            .arg(dir.join("cargo-home"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("{}: {e}", sail.display()))?;
        eprintln!("kani conformance: staged Sail's Kani tools (bootstrap check exit {status})");
        Ok(())
    }

    fn repo_root(corpus: &Path) -> PathBuf {
        corpus
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().and_then(Path::parent).map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub(super) fn main(args: &[String]) -> Result<ExitCode, String> {
        let a = parse(args)?;
        let (direct, cli, corpus_cache, sail_cache) = setup(&a)?;
        match a.command.as_str() {
            "run" => {
                let options = Options {
                    corpus: a.corpus.clone(),
                    repo_root: repo_root(&a.corpus),
                    tier: a.tier,
                    projects: a.projects.clone(),
                    filter: a.filter.clone(),
                    rounds: a.rounds,
                    direct_runs: a.direct_runs,
                    jobs: a.jobs,
                    cache: corpus_cache,
                    out: a.out.clone(),
                    keep_targets: a.keep_targets,
                };
                let report = runner::run(&options, &direct, &cli).map_err(|e| e.to_string())?;
                print!("{}", runner::summary(&report));
                Ok(if report.passed() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                })
            }
            "fault" => fault_arm(&a, &direct, &cli, &sail_cache),
            _ => Err(super::USAGE.to_owned()),
        }
    }
    fn fault_arm(
        a: &Args,
        direct: &Direct,
        cli: &Cli,
        sail_cache: &Path,
    ) -> Result<ExitCode, String> {
        let fixture = a.fixture_dir.clone().map_or_else(
            || repo_root(&a.corpus).join("crates/sail-conformance/tests/fixtures/kani-fault"),
            PathBuf::from,
        );
        let package = fixture
            .canonicalize()
            .map_err(|e| format!("{}: {e}", fixture.display()))?;
        let project = Project {
            name: "kani-fault".into(),
            url: String::new(),
            commit: String::new(),
            package: ".".into(),
            tier: Tier::Fast,
            license: "Apache-2.0".into(),
            ci_args: Vec::new(),
            default_unwind: 1,
            lock: None,
            lock_sha256: None,
            host_requires: Vec::new(),
            own_kani_pin: None,
            expected: None,
        };
        std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
        let out = a.out.canonicalize().map_err(|e| e.to_string())?;
        let home = out.join("cargo-home");
        std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
        let slot = Slot {
            cargo_home: home.canonicalize().map_err(|e| e.to_string())?,
        };
        let harnesses = runner::discover(direct, &project, &package, &slot, &out.join("discover"))
            .map_err(|e| e.to_string())?;
        let find = |name: &str| {
            harnesses
                .iter()
                .find(|h| h.name == name)
                .ok_or(format!("fixture has no harness {name}"))
        };
        let rows = fault::run(
            &fault::Setup {
                direct,
                sail: cli,
                sail_cache,
                slot: &slot,
                scratch: &out.join("scratch"),
                memory_max: &a.memory_max,
            },
            find("harnesses::small")?,
            find("harnesses::hog")?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            out.join("fault.json"),
            serde_json::to_vec_pretty(&rows).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "| Fault | Direct Kani | Decoder alone | Sail | Sail detail | State entries | Pass |\n|---|---|---|---|---|---|---|"
        );
        for r in &rows {
            println!(
                "| {:?} | {} | {} | {:?} | {} | {} | {} |",
                r.fault,
                r.direct_detail.as_deref().unwrap_or("not run"),
                r.decoder_alone.as_deref().unwrap_or("not run"),
                r.sail,
                r.sail_detail.lines().last().unwrap_or("").replace('|', "/"),
                r.new_state_entries.len(),
                r.passed
            );
        }
        Ok(if rows.iter().all(|r| r.passed) {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        })
    }
}

pub(crate) fn main(args: &[String]) -> Result<ExitCode, String> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return imp::main(args);
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    Err("Kani conformance runs on Linux x86-64 only".into())
}

//! Running Verus's own `rust_verify_test` suite from a source build
//! ([`crate::source_build`]), one test per process, with or without the
//! shim (ADR 0019).
//!
//! Each test binary is a libtest harness. A test runs as
//! `<binary> --exact <name> --nocapture --test-threads 1` in the
//! `rust_verify_test` package directory, with the environment `vargo test`
//! gives it (`VERUS_IN_VARGO`, `RUST_MIN_STACK`, `RUSTC_BOOTSTRAP`) plus
//! Verus's Rust toolchain and its library directory, so that `rust_verify`
//! finds `librustc_driver`. The harness hands `VERUS_Z3_PATH` to
//! `rust_verify`, so the solver is chosen exactly as for corpus units
//! ([`Solver`]).
//!
//! What a run records:
//! - the outcome libtest reports (`ok`, `FAILED` or `ignored`);
//! - the exit code;
//! - stderr, which holds every diagnostic Verus reported (the harness
//!   prints each one's rendered text) and, for a failing test, its panic
//!   message;
//! - stdout, libtest's own report.
//!
//! Three pieces of that text vary between identical runs and are normalized
//! narrowly: the OS thread id in a panic
//! message (`thread 'name' (12345) panicked at`), and libtest's
//! `finished in 0.52s` timing, and the exact temporary Cargo project path in
//! the two cargo-new tests. Verus progress notices are suppressed in both
//! runs with `--no-report-long-running`, as in the corpus harness.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde::Serialize;

use crate::run::Solver;
use crate::source_build::Manifest;
use crate::{Error, Result};

/// One test: a test binary and a test name in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCase {
    /// The test binary's target name (the file in `tests/`).
    pub binary: String,
    /// The executable.
    pub path: PathBuf,
    /// The libtest name, for example `test_basic_while`.
    pub name: String,
    /// Whether libtest lists it as `#[ignore]`d.
    pub ignored: bool,
}

impl TestCase {
    /// The id reported for this test: `<binary>::<name>`.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}::{}", self.binary, self.name)
    }
}

/// The outcome libtest reported for one test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// `ok`.
    Ok,
    /// `FAILED`.
    Failed,
    /// `ignored`.
    Ignored,
    /// No libtest summary (the harness crashed, or the test was not found).
    Unknown,
}

/// One run of one test.
#[derive(Debug, Clone, Serialize)]
pub struct TestRecord {
    /// What libtest reported.
    pub outcome: Outcome,
    /// The test process's exit code (`None` if killed by a signal).
    pub exit_code: Option<i32>,
    /// libtest's report, with its timing blanked.
    pub stdout: String,
    /// Verus's diagnostics and any panic message, with thread ids blanked.
    pub stderr: String,
    /// Wall time in milliseconds.
    pub wall_ms: f64,
}

/// What every test run needs: the build and Verus's toolchain.
#[derive(Debug, Clone)]
pub struct Suite {
    /// The source build's directory.
    pub build: PathBuf,
    /// Its manifest.
    pub manifest: Manifest,
    /// The pinned z3.
    pub z3: PathBuf,
    /// The Rust toolchain's sysroot (for `librustc_driver`).
    pub sysroot: PathBuf,
    /// Verus's verification threads per test (`--num-threads`, passed
    /// through the harness's `VERUS_EXTRA_ARGS`), or `None` for Verus's
    /// default (all cores). With more than one, the order in which Verus
    /// reports errors varies between identical runs (VI-0007).
    pub verus_threads: Option<u32>,
}

impl Suite {
    /// Opens the build in `build`, built with `rust_toolchain`, testing
    /// against the pinned `z3`.
    ///
    /// # Errors
    ///
    /// `SAIL3040` if the build is missing or incomplete, or was built with
    /// another toolchain; `SAIL3041` if the toolchain's sysroot cannot be
    /// found.
    pub fn open(build: &Path, rust_toolchain: &str, z3: &Path) -> Result<Self> {
        let manifest = Manifest::load(build)?;
        if !manifest.is_complete(build) {
            return Err(Error::new(
                "SAIL3040",
                format!(
                    "the Verus source build in {} is incomplete; rebuild with `cargo xtask build-verus-source --force`",
                    build.display()
                ),
            ));
        }
        if manifest.rust_toolchain != rust_toolchain {
            return Err(Error::new(
                "SAIL3040",
                format!(
                    "the Verus source build in {} used Rust {}, but {rust_toolchain} is pinned",
                    build.display(),
                    manifest.rust_toolchain
                ),
            ));
        }
        let out = Command::new("rustc")
            .args(["--print", "sysroot"])
            .env("RUSTUP_TOOLCHAIN", rust_toolchain)
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| Error::new("SAIL3041", format!("cannot run rustc: {e}")))?;
        if !out.status.success() {
            return Err(Error::new(
                "SAIL3041",
                format!(
                    "rustc {rust_toolchain} --print sysroot failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        let sysroot = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        Ok(Self {
            build: build.to_path_buf(),
            manifest,
            z3: z3.to_path_buf(),
            sysroot,
            verus_threads: Some(1),
        })
    }

    /// The `rust_verify_test` package directory, where tests run.
    #[must_use]
    pub fn package_dir(&self) -> PathBuf {
        crate::source_build::checkout_dir(&self.build)
            .join("source")
            .join("rust_verify_test")
    }

    /// Where the harness materializes each test's input
    /// (`target-verus/release/test_inputs`). It keeps the inputs of tests
    /// that Verus rejects, so [`Self::clear_inputs`] removes them after a
    /// run.
    #[must_use]
    pub fn inputs_dir(&self) -> PathBuf {
        crate::source_build::checkout_dir(&self.build)
            .join("source")
            .join("target-verus")
            .join("release")
            .join("test_inputs")
    }

    /// Removes every materialized test input.
    pub fn clear_inputs(&self) {
        let _ = std::fs::remove_dir_all(self.inputs_dir());
    }

    /// Every test in the suite, sorted by binary and name.
    ///
    /// # Errors
    ///
    /// `SAIL3042` if a test binary cannot list its tests.
    pub fn list(&self) -> Result<Vec<TestCase>> {
        let mut all = Vec::new();
        for binary in &self.manifest.test_binaries {
            let path = self.build.join(&binary.path);
            let ignored: std::collections::BTreeSet<String> =
                self.list_names(&path, true)?.into_iter().collect();
            for name in self.list_names(&path, false)? {
                all.push(TestCase {
                    binary: binary.name.clone(),
                    path: path.clone(),
                    ignored: ignored.contains(&name),
                    name,
                });
            }
        }
        all.sort_by(|a, b| (&a.binary, &a.name).cmp(&(&b.binary, &b.name)));
        Ok(all)
    }

    fn list_names(&self, binary: &Path, ignored: bool) -> Result<Vec<String>> {
        let mut cmd = Command::new(binary);
        cmd.args(["--list", "--format", "terse"]);
        if ignored {
            cmd.arg("--ignored");
        }
        self.environment(&mut cmd);
        let out = cmd
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Error::new("SAIL3042", format!("cannot run {}: {e}", binary.display())))?;
        if !out.status.success() {
            return Err(Error::new(
                "SAIL3042",
                format!(
                    "{} --list failed: {}",
                    binary.display(),
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        Ok(parse_list(&String::from_utf8_lossy(&out.stdout)))
    }

    /// The environment `vargo test` gives the suite, without any solver.
    fn environment(&self, cmd: &mut Command) {
        let lib = self.sysroot.join("lib");
        let toolchain = &self.manifest.rust_toolchain;
        cmd.current_dir(self.package_dir())
            .env("VERUS_IN_VARGO", "1")
            .env("RUST_MIN_STACK", (20 * 1024 * 1024).to_string())
            .env("RUSTC_BOOTSTRAP", "1")
            .env("RUSTUP_TOOLCHAIN", toolchain)
            .env("CARGO_MANIFEST_DIR", self.package_dir())
            .env("RUST_BACKTRACE", "0")
            .env(
                if cfg!(target_os = "macos") {
                    "DYLD_FALLBACK_LIBRARY_PATH"
                } else {
                    "LD_LIBRARY_PATH"
                },
                lib,
            )
            .env_remove("VERUS_EXTRA_ARGS")
            .env_remove("VERUS_KEEP_TEST_DIR")
            .env_remove("VERUS_Z3_PATH")
            .env_remove("SAIL_Z3_PATH")
            .env_remove("SAIL_TRACE_DIR")
            .env_remove("SAIL_DAEMON_SOCKET")
            .env("SAIL_SOLVER_MODE", "default");
        for var in [
            "CARGO",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_TARGET_DIR",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
        ] {
            cmd.env_remove(var);
        }
    }

    /// The exact `VERUS_EXTRA_ARGS` used for this test, including the
    /// progress-suppression default and any explicit verification threads.
    /// Known tests that supply progress suppression themselves omit only
    /// that redundant default; their original arguments remain unchanged.
    #[must_use]
    pub fn extra_verus_args(&self, test: &TestCase) -> String {
        extra_args(
            self.verus_threads,
            supplies_progress_flag(test, &self.build),
        )
    }

    /// Runs one test with `solver`. With `keep_input`, the harness keeps the
    /// test's materialized input directory and prints where it is and how
    /// to rerun it (`VERUS_KEEP_TEST_DIR`).
    ///
    /// # Errors
    ///
    /// `SAIL3020` if the test cannot be started, `SAIL3022` if it exceeds
    /// the time limit (`SAIL_UNIT_TIMEOUT`).
    pub fn run(&self, test: &TestCase, solver: &Solver, keep_input: bool) -> Result<TestRecord> {
        let mut cmd = Command::new(&test.path);
        cmd.args(["--exact", &test.name, "--nocapture", "--test-threads", "1"]);
        self.environment(&mut cmd);
        // Suppress timing-dependent progress notes on both sides (ADR 0006),
        // without duplicating a source-bound test's own effective option.
        cmd.env("VERUS_EXTRA_ARGS", self.extra_verus_args(test));
        if keep_input {
            cmd.env("VERUS_KEEP_TEST_DIR", "1");
        }
        match solver {
            Solver::Plain => {
                cmd.env("VERUS_Z3_PATH", &self.z3);
            }
            Solver::Engine(_) => {
                return Err(Error::new(
                    "SAIL3020",
                    format!(
                        "{}: the source-engine arm does not run rust_verify_test",
                        test.id()
                    ),
                ));
            }
            Solver::Shim {
                shim,
                trace_dir,
                daemon_socket,
                ..
            } => {
                cmd.env("VERUS_Z3_PATH", shim).env("SAIL_Z3_PATH", &self.z3);
                if let Some(dir) = trace_dir {
                    cmd.env("SAIL_TRACE_DIR", dir);
                }
                if let Some(socket) = daemon_socket {
                    cmd.env("SAIL_DAEMON_SOCKET", socket);
                }
            }
        }
        cmd.stdin(Stdio::null());
        let started = Instant::now();
        let output = crate::run::run_with_timeout(&mut cmd, crate::run::unit_timeout())
            .map_err(|(code, message)| Error::new(code, format!("{}: {message}", test.id())))?;
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        let stdout = blank_libtest_timing(&String::from_utf8_lossy(&output.stdout));
        let stderr = blank_thread_ids(&String::from_utf8_lossy(&output.stderr));
        let stderr = if test.binary == "cargo"
            && matches!(
                test.name.as_str(),
                "cargo_new_builds" | "cargo_new_verifies"
            ) {
            blank_cargo_new_path(&stderr, &std::env::temp_dir())
        } else if test.binary == "sail_vi0012_dispatch_order"
            && test.name == "sail_vi0012_dispatch_order_keeps_queries"
            && crate::source_build::hash_file(
                &self
                    .build
                    .join("verus/source/rust_verify_test/tests/sail_vi0012_dispatch_order.rs"),
            )
            .is_ok_and(|sha| {
                sha == "0eaec1bc54d7821f3026e4151bc07d7eafeff7b69cf3dc92e7a5c608536160db"
            })
        {
            blank_dispatch_log_path(&stderr, &std::env::temp_dir())
        } else {
            stderr
        };
        Ok(TestRecord {
            outcome: parse_outcome(&stdout),
            exit_code: output.status.code(),
            stdout,
            stderr,
            wall_ms,
        })
    }
}

fn extra_args(threads: Option<u32>, progress_supplied: bool) -> String {
    let mut args = Vec::new();
    if !progress_supplied {
        args.push("--no-report-long-running".to_owned());
    }
    if let Some(n) = threads {
        args.push(format!("--num-threads {n}"));
    }
    args.join(" ")
}

fn supplies_progress_flag(test: &TestCase, build: &Path) -> bool {
    // These original regressions supply the flag directly to verify_one_file.
    // Bind the whole source, binary and case; never parse arbitrary arguments.
    let original = match (test.binary.as_str(), test.name.as_str()) {
        ("sail_vi0012_progress_order", "sail_vi0012_bucket_start_notes_in_bucket_order") => Some((
            "sail_vi0012_progress_order.rs",
            "046f7fb964c3a6dcb4cbccca1ec9cd61e4d4ed603d09b7976353f930fb6d8a63",
        )),
        ("sail_vi0012_dispatch_order", "sail_vi0012_dispatch_order_keeps_start_note_order") => {
            Some((
                "sail_vi0012_dispatch_order.rs",
                "0eaec1bc54d7821f3026e4151bc07d7eafeff7b69cf3dc92e7a5c608536160db",
            ))
        }
        _ => None,
    };
    if let Some((file, expected)) = original {
        return crate::source_build::hash_file(
            &build.join("verus/source/rust_verify_test/tests").join(file),
        )
        .is_ok_and(|sha| sha == expected);
    }
    test.binary == "examples"
        && test.name == "examples_assert_by_compute"
        && std::fs::read_to_string(build.join("verus/examples/assert_by_compute.rs"))
            .ok()
            .and_then(|s| s.lines().next().map(str::to_owned))
            .is_some_and(|line| {
                line.trim().split(' ').take(3).eq([
                    "//",
                    "rust_verify/tests/example.rs",
                    "no-report-long-running",
                ])
            })
}

/// Owner-approved removal of only the random component in the pinned dispatch
/// test's printed rerun commands. Diagnostics and all other bytes remain intact.
fn blank_dispatch_log_path(stderr: &str, temp: &Path) -> String {
    let prefix =
        "vargo run -p rust_verify -- --crate-type=lib --sail-raw=--num-threads --sail-raw=";
    let options = " --sail-raw=--log --sail-raw=smt --sail-raw=--log-dir --sail-raw=";
    let directory = format!("{}/.tmp", temp.display().to_string().trim_end_matches('/'));
    stderr
        .split_inclusive('\n')
        .map(|line| {
            let Some(rest) = line.strip_prefix(prefix) else {
                return line.to_owned();
            };
            let Some((threads, rest)) = rest.split_once(options) else {
                return line.to_owned();
            };
            if !matches!(threads, "1" | "4") {
                return line.to_owned();
            }
            let Some(random) = rest.strip_prefix(&directory) else {
                return line.to_owned();
            };
            if random.len() < 7
                || !random.as_bytes()[..6].iter().all(u8::is_ascii_alphanumeric)
                || !random[6..].starts_with(' ')
            {
                return line.to_owned();
            }
            format!(
                "{prefix}{threads}{options}{directory}<random>{}",
                &random[6..]
            )
        })
        .collect()
}

/// Blanks only the six-character tempfile component in the two cargo-new
/// tests' known missing-manifest panic. No diagnostic message is removed.
fn blank_cargo_new_path(stderr: &str, temp: &Path) -> String {
    let prefix = format!(
        "cannot open Cargo.toml file: {}/.tmp",
        temp.display().to_string().trim_end_matches('/')
    );
    stderr
        .split_inclusive('\n')
        .map(|line| {
            let Some(rest) = line.strip_prefix(&prefix) else {
                return line.to_owned();
            };
            if rest
                .as_bytes()
                .get(..6)
                .is_some_and(|s| s.iter().all(u8::is_ascii_alphanumeric))
                && matches!(
                    rest.get(6..),
                    Some("/test_project/Cargo.toml\n" | "/test_project/Cargo.toml")
                )
            {
                format!("{prefix}<id>{}", &rest[6..])
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// The test names in `--list --format terse` output (`name: test` lines).
#[must_use]
pub fn parse_list(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .map(str::to_owned)
        .collect()
}

/// The outcome in libtest's summary line
/// (`test result: ok. 1 passed; 0 failed; 0 ignored; ...`).
#[must_use]
pub fn parse_outcome(stdout: &str) -> Outcome {
    let Some(line) = stdout
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("test result: "))
    else {
        return Outcome::Unknown;
    };
    let words: Vec<&str> = line.split([';', ' ']).filter(|w| !w.is_empty()).collect();
    let count = |what: &str| -> u64 {
        words
            .windows(2)
            .find_map(|pair| match pair {
                [n, w] if *w == what => n.parse().ok(),
                _ => None,
            })
            .unwrap_or(0)
    };
    match (count("passed"), count("failed"), count("ignored")) {
        (1, 0, 0) => Outcome::Ok,
        (0, 1, 0) => Outcome::Failed,
        (0, 0, 1) => Outcome::Ignored,
        _ => Outcome::Unknown,
    }
}

/// Blanks libtest's `finished in <seconds>s` timing.
#[must_use]
pub fn blank_libtest_timing(stdout: &str) -> String {
    stdout
        .split_inclusive('\n')
        .map(|line| {
            if line.starts_with("test result: ")
                && let Some(at) = line.find("; finished in ")
            {
                let end = if line.ends_with('\n') { "\n" } else { "" };
                format!("{}; finished in <time>{end}", &line[..at])
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// Blanks the OS thread id in Rust panic messages:
/// `thread 'name' (12345) panicked at` becomes `thread 'name' (<tid>)
/// panicked at`.
#[must_use]
pub fn blank_thread_ids(stderr: &str) -> String {
    stderr
        .split_inclusive('\n')
        .map(|line| {
            let Some(rest) = line.strip_prefix("thread '") else {
                return line.to_owned();
            };
            let Some(close) = rest.find("' (") else {
                return line.to_owned();
            };
            let after = &rest[close + 3..];
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 && after[digits..].starts_with(") panicked at") {
                format!("thread '{}' (<tid>{}", &rest[..close], &after[digits..])
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// The input directory the harness reports with `VERUS_KEEP_TEST_DIR`
/// (`the input directory is <path>`).
#[must_use]
pub fn kept_input_dir(stderr: &str) -> Option<PathBuf> {
    stderr
        .lines()
        .find_map(|l| l.strip_prefix("the input directory is "))
        .map(|p| PathBuf::from(p.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_log_paths_preserve_functional_output() {
        let command = "vargo run -p rust_verify -- --crate-type=lib --sail-raw=--num-threads --sail-raw=1 --sail-raw=--log --sail-raw=smt --sail-raw=--log-dir --sail-raw=/tmp/.tmpAb123z /owned/test.rs\n";
        let other = command.replace("Ab123z", "Zx987q");
        assert_eq!(
            blank_dispatch_log_path(command, Path::new("/tmp")),
            blank_dispatch_log_path(&other, Path::new("/tmp"))
        );
        for line in [
            format!("error: {command}"),
            command.replace("--sail-raw=1 ", "--sail-raw=2 "),
            command.replace(".tmpAb123z", ".tmpAb123zz"),
        ] {
            assert_eq!(blank_dispatch_log_path(&line, Path::new("/tmp")), line);
        }
        assert_ne!(
            blank_dispatch_log_path(command, Path::new("/tmp")),
            blank_dispatch_log_path(
                &command.replace("--sail-raw=1 ", "--sail-raw=4 "),
                Path::new("/tmp")
            )
        );
    }

    #[test]
    fn dispatch_progress_default_is_source_and_case_bound()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        const ORIGINAL: &[u8] =
            include_bytes!("../tests/fixtures/rvt-progress/sail_vi0012_dispatch_order.rs");
        let dir = std::env::temp_dir().join(format!("sail-rvt-dispatch-{}", std::process::id()));
        let source = dir.join("verus/source/rust_verify_test/tests/sail_vi0012_dispatch_order.rs");
        std::fs::create_dir_all(source.parent().ok_or("source directory")?)?;
        let mut test = TestCase {
            binary: "sail_vi0012_dispatch_order".to_owned(),
            path: PathBuf::new(),
            name: "sail_vi0012_dispatch_order_keeps_start_note_order".to_owned(),
            ignored: false,
        };
        assert!(!supplies_progress_flag(&test, &dir));
        std::fs::write(&source, ORIGINAL)?;
        assert!(supplies_progress_flag(&test, &dir));
        assert_eq!(extra_args(None, supplies_progress_flag(&test, &dir)), "");
        assert_eq!(
            extra_args(Some(4), supplies_progress_flag(&test, &dir)),
            "--num-threads 4"
        );
        test.name = "sail_vi0012_dispatch_order_keeps_queries".to_owned();
        assert!(!supplies_progress_flag(&test, &dir));
        test.name = "sail_vi0012_dispatch_order_keeps_start_note_order".to_owned();
        test.binary = "foreign_binary".to_owned();
        assert!(!supplies_progress_flag(&test, &dir));
        test.binary = "sail_vi0012_dispatch_order".to_owned();
        let mut changed = ORIGINAL.to_vec();
        changed.extend_from_slice(b"\n// changed fixture\n");
        std::fs::write(&source, changed)?;
        assert!(!supplies_progress_flag(&test, &dir));
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn fork_progress_default_requires_exact_original_source_and_case() {
        const ORIGINAL: &[u8] =
            include_bytes!("../tests/fixtures/rvt-progress/sail_vi0012_progress_order.rs");
        let dir = std::env::temp_dir().join(format!("sail-rvt-vi0012-{}", std::process::id()));
        let source = dir.join("verus/source/rust_verify_test/tests/sail_vi0012_progress_order.rs");
        assert!(std::fs::create_dir_all(source.parent().unwrap_or(&dir)).is_ok());
        let mut test = TestCase {
            binary: "sail_vi0012_progress_order".to_owned(),
            path: PathBuf::new(),
            name: "sail_vi0012_bucket_start_notes_in_bucket_order".to_owned(),
            ignored: false,
        };
        assert!(!supplies_progress_flag(&test, &dir));
        assert!(std::fs::write(&source, ORIGINAL).is_ok());
        assert!(supplies_progress_flag(&test, &dir));
        // Omit only the redundant default, never an explicit thread override.
        assert_eq!(extra_args(None, supplies_progress_flag(&test, &dir)), "");
        assert_eq!(
            extra_args(Some(1), supplies_progress_flag(&test, &dir)),
            "--num-threads 1"
        );
        test.name = "another_case".to_owned();
        assert!(!supplies_progress_flag(&test, &dir));
        test.name = "sail_vi0012_bucket_start_notes_in_bucket_order".to_owned();
        test.binary = "foreign_binary".to_owned();
        assert!(!supplies_progress_flag(&test, &dir));
        test.binary = "sail_vi0012_progress_order".to_owned();
        let mut changed = ORIGINAL.to_vec();
        changed.extend_from_slice(b"\n// changed source must retain the default\n");
        assert!(std::fs::write(&source, changed).is_ok());
        assert_eq!(
            extra_args(None, supplies_progress_flag(&test, &dir)),
            "--no-report-long-running"
        );
        let changed = String::from_utf8_lossy(ORIGINAL).replace(
            "\"--no-report-long-running\"",
            "\"--no-report-long-runninG\"",
        );
        assert!(std::fs::write(&source, changed).is_ok());
        assert!(!supplies_progress_flag(&test, &dir));
        assert!(std::fs::remove_file(&source).is_ok());
        assert!(!supplies_progress_flag(&test, &dir));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn source_supplied_progress_option_is_not_added_twice() {
        let dir = std::env::temp_dir().join(format!("sail-rvt-progress-{}", std::process::id()));
        let source = dir.join("verus/examples/assert_by_compute.rs");
        assert!(std::fs::create_dir_all(source.parent().unwrap_or(&dir)).is_ok());
        let mut test = TestCase {
            binary: "examples".to_owned(),
            path: PathBuf::new(),
            name: "examples_assert_by_compute".to_owned(),
            ignored: false,
        };
        assert!(
            std::fs::write(
                &source,
                "// rust_verify/tests/example.rs no-report-long-running --- comment\n"
            )
            .is_ok()
        );
        assert!(supplies_progress_flag(&test, &dir));
        assert_eq!(extra_args(Some(1), true), "--num-threads 1");
        assert_eq!(extra_args(None, true), "");
        test.name = "another_example".to_owned();
        assert!(!supplies_progress_flag(&test, &dir));
        test.name = "examples_assert_by_compute".to_owned();
        assert!(
            std::fs::write(&source, "// rust_verify/tests/example.rs expect-success\n").is_ok()
        );
        assert!(!supplies_progress_flag(&test, &dir));
        assert_eq!(
            extra_args(Some(1), false),
            "--no-report-long-running --num-threads 1"
        );
        assert_eq!(extra_args(None, false), "--no-report-long-running");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn blanks_only_the_known_cargo_new_generated_path() {
        let root = Path::new("/tmp");
        let a = "cannot open Cargo.toml file: /tmp/.tmpAb123X/test_project/Cargo.toml\n";
        let b = "cannot open Cargo.toml file: /tmp/.tmpZy987Q/test_project/Cargo.toml\n";
        assert_eq!(blank_cargo_new_path(a, root), blank_cargo_new_path(b, root));
        for text in [
            "wrong error: /tmp/.tmpAb123X/test_project/Cargo.toml\n",
            "cannot open Cargo.toml file: /tmp/.tmpAb123X/other/Cargo.toml\n",
            "cannot open Cargo.toml file: /tmp/.tmpAb123XY/test_project/Cargo.toml\n",
            "cannot open Cargo.toml file: /else/.tmpAb123X/test_project/Cargo.toml\n",
        ] {
            assert_eq!(blank_cargo_new_path(text, root), text);
        }
    }

    #[test]
    fn lists_terse_test_names() {
        let text = "a::b: test\nc: test\nx: bench\n\n";
        assert_eq!(parse_list(text), vec!["a::b".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn reads_the_libtest_summary() {
        let ok = "running 1 test\ntest t ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in <time>\n";
        assert_eq!(parse_outcome(ok), Outcome::Ok);
        let failed = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out; finished in <time>\n";
        assert_eq!(parse_outcome(failed), Outcome::Failed);
        let ignored = "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 9 filtered out; finished in <time>\n";
        assert_eq!(parse_outcome(ignored), Outcome::Ignored);
        let none = "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in <time>\n";
        assert_eq!(parse_outcome(none), Outcome::Unknown);
        assert_eq!(parse_outcome("thread panicked\n"), Outcome::Unknown);
    }

    #[test]
    fn blanks_only_libtest_timing() {
        let s = "running 1 test\ntest result: ok. 1 passed; 0 failed; finished in 0.52s\nafter\n";
        assert_eq!(
            blank_libtest_timing(s),
            "running 1 test\ntest result: ok. 1 passed; 0 failed; finished in <time>\nafter\n"
        );
        assert_eq!(
            blank_libtest_timing("x finished in 1s\n"),
            "x finished in 1s\n"
        );
    }

    #[test]
    fn blanks_only_panic_thread_ids() {
        let s = "error: x\nthread 'modes::t' (78167816) panicked at rust_verify_test/tests/modes.rs:5:1:\nthread 'a' (b) panicked at\n";
        assert_eq!(
            blank_thread_ids(s),
            "error: x\nthread 'modes::t' (<tid>) panicked at rust_verify_test/tests/modes.rs:5:1:\nthread 'a' (b) panicked at\n"
        );
    }

    #[test]
    fn finds_the_kept_input_dir() {
        let s = "the input directory is /t/test_inputs/basic-1-t\nrerun this test with:\n";
        assert_eq!(
            kept_input_dir(s),
            Some(PathBuf::from("/t/test_inputs/basic-1-t"))
        );
        assert_eq!(kept_input_dir("nothing\n"), None);
    }
}

use std::ffi::OsString;
use std::fs::{self, File, FileTimes};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use super::{MetadataStdout, metadata_output};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    run: PathBuf,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = loop {
            let candidate = std::env::temp_dir().join(format!(
                "sail-metadata-stdout,{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        };
        let fixture = Self {
            source: root.join("source"),
            run: root.join("run"),
            root,
        };
        fs::create_dir(&fixture.source)?;
        fs::create_dir(&fixture.run)?;
        fs::write(fixture.run.join("unrelated"), b"retain this file")?;
        Ok(fixture)
    }

    fn source_stamp(&self) -> TestResult<DirectoryStamp> {
        // An old mtime makes temporary entry changes visible on coarse clocks.
        File::open(&self.source)?.set_times(
            FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)),
        )?;
        directory_stamp(&self.source)
    }

    fn assert_run_clean(&self) -> TestResult {
        assert_eq!(fs::read(self.run.join("unrelated"))?, b"retain this file");
        let entries = fs::read_dir(&self.run)?.collect::<io::Result<Vec<_>>>()?;
        assert_eq!(entries.len(), 1);
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DirectoryStamp {
    modified: SystemTime,
    #[cfg(unix)]
    changed: (i64, i64),
}

fn directory_stamp(path: &Path) -> TestResult<DirectoryStamp> {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt as _;

    let metadata = fs::metadata(path)?;
    Ok(DirectoryStamp {
        modified: metadata.modified()?,
        #[cfg(unix)]
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
}

fn arguments(emission: &[&str]) -> Vec<OsString> {
    let mut args = vec![std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into())];
    args.extend(
        [
            "--crate-name",
            "sail_metadata_stdout_probe",
            "--crate-type=lib",
            "--edition=2024",
            "--color=never",
        ]
        .map(OsString::from),
    );
    args.extend(emission.iter().map(OsString::from));
    args.extend(["-o", "-", "-"].map(OsString::from));
    args
}

fn compile(args: &[OsString], source_dir: &Path, source: &[u8]) -> TestResult<Output> {
    let (compiler, options) = args.split_first().ok_or("missing compiler argument")?;
    let mut child = Command::new(compiler)
        .args(options)
        .current_dir(source_dir)
        .env("RUSTC_BOOTSTRAP", "1")
        .env("SAIL_METADATA_STDOUT_TEST_VALUE", "child-local-value")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("missing compiler stdin")?
        .write_all(source)?;
    Ok(child.wait_with_output()?)
}

fn assert_success_preserved(fixture: &Fixture, emission: &[&str], source: &[u8]) -> TestResult {
    let mut args = arguments(emission);
    let before = fixture.source_stamp()?;
    let expected = compile(&args, &fixture.source, source)?;
    assert!(expected.status.success(), "{expected:?}");
    assert_ne!(
        before.modified,
        directory_stamp(&fixture.source)?.modified,
        "the control must expose rustc temporary output in the source directory"
    );

    let before = fixture.source_stamp()?;
    let index = metadata_output(&args).ok_or("metadata stdout probe not selected")?;
    let metadata = MetadataStdout::prepare(&mut args, index, &fixture.run)?;
    let actual = compile(&args, &fixture.source, source)?;
    assert_eq!(directory_stamp(&fixture.source)?, before);
    let mut stdout = actual.stdout;
    metadata.replay(&mut stdout)?;
    assert_eq!(actual.status, expected.status);
    assert_eq!(stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    drop(metadata);
    assert_eq!(directory_stamp(&fixture.source)?, before);
    fixture.assert_run_clean()
}

#[test]
fn metadata_stdout_preserves_compiler_bytes_diagnostics_and_directory_times() -> TestResult {
    let fixture = Fixture::new()?;
    let source = br#"fn unused_probe() {}
pub const VALUE: &str = env!("SAIL_METADATA_STDOUT_TEST_VALUE");
"#;
    for emission in [&["--emit=metadata"][..], &["--emit", "metadata"][..]] {
        assert_success_preserved(&fixture, emission, source)?;
    }
    assert_eq!(fs::read_dir(&fixture.source)?.count(), 0);
    Ok(())
}

#[test]
fn stdin_modules_resolve_from_the_original_source_directory() -> TestResult {
    let fixture = Fixture::new()?;
    fs::write(
        fixture.source.join("relative.rs"),
        b"pub const VALUE: u8 = 7;\n",
    )?;
    let source = br#"#[path = "relative.rs"]
mod relative;
fn unused_probe() {}
pub const VALUE: u8 = relative::VALUE;
"#;
    assert_success_preserved(&fixture, &["--emit=metadata"], source)?;
    assert_eq!(fs::read_dir(&fixture.source)?.count(), 1);
    assert_eq!(
        fs::read(fixture.source.join("relative.rs"))?,
        b"pub const VALUE: u8 = 7;\n"
    );
    Ok(())
}

#[test]
fn failed_compiler_preserves_status_diagnostics_and_empty_stdout() -> TestResult {
    let fixture = Fixture::new()?;
    let source = b"pub const VALUE: u8 = false;\n";
    let mut args = arguments(&["--emit", "metadata"]);
    let expected = compile(&args, &fixture.source, source)?;
    assert!(!expected.status.success());
    assert_eq!(expected.stdout.len(), 0);

    let before = fixture.source_stamp()?;
    let index = metadata_output(&args).ok_or("metadata stdout probe not selected")?;
    let metadata = MetadataStdout::prepare(&mut args, index, &fixture.run)?;
    let actual = compile(&args, &fixture.source, source)?;
    let mut stdout = actual.stdout;
    metadata.replay(&mut stdout)?;
    assert_eq!(actual.status, expected.status);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(stdout, expected.stdout);
    drop(metadata);
    assert_eq!(directory_stamp(&fixture.source)?, before);
    fixture.assert_run_clean()
}

struct FailedWriter;

impl io::Write for FailedWriter {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("test writer failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn real_metadata_replay_reports_writer_failure() -> TestResult {
    let fixture = Fixture::new()?;
    let mut args = arguments(&["--emit=metadata"]);
    let index = metadata_output(&args).ok_or("metadata stdout probe not selected")?;
    let metadata = MetadataStdout::prepare(&mut args, index, &fixture.run)?;
    let actual = compile(&args, &fixture.source, b"pub const VALUE: u8 = 7;\n")?;
    assert!(actual.status.success(), "{actual:?}");
    let error = metadata
        .replay(&mut FailedWriter)
        .err()
        .ok_or("metadata replay accepted a failed writer")?;
    assert_eq!(error.code, "SAIL3111");
    drop(metadata);
    fixture.assert_run_clean()
}

#[test]
fn ambiguous_outputs_and_queries_do_not_redirect_metadata() {
    let cases: &[&[&str]] = &[
        &["rustc", "--emit=metadata", "-o", "file", "-"],
        &["rustc", "--emit=metadata", "--out-dir", "output", "-"],
        &["rustc", "--emit=metadata=file", "-o", "-", "-"],
        &["rustc", "--emit", "metadata=file", "-o", "-", "-"],
        &["rustc", "--emit=metadata,link", "-o", "-", "-"],
        &["rustc", "--emit=link", "-o", "-", "-"],
        &["rustc", "--emit=metadata", "--emit=link", "-o", "-", "-"],
        &["rustc", "--emit=metadata", "-o", "-", "-o", "file", "-"],
        &["rustc", "--emit=metadata", "-ofile", "-o", "-", "-"],
        &["rustc", "--emit=metadata", "-o", "-", "--help"],
        &["rustc", "--emit=metadata", "-o", "-", "--version"],
        &["rustc", "--emit=metadata", "-o", "-", "--print=cfg"],
        &["rustc", "--emit=metadata", "-o", "-", "--print", "cfg"],
    ];
    for case in cases {
        let args: Vec<OsString> = case.iter().map(OsString::from).collect();
        assert_eq!(metadata_output(&args), None, "{case:?}");
    }
}

//! `sail new`: create a Cargo project that `sail verify` verifies as it
//! stands.
//!
//! The project is exactly what `cargo verus new` writes, made by Sail's own
//! Verus (ADR 0043): Sail runs that runtime's `cargo-verus` by path. Its
//! `vstd` dependency comes from the toolchain manifest compiled into that
//! `cargo-verus` for its own Verus version, so it always matches the
//! verifier Sail runs and is never copied into Sail. The runtime's
//! `cargo-verus` is never put on `PATH`, so a user's own `cargo verus` keeps
//! resolving as before.
//!
//! `cargo verus new` only takes a package name and creates `./NAME`. Sail
//! runs it in a private staging directory next to the destination and moves
//! the result into place, which gives `sail new` Cargo's `PATH` and `--name`
//! arguments and means a failed run leaves nothing behind. Two differences
//! from `cargo verus new`, both following `cargo new`: no nested Git
//! repository is created inside an existing one, and a missing `git` is not
//! an error (VI-0016) but leaves the project without a repository.
//! A qualified adjacent proof bundle may supply its admitted resolution as a
//! new project's lockfile. A lockfile already written by the generator is kept.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// `sail new --help`.
pub(crate) const USAGE: &str = "\
Create a Cargo project that Sail verifies as it stands: a Verus binary
(default) or library whose vstd dependency matches Sail's Verus. The files
are what `cargo verus new` writes, made by Sail's own Verus.

USAGE:
    sail new [--bin | --lib] [--name NAME] PATH

OPTIONS:
    --bin          create a binary (src/main.rs); the default
    --lib          create a library (src/lib.rs)
    --name NAME    the package name; defaults to PATH's last component

PATH must not exist, or be an empty directory. Then:

    cd PATH && sail verify

Exits 0 on success, 2 on a usage error and 1 otherwise.
";

/// Exit code for a malformed command line.
const EXIT_USAGE: u8 = 2;

/// What to create.
#[derive(Debug, PartialEq, Eq)]
struct Options {
    path: PathBuf,
    name: Option<String>,
    lib: bool,
}

/// A failure: a stable code and a message.
#[derive(Debug, PartialEq, Eq)]
struct Failure(&'static str, String);

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let options = match parse(args) {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(
                err,
                "error[SAIL0002]: sail new: {message}\nusage: sail new [--bin | --lib] [--name NAME] PATH"
            );
            return ExitCode::from(EXIT_USAGE);
        }
    };
    match create(&options) {
        Ok(created) => {
            let kind = if options.lib { "library" } else { "binary" };
            let path = options.path.display();
            let vstd = created
                .vstd
                .map(|v| format!(" (vstd {v}, matching Sail's Verus)"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "Created Verus {kind} package `{}` at {path}{vstd}\nNext: cd {path} && sail verify",
                created.name
            );
            ExitCode::SUCCESS
        }
        Err(Failure(code, message)) => {
            let _ = writeln!(err, "error[{code}]: sail new: {message}");
            ExitCode::from(1)
        }
    }
}

fn parse(args: &[String]) -> Result<Options, String> {
    let (mut path, mut name, mut kind) = (None, None, None);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_str();
        match arg {
            "--bin" | "--lib" => {
                if kind.is_some_and(|k| k != arg) {
                    return Err("`--bin` and `--lib` cannot be combined".to_owned());
                }
                kind = Some(arg);
            }
            "--name" => {
                let value = args.next().ok_or("`--name` needs a value")?;
                name = Some(value.clone());
            }
            _ if arg.starts_with("--name=") => {
                name = Some(arg["--name=".len()..].to_owned());
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option `{arg}`")),
            _ if path.is_some() => return Err(format!("unexpected argument `{arg}`")),
            _ => path = Some(PathBuf::from(arg)),
        }
    }
    let path = path.ok_or("missing PATH, the directory to create")?;
    Ok(Options {
        path,
        name,
        lib: kind == Some("--lib"),
    })
}

/// Rust keywords, which Cargo refuses as package names.
const KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// The package name: `--name`, else the destination's last component.
fn package_name(options: &Options) -> Result<String, Failure> {
    let name = match &options.name {
        Some(name) => name.clone(),
        None => options
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .ok_or_else(|| {
                Failure(
                    "SAIL0131",
                    format!(
                        "cannot take a package name from `{}`; pass --name NAME",
                        options.path.display()
                    ),
                )
            })?,
    };
    check_name(&name, options.lib).map_err(|why| {
        Failure(
            "SAIL0131",
            format!("`{name}` is not a valid package name: {why}; pass --name NAME"),
        )
    })?;
    Ok(name)
}

/// Cargo's rules for a new package's name, restricted to ASCII, plus `vstd`,
/// which the project depends on.
fn check_name(name: &str, lib: bool) -> Result<(), &'static str> {
    let first = name.chars().next().ok_or("it is empty")?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err("it must start with a letter or `_`");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("it may contain only ASCII letters, digits, `-` and `_`");
    }
    let crate_name = name.replace('-', "_");
    if KEYWORDS.contains(&crate_name.as_str()) {
        return Err("it is a Rust keyword");
    }
    if crate_name == "test" {
        return Err("it conflicts with Rust's built-in test library");
    }
    if crate_name == "vstd" {
        return Err("it conflicts with the project's vstd dependency");
    }
    if !lib && ["deps", "examples", "build", "incremental"].contains(&name) {
        return Err("it conflicts with Cargo's build directory names");
    }
    Ok(())
}

/// Refuses an existing destination unless it is an empty directory.
fn check_destination(path: &Path) -> Result<(), Failure> {
    let exists = |why: &str| {
        Failure(
            "SAIL0130",
            format!(
                "`{}` already exists and {why}; choose a new path",
                path.display()
            ),
        )
    };
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Failure(
            "SAIL0132",
            format!("cannot inspect `{}`: {e}", path.display()),
        )),
        Ok(meta) if !meta.is_dir() => Err(exists("is not a directory")),
        Ok(_) => match std::fs::read_dir(path).map(|mut entries| entries.next().is_none()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(exists("is not empty")),
            Err(e) => Err(Failure(
                "SAIL0132",
                format!("cannot read `{}`: {e}", path.display()),
            )),
        },
    }
}

/// Whether `dir` is inside a Git working tree (an ancestor holds `.git`),
/// where `cargo new` creates no repository of its own.
fn inside_git_repo(dir: &Path) -> bool {
    dir.canonicalize()
        .is_ok_and(|dir| dir.ancestors().any(|a| a.join(".git").exists()))
}

/// Whether `cargo verus new` wrote every project file under `dir`.
fn project_written(dir: &Path, lib: bool) -> bool {
    let source = if lib { "lib.rs" } else { "main.rs" };
    [
        dir.join("Cargo.toml"),
        dir.join(".gitignore"),
        dir.join("src").join(source),
    ]
    .iter()
    .all(|file| file.is_file())
}

/// The `vstd` requirement in a generated `Cargo.toml`, for the summary line.
fn vstd_requirement(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let value = line.strip_prefix("vstd")?.trim_start().strip_prefix('=')?;
        Some(
            value
                .trim()
                .trim_matches('"')
                .trim_start_matches('=')
                .to_owned(),
        )
    })
}

/// A created project.
struct Created {
    name: String,
    vstd: Option<String>,
}

/// A staging directory beside the destination, removed when dropped.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Seed only a newly generated project, without replacing a generator's lock.
fn write_shipped_lockfile(project: &Path, lock: Option<&str>) -> Result<(), Failure> {
    let Some(lock) = lock else {
        return Ok(());
    };
    let path = project.join("Cargo.lock");
    let cannot = |error| {
        Failure(
            "SAIL0132",
            format!(
                "cannot write admitted lockfile `{}`: {error}",
                path.display()
            ),
        )
    };
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(cannot(error)),
    };
    file.write_all(lock.as_bytes()).map_err(cannot)
}

fn create(options: &Options) -> Result<Created, Failure> {
    let name = package_name(options)?;
    check_destination(&options.path)?;
    let cargo_verus =
        crate::verify::cargo_verus().map_err(|message| Failure("SAIL0101", message))?;
    let cannot =
        |what: String| move |e: std::io::Error| Failure("SAIL0132", format!("{what}: {e}"));
    let parent = match options.path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&parent)
        .map_err(cannot(format!("cannot create `{}`", parent.display())))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let staging = Staging(parent.join(format!(".sail-new-{}-{nanos}", std::process::id())));
    std::fs::create_dir(&staging.0).map_err(cannot(format!(
        "cannot create a staging directory in `{}`",
        parent.display()
    )))?;
    let git = crate::verify::on_path("git").is_some();
    let output = Command::new(&cargo_verus)
        .args(["verus", "new", if options.lib { "--lib" } else { "--bin" }])
        .arg(&name)
        .current_dir(&staging.0)
        .stdin(Stdio::null())
        .output()
        .map_err(cannot(format!("cannot run {}", cargo_verus.display())))?;
    let staged = staging.0.join(&name);
    // Without git, `cargo verus new` writes every file, then fails to run
    // `git init` (VI-0016); the project is complete, as with `cargo new
    // --vcs none`.
    if !(output.status.success() || (!git && project_written(&staged, options.lib))) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Failure(
            "SAIL0132",
            format!(
                "`cargo verus new` (Sail's Verus, {}) failed ({}): {}",
                cargo_verus.display(),
                output.status,
                stderr.trim()
            ),
        ));
    }
    if !project_written(&staged, options.lib) {
        return Err(Failure(
            "SAIL0132",
            format!(
                "`cargo verus new` (Sail's Verus, {}) succeeded but wrote no project",
                cargo_verus.display()
            ),
        ));
    }
    let shipped_lock = sail_verus::managed::new_project_lockfile(&cargo_verus, &name);
    write_shipped_lockfile(&staged, shipped_lock.as_deref())?;
    if inside_git_repo(&parent) {
        match std::fs::remove_dir_all(staged.join(".git")) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(cannot("cannot remove the nested Git repository".to_owned())(e));
            }
            _ => {}
        }
    }
    let manifest = std::fs::read_to_string(staged.join("Cargo.toml")).unwrap_or_default();
    // An empty destination directory is replaced; a non-empty one (created
    // meanwhile) makes the rename fail, so nothing is ever overwritten.
    if options.path.is_dir() {
        std::fs::remove_dir(&options.path).map_err(cannot(format!(
            "cannot replace the empty directory `{}`",
            options.path.display()
        )))?;
    }
    std::fs::rename(&staged, &options.path).map_err(cannot(format!(
        "cannot move the project to `{}`",
        options.path.display()
    )))?;
    Ok(Created {
        name,
        vstd: vstd_requirement(&manifest),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Failure, Options, Staging, check_destination, check_name, inside_git_repo, package_name,
        parse, project_written, vstd_requirement, write_shipped_lockfile,
    };
    use std::path::{Path, PathBuf};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sail-new-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(&dir).is_ok());
        dir
    }

    #[test]
    fn parses_cargo_new_arguments() {
        let parsed = |list: &[&str]| parse(&args(list));
        assert_eq!(
            parsed(&["demo"]),
            Ok(Options {
                path: "demo".into(),
                name: None,
                lib: false
            })
        );
        assert_eq!(
            parsed(&["--lib", "--name", "x", "a/b"]),
            Ok(Options {
                path: "a/b".into(),
                name: Some("x".into()),
                lib: true
            })
        );
        assert_eq!(
            parsed(&["--name=y", "--bin", "d"]).map(|o| (o.name, o.lib)),
            Ok((Some("y".into()), false))
        );
        assert!(parsed(&["--lib", "--lib", "d"]).is_ok());
        for bad in [
            &[][..],
            &["--lib"],
            &["--bin", "--lib", "d"],
            &["--name"],
            &["--vcs", "d"],
            &["a", "b"],
        ] {
            assert!(parsed(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn names_follow_cargo() {
        for good in ["demo", "my-proofs", "_x", "a1_b2"] {
            assert_eq!(check_name(good, false), Ok(()), "{good}");
        }
        for bad in [
            "", "1demo", "-x", "a/b", "a.b", "é", "fn", "self", "test", "vstd",
        ] {
            assert!(check_name(bad, true).is_err(), "{bad}");
        }
        assert!(check_name("build", false).is_err());
        assert_eq!(check_name("build", true), Ok(()));
    }

    #[test]
    fn name_defaults_to_the_last_path_component() {
        let options = |path: &str, name: Option<&str>| Options {
            path: path.into(),
            name: name.map(str::to_owned),
            lib: true,
        };
        assert_eq!(
            package_name(&options("work/demo", None)),
            Ok("demo".to_owned())
        );
        assert_eq!(
            package_name(&options("work/2026", Some("proofs"))),
            Ok("proofs".to_owned())
        );
        assert!(matches!(
            package_name(&options("work/2026", None)),
            Err(Failure("SAIL0131", _))
        ));
        assert!(matches!(
            package_name(&options("..", None)),
            Err(Failure("SAIL0131", _))
        ));
    }

    #[test]
    fn refuses_existing_non_empty_destinations() {
        let root = scratch("dest");
        assert_eq!(check_destination(&root.join("absent")), Ok(()));
        let empty = root.join("empty");
        assert!(std::fs::create_dir(&empty).is_ok());
        assert_eq!(check_destination(&empty), Ok(()));
        let full = root.join("full");
        assert!(std::fs::create_dir(&full).is_ok());
        assert!(std::fs::write(full.join("keep"), "x").is_ok());
        assert!(matches!(
            check_destination(&full),
            Err(Failure("SAIL0130", m)) if m.contains("is not empty")
        ));
        let file = root.join("file");
        assert!(std::fs::write(&file, "x").is_ok());
        assert!(matches!(
            check_destination(&file),
            Err(Failure("SAIL0130", m)) if m.contains("not a directory")
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_written_projects_and_git_repositories() {
        let root = scratch("files");
        let project = root.join("p");
        assert!(std::fs::create_dir_all(project.join("src")).is_ok());
        assert!(!project_written(&project, true));
        for file in ["Cargo.toml", ".gitignore", "src/lib.rs"] {
            assert!(std::fs::write(project.join(file), "").is_ok());
        }
        assert!(project_written(&project, true));
        assert!(!project_written(&project, false));
        let nested = root.join("repo").join("a");
        assert!(std::fs::create_dir_all(&nested).is_ok());
        let outside = !inside_git_repo(&nested);
        assert!(std::fs::create_dir(root.join("repo").join(".git")).is_ok());
        assert!(inside_git_repo(&nested));
        // The temporary directory itself may lie inside a repository.
        assert!(outside || inside_git_repo(Path::new(&root)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_the_vstd_requirement() {
        let manifest =
            "[package]\nname = \"d\"\n\n[dependencies]\nvstd = \"=0.0.0-2026-10-18-0147\"\n";
        assert_eq!(
            vstd_requirement(manifest),
            Some("0.0.0-2026-10-18-0147".to_owned())
        );
        assert_eq!(vstd_requirement("[dependencies]\nvstdx = \"1\"\n"), None);
    }

    #[test]
    fn admitted_lock_is_written_only_for_new_projects_and_survives_rename() {
        let root = scratch("admitted-lock");
        let project = root.join("private-generated");
        assert!(std::fs::create_dir(&project).is_ok());
        assert!(write_shipped_lockfile(&project, None).is_ok());
        assert!(!project.join("Cargo.lock").exists());
        // The provider has already renamed and ordered the admitted resolution.
        // The CLI must preserve every supplied byte, rather than resolve again.
        let lock = "version = 4\n\n[[package]]\nname = \"client\"\nversion = \"0.1.0\"\ndependencies = [\n \"vstd\",\n]\n\n[[package]]\nname = \"vstd\"\nversion = \"0.0.0-2026-10-18-0147\"\n";
        assert!(write_shipped_lockfile(&project, Some(lock)).is_ok());
        assert_eq!(
            std::fs::read(project.join("Cargo.lock")).ok().as_deref(),
            Some(lock.as_bytes())
        );
        let destination = root.join("public-project");
        assert!(std::fs::rename(&project, &destination).is_ok());
        assert_eq!(
            std::fs::read(destination.join("Cargo.lock"))
                .ok()
                .as_deref(),
            Some(lock.as_bytes())
        );
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn generated_lock_is_preserved_and_lock_write_failure_cleans_staging() {
        let root = scratch("lock-failure");
        let generated = root.join("Cargo.lock");
        assert!(std::fs::write(&generated, b"generator lock\n").is_ok());
        assert!(write_shipped_lockfile(&root, Some("admitted lock\n")).is_ok());
        assert_eq!(
            std::fs::read(&generated).ok().as_deref(),
            Some(b"generator lock\n".as_slice())
        );
        let staging = Staging(root.join("private-stage"));
        assert!(std::fs::create_dir(&staging.0).is_ok());
        assert!(matches!(
            write_shipped_lockfile(&staging.0.join("missing-project"), Some("lock")),
            Err(Failure("SAIL0132", message)) if message.contains("Cargo.lock")
        ));
        let path = staging.0.clone();
        drop(staging);
        assert!(!path.exists());
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}

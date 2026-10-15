//! `sail new` against stand-in `cargo-verus` programs. That the real
//! runtime's project verifies, and matches stock `cargo verus new`, is
//! checked by the end-to-end tier (`ci/run.sh e2e legacy`).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh directory for one test.
fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sail-new-cli-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir_all(&dir).is_ok());
    dir
}

/// A stand-in `cargo-verus` that writes what `cargo verus new` writes,
/// records its arguments, then runs `tail` (shell text).
fn fake_cargo_verus(root: &Path, tail: &str) -> PathBuf {
    let path = root.join("fake-cargo-verus");
    let script = format!(
        "#!/bin/sh\n\
         echo \"$@\" > '{log}'\n\
         name=\"$4\"\n\
         if [ \"$3\" = --lib ]; then src=lib.rs; else src=main.rs; fi\n\
         mkdir -p \"$name/src\" \"$name/.git\"\n\
         printf '[package]\\nname = \"%s\"\\n\\n[dependencies]\\nvstd = \"=9.9.9\"\\n' \"$name\" > \"$name/Cargo.toml\"\n\
         echo /target > \"$name/.gitignore\"\n\
         echo 'use vstd::prelude::*;' > \"$name/src/$src\"\n\
         {tail}\n",
        log = root.join("args").display()
    );
    assert!(std::fs::write(&path, script).is_ok());
    assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok());
    path
}

/// Runs `sail new ARGS` in `cwd` with `cargo_verus` as Sail's Verus (or
/// with no Verus at all) and the given `PATH`.
fn sail_new(cwd: &Path, cargo_verus: Option<&Path>, path: Option<&str>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sail"));
    command
        .arg("new")
        .args(args)
        .current_dir(cwd)
        .env("SAIL_CACHE_DIR", cwd.join("cache"))
        .env_remove("SAIL_VERUS_BUILD")
        .env_remove("SAIL_CARGO_VERUS");
    if let Some(cargo_verus) = cargo_verus {
        command.env("SAIL_CARGO_VERUS", cargo_verus);
    }
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command
        .output()
        .unwrap_or_else(|e| unreachable!("cannot run sail: {e}"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// No staging directory is left beside the destination.
fn no_staging_left(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .all(|e| !e.file_name().to_string_lossy().starts_with(".sail-new-"))
    })
}

#[test]
fn creates_the_project_at_path_under_the_package_name() {
    let root = scratch("create");
    let fake = fake_cargo_verus(&root, "exit 0");
    let output = sail_new(
        &root,
        Some(&fake),
        None,
        &["--lib", "--name", "proofs", "work/2026"],
    );
    let stdout = text(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        std::fs::read_to_string(root.join("args")).ok().as_deref(),
        Some("verus new --lib proofs\n")
    );
    let project = root.join("work/2026");
    assert!(project.join("src/lib.rs").is_file());
    assert!(project.join("Cargo.toml").is_file());
    assert!(
        stdout.contains("Created Verus library package `proofs` at work/2026 (vstd 9.9.9"),
        "{stdout}"
    );
    assert!(stdout.contains("cd work/2026 && sail verify"), "{stdout}");
    // The development override is announced, as for every command.
    assert!(text(&output.stderr).contains("warning[SAIL0108]"));
    assert!(no_staging_left(&root.join("work")));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fills_an_empty_directory_and_refuses_a_non_empty_one() {
    let root = scratch("existing");
    let fake = fake_cargo_verus(&root, "exit 0");
    assert!(std::fs::create_dir(root.join("empty")).is_ok());
    let output = sail_new(&root, Some(&fake), None, &["empty"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(root.join("empty/src/main.rs").is_file());

    let before = std::fs::read_to_string(root.join("empty/Cargo.toml")).ok();
    let output = sail_new(&root, Some(&fake), None, &["--lib", "empty"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("error[SAIL0130]: sail new: `empty` already exists and is not empty"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("empty/Cargo.toml")).ok(),
        before
    );
    assert!(!root.join("empty/src/lib.rs").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn keeps_no_nested_repository_inside_a_git_work_tree() {
    let root = scratch("nested");
    let fake = fake_cargo_verus(&root, "exit 0");
    assert!(std::fs::create_dir_all(root.join("repo/.git")).is_ok());
    let output = sail_new(&root.join("repo"), Some(&fake), None, &["demo"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(root.join("repo/demo/Cargo.toml").is_file());
    assert!(!root.join("repo/demo/.git").exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// VI-0016: without `git`, `cargo verus new` writes the project and then
/// fails to run `git init`. The project is complete, so `sail new` succeeds.
#[test]
fn succeeds_without_git() {
    let root = scratch("nogit");
    let fake = fake_cargo_verus(
        &root,
        "rm -rf \"$name/.git\"; echo 'Error: No such file or directory (os error 2)' >&2; exit 1",
    );
    // A PATH with the shell tools the stand-in needs, and no git.
    let bin = root.join("bin");
    assert!(std::fs::create_dir(&bin).is_ok());
    for tool in ["mkdir", "rm", "printf", "echo"] {
        if let Some(found) = ["/bin", "/usr/bin"]
            .iter()
            .map(|dir| Path::new(dir).join(tool))
            .find(|p| p.is_file())
        {
            assert!(std::os::unix::fs::symlink(found, bin.join(tool)).is_ok());
        }
    }
    let path = bin.display().to_string();
    let output = sail_new(&root, Some(&fake), Some(&path), &["--lib", "demo"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(root.join("demo/src/lib.rs").is_file());

    // With git on PATH, the same failure is reported and nothing is left.
    if Command::new("git").arg("--version").output().is_ok() {
        let output = sail_new(&root, Some(&fake), None, &["--lib", "other"]);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.contains("error[SAIL0132]: sail new: `cargo verus new`")
                && stderr.contains("No such file or directory"),
            "{stderr}"
        );
        assert!(!root.join("other").exists());
        assert!(no_staging_left(&root));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reports_missing_runtime_bad_names_and_usage() {
    let root = scratch("errors");
    let output = sail_new(&root, None, None, &["demo"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.starts_with("error[SAIL0101]: sail new: Sail's Verus"),
        "{stderr}"
    );
    assert!(!root.join("demo").exists());

    let fake = fake_cargo_verus(&root, "exit 0");
    let output = sail_new(&root, Some(&fake), None, &["2026"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("error[SAIL0131]"));
    assert!(
        !root.join("args").exists(),
        "cargo-verus ran for a bad name"
    );

    let output = sail_new(&root, Some(&fake), None, &["--bin", "--lib", "x"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("error[SAIL0002]"));

    let help = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["new", "--help"])
        .output()
        .unwrap_or_else(|e| unreachable!("cannot run sail: {e}"));
    assert_eq!(help.status.code(), Some(0));
    assert!(text(&help.stdout).contains("sail new [--bin | --lib] [--name NAME] PATH"));
    let _ = std::fs::remove_dir_all(&root);
}

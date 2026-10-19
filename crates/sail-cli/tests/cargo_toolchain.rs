//! `sail verify` runs Cargo with the engine's pinned toolchain unless the
//! caller or the project chose one (ADR 0058). A fake rustup installation and
//! a fake Cargo-Verus record the `RUSTUP_TOOLCHAIN` each verification runs
//! with; no Cargo, Verus or Z3 runs. The setups here are verified as plain
//! Cargo-Verus verifies them (`--tests`, ADR 0046, and partial verification),
//! which select exactly as managed verification does.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn write_exec(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    assert!(std::fs::write(path, text).is_ok());
    assert!(std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_ok());
}

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    log: PathBuf,
    pinned: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = Path::new("/tmp").join(format!("sail-tc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("work/input");
        assert!(std::fs::create_dir_all(project.join("src")).is_ok());
        assert!(
            std::fs::write(
                project.join("Cargo.toml"),
                "[package]\nname = \"input\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[package.metadata.verus]\nverify = true\n\n[workspace]\n"
            )
            .is_ok()
        );
        assert!(std::fs::write(project.join("src/lib.rs"), "pub fn f() {}\n").is_ok());
        let bin = root.join("bin");
        assert!(std::fs::create_dir_all(&bin).is_ok());
        assert!(std::fs::copy(env!("CARGO_BIN_EXE_sail"), bin.join("sail")).is_ok());
        write_exec(&bin.join("sail-z3"), "#!/bin/sh\nexit 2\n");
        assert!(std::fs::write(root.join("z3"), "").is_ok());
        // rustup and its `cargo` proxy, a symbolic link as rustup installs it.
        write_exec(&bin.join("rustup"), "#!/bin/sh\nexit 1\n");
        assert!(std::os::unix::fs::symlink("rustup", bin.join("cargo")).is_ok());
        let log = root.join("toolchains.log");
        write_exec(
            &root.join("cargo-verus"),
            &format!(
                "#!/bin/sh\necho \"${{RUSTUP_TOOLCHAIN-unset}}\" >> '{}'\nexit 0\n",
                log.display()
            ),
        );
        let pinned = sail_verus::toolchain::Pins::embedded().map_or_else(
            |error| unreachable!("{error}"),
            |pins| pins.verus.rust_toolchain,
        );
        let fixture = Self {
            root,
            project,
            log,
            pinned,
        };
        fixture.install(&["cargo", "rust-src", "rustc"]);
        fixture
    }

    /// Installs the pinned toolchain with these components in the fake
    /// rustup home.
    fn install(&self, components: &[&str]) {
        let toolchain = self.root.join("rustup/toolchains").join(format!(
            "{}-{}",
            self.pinned,
            sail_verus::toolchain::host()
        ));
        assert!(std::fs::create_dir_all(toolchain.join("bin")).is_ok());
        assert!(std::fs::create_dir_all(toolchain.join("lib/rustlib")).is_ok());
        assert!(std::fs::write(toolchain.join("bin/cargo"), "").is_ok());
        assert!(
            std::fs::write(
                toolchain.join("lib/rustlib/components"),
                components.join("\n") + "\n"
            )
            .is_ok()
        );
    }

    fn sail(&self, args: &[&str], extra: &[(&str, &str)]) -> String {
        let _ = std::fs::remove_file(&self.log);
        let output = Command::new(self.root.join("bin/sail"))
            .args(["verify"])
            .args(args)
            .current_dir(&self.project)
            .env_clear()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.join("bin").display()),
            )
            .env("HOME", &self.root)
            .env("RUSTUP_HOME", self.root.join("rustup"))
            .env("SAIL_CARGO_VERUS", self.root.join("cargo-verus"))
            .env("VERUS_Z3_PATH", self.root.join("z3"))
            .env("SAIL_CACHE_DIR", self.root.join("cache"))
            // Too long for a socket: no cache daemon starts.
            .env("SAIL_DAEMON_SOCKET", self.root.join("d".repeat(120)))
            .envs(extra.iter().copied())
            .output()
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
}

/// The plain setups `sail verify` verifies itself: an unbound target
/// selection and a partial verification.
const SETUPS: [&[&str]; 2] = [&["--tests"], &["--", "--verify-function", "f"]];

#[test]
fn the_default_rustup_cargo_runs_with_the_pinned_toolchain() {
    let fixture = Fixture::new("pinned");
    for setup in SETUPS {
        assert_eq!(fixture.sail(setup, &[]), fixture.pinned, "{setup:?}");
    }
}

#[test]
fn a_caller_or_project_choice_keeps_the_callers_cargo() {
    let fixture = Fixture::new("caller");
    let cargo = fixture.root.join("bin/cargo").display().to_string();
    for setup in SETUPS {
        assert_eq!(
            fixture.sail(setup, &[("RUSTUP_TOOLCHAIN", "sail-caller")]),
            "sail-caller"
        );
        assert_eq!(fixture.sail(setup, &[("CARGO", &cargo)]), "unset");
        assert_eq!(fixture.sail(setup, &[("RUSTC", "rustc")]), "unset");
    }
    // A toolchain file above the project.
    let file = fixture.root.join("work/rust-toolchain.toml");
    assert!(std::fs::write(&file, "[toolchain]\nchannel = \"1.99.0\"\n").is_ok());
    for setup in SETUPS {
        assert_eq!(fixture.sail(setup, &[]), "unset");
    }
    assert!(std::fs::remove_file(&file).is_ok());
    // A rustup directory override of the project's parent, keyed by the
    // resolved path as `rustup override set` writes it (on macOS `/tmp` is
    // `/private/tmp`).
    let work = fixture.root.join("work");
    let work = work.canonicalize().unwrap_or(work);
    assert!(
        std::fs::write(
            fixture.root.join("rustup/settings.toml"),
            format!(
                "version = \"12\"\n[overrides]\n{:?} = \"1.99.0\"\n",
                work.display().to_string()
            ),
        )
        .is_ok()
    );
    for setup in SETUPS {
        assert_eq!(fixture.sail(setup, &[]), "unset");
    }
}

#[test]
fn a_missing_pin_keeps_the_callers_cargo_and_installs_nothing() {
    let fixture = Fixture::new("missing");
    fixture.install(&["cargo", "rustc"]);
    for setup in SETUPS {
        assert_eq!(fixture.sail(setup, &[]), "unset");
    }
    // Not a rustup proxy: a Cargo of its own.
    let fixture = Fixture::new("standalone");
    let cargo = fixture.root.join("bin/cargo");
    assert!(std::fs::remove_file(&cargo).is_ok());
    write_exec(&cargo, "#!/bin/sh\nexit 1\n");
    for setup in SETUPS {
        assert_eq!(fixture.sail(setup, &[]), "unset");
    }
}

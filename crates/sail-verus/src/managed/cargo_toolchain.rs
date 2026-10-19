//! Which Rust toolchain managed Cargo runs with (ADR 0058).
//!
//! Verus already runs its driver with the engine's pinned toolchain
//! (`rustup run <toolchain>`); only the Cargo layer followed the caller. The
//! shipped vstd proof is keyed to the exact compiler arguments the pinned
//! Cargo emits, so a newer default Cargo silently misses it. Managed builds
//! therefore run Cargo with the pinned toolchain, through `RUSTUP_TOOLCHAIN`
//! on every Cargo, rustc and Cargo-Verus process Sail starts, unless the
//! caller or the project chose a toolchain. Nothing is installed: a missing
//! pinned toolchain keeps the caller's Cargo and the run notes `SAIL3137`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The variable that selects the toolchain of every rustup proxy.
pub const ENV: &str = "RUSTUP_TOOLCHAIN";

/// The toolchain managed Cargo runs with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CargoToolchain {
    /// The caller's own selection: Sail sets nothing.
    #[default]
    Caller,
    /// The engine's pinned toolchain, selected with `RUSTUP_TOOLCHAIN`.
    Pinned(String),
    /// The caller's selection, because the pinned toolchain (named here) is
    /// not installed with `rust-src`; the result notes `SAIL3137`.
    Unavailable(String),
}

impl CargoToolchain {
    /// The `RUSTUP_TOOLCHAIN` value Sail sets, if it sets one.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        match self {
            Self::Pinned(toolchain) => Some(toolchain),
            Self::Caller | Self::Unavailable(_) => None,
        }
    }

    /// Applies the selection to a Cargo, rustc or Cargo-Verus process.
    pub fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        if let Some(toolchain) = self.selected() {
            command.env(ENV, toolchain);
        }
        command
    }

    /// The `SAIL3137` result note for an unavailable pinned toolchain.
    #[must_use]
    pub fn note(&self) -> Option<String> {
        match self {
            Self::Unavailable(toolchain) => Some(format!(
                "SAIL3137: Rust {toolchain} with rust-src is not installed, so this run used the \
                 default Cargo and verifies the shipped vstd proof again; \
                 `rustup toolchain install {toolchain} --profile minimal --component rust-src` \
                 enables the shipped proof"
            )),
            Self::Caller | Self::Pinned(_) => None,
        }
    }
}

/// What the caller's environment and project say about the toolchain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each field is one independent, separately tested observation."
)]
pub struct Observed {
    /// `RUSTUP_TOOLCHAIN` is set.
    pub rustup_toolchain: bool,
    /// `CARGO` names a Cargo explicitly.
    pub explicit_cargo: bool,
    /// `RUSTC` names a compiler explicitly.
    pub explicit_rustc: bool,
    /// The workspace or an ancestor has `rust-toolchain(.toml)`.
    pub toolchain_file: bool,
    /// A rustup directory override covers the workspace, or rustup's
    /// settings could not be read.
    pub directory_override: bool,
    /// The Cargo Sail runs is a rustup proxy, with rustup on `PATH`.
    pub rustup_proxy: bool,
    /// The pinned toolchain is installed with Cargo and `rust-src`.
    pub pinned_installed: bool,
}

/// The selection for these observations; pure.
#[must_use]
pub fn select(pinned: &str, observed: &Observed) -> CargoToolchain {
    if pinned.is_empty()
        || observed.rustup_toolchain
        || observed.explicit_cargo
        || observed.explicit_rustc
        || observed.toolchain_file
        || observed.directory_override
        || !observed.rustup_proxy
    {
        CargoToolchain::Caller
    } else if observed.pinned_installed {
        CargoToolchain::Pinned(pinned.to_owned())
    } else {
        CargoToolchain::Unavailable(pinned.to_owned())
    }
}

/// Selects the toolchain for managed Cargo in `workspace`, where `cargo` is
/// the Cargo Sail would otherwise run. Reads only local files and the
/// environment: no process is started and nothing is downloaded.
#[must_use]
pub fn choose(workspace: &Path, cargo: &Path, pinned: &str) -> CargoToolchain {
    select(pinned, &observe(workspace, cargo, pinned))
}

fn set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

/// The observations [`choose`] selects from.
#[must_use]
pub fn observe(workspace: &Path, cargo: &Path, pinned: &str) -> Observed {
    let home = rustup_home();
    let directories = directories(workspace);
    Observed {
        rustup_toolchain: set(ENV),
        explicit_cargo: set("CARGO"),
        explicit_rustc: set("RUSTC"),
        toolchain_file: directories.iter().any(|directory| {
            ["rust-toolchain", "rust-toolchain.toml"]
                .iter()
                .any(|name| directory.join(name).is_file())
        }),
        directory_override: home
            .as_deref()
            .is_none_or(|home| directory_override(home, &directories)),
        rustup_proxy: on_path("rustup").is_some_and(|rustup| rustup_proxy(cargo, &rustup)),
        pinned_installed: home
            .as_deref()
            .is_some_and(|home| installed(home, pinned, crate::toolchain::host())),
    }
}

fn rustup_home() -> Option<PathBuf> {
    std::env::var_os("RUSTUP_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")))
}

/// The workspace and its ancestors, as given and as resolved: rustup looks
/// for toolchain files and overrides from the current directory upward.
fn directories(workspace: &Path) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for root in [
        std::path::absolute(workspace).ok(),
        workspace.canonicalize().ok(),
    ]
    .into_iter()
    .flatten()
    {
        for directory in root.ancestors() {
            if !directories.iter().any(|known| known == directory) {
                directories.push(directory.to_owned());
            }
        }
    }
    directories
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// Whether rustup's `settings.toml` overrides the toolchain for one of
/// `directories`. Settings that exist but cannot be read count as an
/// override: Sail cannot tell, so it keeps the caller's Cargo.
fn directory_override(home: &Path, directories: &[PathBuf]) -> bool {
    let settings = home.join("settings.toml");
    let text = match std::fs::read_to_string(&settings) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(_) => return true,
    };
    let Ok(value) = text.parse::<toml::Table>() else {
        return true;
    };
    match value.get("overrides") {
        None => false,
        Some(toml::Value::Table(overrides)) => overrides.keys().any(|path| {
            directories
                .iter()
                .any(|directory| directory == Path::new(path))
        }),
        Some(_) => true,
    }
}

/// Whether `cargo` is the rustup proxy `rustup` is: the same file (a
/// symbolic or hard link), as rustup installs its proxies.
fn rustup_proxy(cargo: &Path, rustup: &Path) -> bool {
    let (Ok(cargo), Ok(rustup)) = (cargo.canonicalize(), rustup.canonicalize()) else {
        return false;
    };
    if cargo == rustup {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if let (Ok(a), Ok(b)) = (std::fs::metadata(&cargo), std::fs::metadata(&rustup)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    false
}

/// Whether rustup has `toolchain` (as `<toolchain>-<host>` or exactly that
/// name) installed with Cargo and the `rust-src` component.
fn installed(home: &Path, toolchain: &str, host: &str) -> bool {
    if toolchain.is_empty() {
        return false;
    }
    let toolchains = home.join("toolchains");
    [
        toolchains.join(format!("{toolchain}-{host}")),
        toolchains.join(toolchain),
    ]
    .iter()
    .any(|root| {
        root.join("bin").join(cargo_name()).is_file()
            && std::fs::read_to_string(root.join("lib/rustlib/components"))
                .is_ok_and(|components| components.lines().any(|line| line.trim() == "rust-src"))
    })
}

fn cargo_name() -> &'static str {
    if cfg!(windows) { "cargo.exe" } else { "cargo" }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIN: &str = "1.99.1";

    fn proxy() -> Observed {
        Observed {
            rustup_proxy: true,
            pinned_installed: true,
            ..Observed::default()
        }
    }

    #[test]
    fn the_default_rustup_cargo_selects_the_installed_pin() {
        assert_eq!(select(PIN, &proxy()), CargoToolchain::Pinned(PIN.into()));
    }

    #[test]
    fn every_caller_or_project_choice_keeps_the_callers_cargo() {
        let choices: [fn(&mut Observed); 6] = [
            |o| o.rustup_toolchain = true,
            |o| o.explicit_cargo = true,
            |o| o.explicit_rustc = true,
            |o| o.toolchain_file = true,
            |o| o.directory_override = true,
            |o| o.rustup_proxy = false,
        ];
        for choose in choices {
            for installed in [true, false] {
                let mut observed = proxy();
                observed.pinned_installed = installed;
                choose(&mut observed);
                assert_eq!(
                    select(PIN, &observed),
                    CargoToolchain::Caller,
                    "{observed:?}"
                );
            }
        }
        assert_eq!(select("", &proxy()), CargoToolchain::Caller);
    }

    #[test]
    fn a_missing_pin_keeps_the_callers_cargo_and_notes_how_to_install_it() {
        let observed = Observed {
            pinned_installed: false,
            ..proxy()
        };
        let selection = select(PIN, &observed);
        assert_eq!(selection, CargoToolchain::Unavailable(PIN.into()));
        assert_eq!(selection.selected(), None);
        let note = selection.note().unwrap_or_default();
        assert!(note.starts_with("SAIL3137: "), "{note}");
        assert!(
            note.contains(
                "`rustup toolchain install 1.99.1 --profile minimal --component rust-src`"
            ),
            "{note}"
        );
        assert_eq!(CargoToolchain::Pinned(PIN.into()).note(), None);
        assert_eq!(CargoToolchain::Caller.note(), None);
    }

    #[test]
    fn only_the_pinned_selection_sets_the_variable() {
        let value = |selection: &CargoToolchain| {
            let mut command = Command::new("cargo");
            selection.apply(&mut command);
            command
                .get_envs()
                .find(|(name, _)| *name == ENV)
                .and_then(|(_, value)| value.map(std::ffi::OsStr::to_owned))
        };
        assert_eq!(value(&CargoToolchain::Pinned(PIN.into())), Some(PIN.into()));
        assert_eq!(value(&CargoToolchain::Caller), None);
        assert_eq!(value(&CargoToolchain::Unavailable(PIN.into())), None);
    }

    fn scratch(name: &str) -> std::io::Result<PathBuf> {
        let root = std::env::temp_dir().join(format!(
            "sail-cargo-toolchain-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    #[test]
    fn installation_needs_cargo_and_rust_src() -> std::io::Result<()> {
        let home = scratch("installed")?;
        let host = "x86_64-unknown-linux-gnu";
        let root = home.join("toolchains").join(format!("{PIN}-{host}"));
        assert!(!installed(&home, PIN, host));
        std::fs::create_dir_all(root.join("bin"))?;
        std::fs::create_dir_all(root.join("lib/rustlib"))?;
        std::fs::write(root.join("bin").join(cargo_name()), b"")?;
        std::fs::write(root.join("lib/rustlib/components"), "cargo\nrustc\n")?;
        assert!(!installed(&home, PIN, host), "rust-src is required");
        std::fs::write(
            root.join("lib/rustlib/components"),
            "cargo\nrust-src\nrustc\n",
        )?;
        assert!(installed(&home, PIN, host));
        assert!(!installed(&home, "1.98.0", host));
        assert!(!installed(&home, PIN, "aarch64-apple-darwin"));
        assert!(!installed(&home, "", host));
        std::fs::remove_dir_all(&home)
    }

    #[test]
    fn directory_overrides_cover_the_workspace_and_its_ancestors() -> std::io::Result<()> {
        let home = scratch("override")?;
        let workspace = home.join("projects/demo");
        std::fs::create_dir_all(&workspace)?;
        let directories = directories(&workspace);
        assert!(!directory_override(&home, &directories), "no settings");
        std::fs::write(
            home.join("settings.toml"),
            "version = \"12\"\nprofile = \"default\"\n\n[overrides]\n",
        )?;
        assert!(!directory_override(&home, &directories));
        let set = |path: &Path| {
            std::fs::write(
                home.join("settings.toml"),
                format!(
                    "version = \"12\"\n[overrides]\n{:?} = \"1.99.0-x86_64-unknown-linux-gnu\"\n",
                    path.to_string_lossy()
                ),
            )
        };
        set(&home.join("elsewhere"))?;
        assert!(!directory_override(&home, &directories));
        set(&home.join("projects"))?;
        assert!(directory_override(&home, &directories), "an ancestor");
        set(&workspace)?;
        assert!(directory_override(&home, &directories), "the workspace");
        std::fs::write(home.join("settings.toml"), "not toml [")?;
        assert!(
            directory_override(&home, &directories),
            "unreadable settings keep the caller's Cargo"
        );
        std::fs::remove_dir_all(&home)
    }

    #[cfg(unix)]
    #[test]
    fn rustup_proxies_are_links_to_rustup() -> std::io::Result<()> {
        let bin = scratch("proxy")?;
        let rustup = bin.join("rustup");
        std::fs::write(&rustup, b"rustup")?;
        std::os::unix::fs::symlink("rustup", bin.join("cargo"))?;
        std::fs::hard_link(&rustup, bin.join("hard-cargo"))?;
        std::fs::write(bin.join("plain-cargo"), b"rustup")?;
        assert!(rustup_proxy(&bin.join("cargo"), &rustup));
        assert!(rustup_proxy(&bin.join("hard-cargo"), &rustup));
        assert!(
            !rustup_proxy(&bin.join("plain-cargo"), &rustup),
            "same bytes, another file"
        );
        assert!(!rustup_proxy(&bin.join("missing"), &rustup));
        std::fs::remove_dir_all(&bin)
    }

    #[test]
    fn toolchain_files_are_found_in_ancestors() -> std::io::Result<()> {
        let root = scratch("file")?;
        let workspace = root.join("a/b");
        std::fs::create_dir_all(&workspace)?;
        let found =
            |workspace: &Path| observe(workspace, Path::new("/not-cargo"), PIN).toolchain_file;
        // A host whose temporary directory sits under a toolchain file
        // cannot show the difference.
        if found(&workspace) {
            return std::fs::remove_dir_all(&root);
        }
        std::fs::write(root.join("a/rust-toolchain.toml"), "[toolchain]\n")?;
        assert!(found(&workspace));
        std::fs::remove_file(root.join("a/rust-toolchain.toml"))?;
        std::fs::write(workspace.join("rust-toolchain"), "1.99.0\n")?;
        assert!(found(&workspace));
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }
}

//! Repository automation. Run as `cargo xtask <command>`.
#![forbid(unsafe_code)]

mod error_codes;
mod layering;
mod verus;
mod verus_source;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <command>

commands:
    install-hooks   copy tracked git hooks into the shared hooks directory
    check-layering  verify internal crate dependencies follow the charter DAG
    check-error-codes
                    verify docs/errors.md lists exactly the SAIL codes in use
    protect-main    apply branch protection to main on GitHub (PR mode)
    build-verus-engine [--force] [--keep-intermediates]
                    build the isolated optional interactive proof engine
    fetch-verus     install the pinned Verus release (corpus/toolchain.toml)
    build-verus-source [--force] [--keep-intermediates] [--fork] [--features=singular]
                    build the pinned Verus commit from source, with its
                    rust_verify_test binaries (deep tier; ADR 0019)
";

type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1);
    let result = match cmd.as_deref() {
        Some("install-hooks") => install_hooks(),
        Some("check-layering") => check_layering(),
        Some("check-error-codes") => error_codes::check(&workspace_root()),
        Some("protect-main") => protect_main(),
        Some("fetch-verus") => verus::fetch(),
        Some("build-verus-engine") => {
            let mut rest: Vec<String> = std::env::args().skip(2).collect();
            rest.push("--interactive-engine".to_owned());
            verus_source::build(&rest)
        }
        Some("build-verus-source") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            verus_source::build(&rest)
        }
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    // xtask lives one level below the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn run_capture(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .output()
        .map_err(|e| format!("failed to run {cmd:?}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{cmd:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("{cmd:?} produced non-UTF-8 output: {e}"))
}

/// Copies `.githooks/*` into the *common* git directory's hooks folder, so a
/// single install covers every worktree. Deliberately avoids
/// `core.hooksPath`, which would silently disable hooks on branches that
/// predate the tracked hook files.
fn install_hooks() -> Result<()> {
    let root = workspace_root();
    let common = run_capture(Command::new("git").arg("-C").arg(&root).args([
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
    ]))?;
    let hooks_dir = PathBuf::from(common.trim()).join("hooks");
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|e| format!("cannot create {}: {e}", hooks_dir.display()))?;
    let source = root.join(".githooks");
    let entries =
        std::fs::read_dir(&source).map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", source.display()))?;
        let from = entry.path();
        let to = hooks_dir.join(entry.file_name());
        std::fs::copy(&from, &to)
            .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))?;
        set_executable(&to)?;
        println!("installed {}", to.display());
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("cannot chmod {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Checks every workspace member's direct `sail-*` dependencies (normal and
/// build; dev-dependencies are exempt) against [`layering::ALLOWED`].
fn check_layering() -> Result<()> {
    let root = workspace_root();
    let raw = run_capture(
        Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
            .current_dir(&root)
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--offline",
            ]),
    )?;
    let meta: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    let allowed: BTreeMap<&str, BTreeSet<&str>> = layering::ALLOWED
        .iter()
        .map(|(name, deps)| (*name, deps.iter().copied().collect()))
        .collect();

    let packages = meta["packages"]
        .as_array()
        .ok_or("cargo metadata has no packages array")?;
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default();
        if !name.starts_with("sail-") {
            continue;
        }
        seen.insert(name.to_owned());
        let Some(permitted) = allowed.get(name) else {
            violations.push(format!(
                "{name}: not listed in xtask/src/layering.rs (add it with an ADR)"
            ));
            continue;
        };
        for dep in package["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            let is_dev = dep["kind"].as_str() == Some("dev");
            if dep_name.starts_with("sail-") && !is_dev && !permitted.contains(dep_name) {
                violations.push(format!("{name} -> {dep_name} is not a permitted edge"));
            }
        }
    }
    for name in allowed.keys() {
        if !seen.contains(*name) {
            violations.push(format!(
                "{name}: listed in layering table but not in the workspace"
            ));
        }
    }
    if violations.is_empty() {
        println!("layering ok ({} crates)", seen.len());
        Ok(())
    } else {
        Err(format!(
            "layering violations:\n  {}",
            violations.join("\n  ")
        ))
    }
}

/// Applies the org-standard branch protection to `main`: PRs required,
/// required fast checks and their completion guard, admins
/// included, no force pushes. Run once when the project switches to
/// PR-focused development (see AGENTS.md).
fn protect_main() -> Result<()> {
    let body = serde_json::json!({
        "required_status_checks": {
            "strict": true,
            "contexts": [
                "No AI authorship markers",
                "Workspace gate",
                "Solver tests",
                "End to end",
                "Conformance",
                "CI completion"
            ]
        },
        "enforce_admins": true,
        "required_pull_request_reviews": {
            "required_approving_review_count": 0,
            "dismiss_stale_reviews": false,
            "require_code_owner_reviews": false
        },
        "restrictions": null,
        "allow_force_pushes": false,
        "allow_deletions": false
    });
    let slug = github_slug()?;
    let endpoint = format!("repos/{slug}/branches/main/protection");
    let mut child = Command::new("gh")
        .args(["api", "--method", "PUT", &endpoint, "--input", "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run gh: {e}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write as _;
        stdin
            .write_all(body.to_string().as_bytes())
            .map_err(|e| format!("failed to write to gh: {e}"))?;
    }
    let status = child
        .wait()
        .map_err(|e| format!("gh did not finish: {e}"))?;
    if status.success() {
        println!("branch protection applied to {slug} main");
        Ok(())
    } else {
        Err(format!("gh api exited with {status}"))
    }
}

/// The `owner/repo` slug of the `origin` remote, so the command cannot target
/// a different repository than the clone it runs in.
fn github_slug() -> Result<String> {
    let url = run_capture(
        Command::new("git")
            .arg("-C")
            .arg(workspace_root())
            .args(["remote", "get-url", "origin"]),
    )?;
    parse_github_slug(url.trim()).ok_or_else(|| format!("origin is not a GitHub remote: {url}"))
}

fn parse_github_slug(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))?;
    let slug = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = slug.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) if !owner.is_empty() && !repo.is_empty() => {
            Some(format!("{owner}/{repo}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_github_slug;

    #[test]
    fn parses_common_remote_forms() {
        for url in [
            "https://github.com/Chelis-Lang/sail.git",
            "https://github.com/Chelis-Lang/sail",
            "git@github.com:Chelis-Lang/sail.git",
            "ssh://git@github.com/Chelis-Lang/sail.git",
        ] {
            assert_eq!(
                parse_github_slug(url).as_deref(),
                Some("Chelis-Lang/sail"),
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_non_github_remotes() {
        assert_eq!(parse_github_slug("https://gitlab.com/a/b.git"), None);
        assert_eq!(parse_github_slug("https://github.com/only-owner"), None);
    }
}

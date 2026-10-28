// Real Cargo artifacts and strict cleanup postconditions for compiler-origin retirement.
type OriginCleanResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct OriginCleanFixture {
    root: PathBuf,
    config: Config,
    metadata: Value,
    target: PathBuf,
    origins: PathBuf,
}

impl Drop for OriginCleanFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl OriginCleanFixture {
    fn new(label: &str) -> OriginCleanResult<Self> {
        let root =
            std::env::temp_dir().join(format!("sail-origin-clean-{label}-{}", std::process::id()));
        let mut config = compiler_fixture(&root)?;
        config.cargo = resolve_program(std::ffi::OsStr::new("cargo"), &root)?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=['one','two']\nresolver='3'\n",
        )?;
        for (directory, name) in [("one", "origin-one"), ("two", "origin-two")] {
            std::fs::create_dir_all(root.join(directory).join("src"))?;
            std::fs::write(
                root.join(directory).join("Cargo.toml"),
                format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2024'\n"),
            )?;
            std::fs::write(
                root.join(directory).join("src/lib.rs"),
                "pub fn value() -> u32 { 1 }\n",
            )?;
        }
        let output = Command::new(&config.cargo)
            .current_dir(&root)
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata = serde_json::from_slice(&output.stdout)?;
        let target = root.join("family/target");
        let origins = root.join("family/compiler-read-origins");
        super::super::compiler_origin::initialized(&origins)?;
        Ok(Self {
            root,
            config,
            metadata,
            target,
            origins,
        })
    }

    fn package(&self, name: &str) -> OriginCleanResult<&Value> {
        self.metadata["packages"]
            .as_array()
            .ok_or("packages")?
            .iter()
            .find(|package| package["name"] == name)
            .ok_or_else(|| "package".into())
    }

    fn mark(&self, name: &str, variant: &str) -> OriginCleanResult<PathBuf> {
        let package = self.package(name)?;
        let identity = super::super::receipts::TargetIdentity {
            manifest: PathBuf::from(package["manifest_path"].as_str().ok_or("manifest")?),
            name: name.replace('-', "_"),
            crate_type: "lib".into(),
            rustc_args_sha256: variant.into(),
            cargo_name: None,
        };
        let path = self
            .origins
            .join("pending")
            .join(format!("{}.json", identity.key()));
        super::super::files::json(
            &path,
            &serde_json::json!({"version":2,"package_id":package["id"],"identity":identity}),
        )?;
        Ok(path)
    }

    fn build(&self, target: &Path, args: &[&str]) -> OriginCleanResult {
        let output = Command::new(&self.config.cargo)
            .current_dir(&self.root)
            .args(["check", "--quiet", "--workspace", "--target-dir"])
            .arg(target)
            .args(args)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    fn artifacts(&self, triple: &str) -> OriginCleanResult<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for root in [&self.target, &self.target.join("verus-partial")] {
            for relative in [
                "debug/deps".to_owned(),
                "release/deps".to_owned(),
                format!("{triple}/debug/deps"),
            ] {
                for entry in std::fs::read_dir(root.join(relative))? {
                    let path = entry?.path();
                    if path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("liborigin_one-"))
                        && path
                            .extension()
                            .is_some_and(|extension| extension == "rmeta")
                    {
                        paths.push(path);
                    }
                }
            }
        }
        Ok(paths)
    }

    fn clean(&mut self, args: &[&str]) -> crate::Result<()> {
        self.config.args = args.iter().map(|arg| (*arg).to_owned()).collect();
        let package = self.metadata["packages"]
            .as_array()
            .and_then(|all| all.iter().find(|p| p["name"] == "origin-one"))
            .and_then(|p| p["id"].as_str())
            .ok_or_else(|| Error::new("SAIL3111", "test package"))?
            .to_owned();
        let super::super::compiler_origin::Cleanup::Packages(plan) =
            super::super::compiler_origin::cleanup(&self.origins, &self.metadata)
        else {
            return Err(Error::new("SAIL3111", "test cleanup plan"));
        };
        assert_eq!(plan.packages(), [package]);
        clean_origin_packages(
            &self.config,
            &self.target,
            &self.origins,
            &plan,
            &self.metadata,
        )
    }

    fn config(&self, text: &str) -> OriginCleanResult {
        std::fs::create_dir_all(self.root.join(".cargo"))?;
        std::fs::write(self.root.join(".cargo/config.toml"), text)?;
        Ok(())
    }

    fn wrapper(&mut self, behavior: &str) -> OriginCleanResult {
        use std::os::unix::fs::PermissionsExt as _;
        let wrapper = self.root.join("cleanup-cargo");
        let cargo = format!(
            "'{}'",
            self.config.cargo.to_string_lossy().replace('\'', "'\\''")
        );
        std::fs::write(
            &wrapper,
            format!("#!/bin/sh\n{behavior}\nexec {cargo} \"$@\"\n"),
        )?;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
        self.config.cargo = wrapper;
        Ok(())
    }
}

#[test]
fn origin_package_retirement_requires_all_profile_and_target_fingerprints_gone() -> OriginCleanResult
{
    let mut f = OriginCleanFixture::new("variants")?;
    let host = std::env::consts::ARCH;
    let output = Command::new(&f.config.cargo).arg("-Vv").output()?;
    let version = String::from_utf8(output.stdout)?;
    let triple = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("host")?;
    assert!(triple.starts_with(host));
    let roots = [f.target.clone(), f.target.join("verus-partial")];
    let variants = [Vec::new(), vec!["--release"], vec!["--target", triple]];
    for root in &roots {
        for args in &variants {
            f.build(root, args)?;
        }
    }
    let artifacts = f.artifacts(triple)?;
    assert_eq!(
        artifacts.len(),
        6,
        "every layout has a real compiler artifact"
    );
    let old = f.mark("origin-one", "old-arguments")?;
    let new = f.mark("origin-one", "new-arguments")?;
    f.config(&format!("[build]\ntarget='{triple}'\n"))?;
    assert!(
        f.clean(&[]).is_err(),
        "configured-target cleaning must not retire host/release facts"
    );
    assert!(old.is_file() && new.is_file());
    std::fs::remove_file(f.root.join(".cargo/config.toml"))?;
    assert!(f.clean(&[]).is_err(), "release fingerprints still exist");
    assert!(old.is_file() && new.is_file());
    f.clean(&["--profile", "release"])?;
    assert!(!old.exists() && !new.exists());
    assert!(
        artifacts.iter().all(|path| !path.exists()),
        "all selected actual artifacts are gone"
    );
    for root in &roots {
        origin_fingerprints_absent(root, &BTreeSet::from(["origin-one"]), 0)?;
        assert!(origin_fingerprints_absent(root, &BTreeSet::from(["origin-two"]), 0).is_err());
    }
    Ok(())
}

#[test]
fn origin_cleanup_failure_and_false_success_preserve_marker_authority() -> OriginCleanResult {
    for (label, behavior) in [
        (
            "partial",
            "if [ \"$1\" = clean ]; then case \"$*\" in *verus-partial*) exit 1;; esac; fi",
        ),
        ("false-success", "if [ \"$1\" = clean ]; then exit 0; fi"),
    ] {
        let mut f = OriginCleanFixture::new(label)?;
        f.build(&f.target, &[])?;
        f.build(&f.target.join("verus-partial"), &[])?;
        let marker = f.mark("origin-one", label)?;
        let before = std::fs::read(&marker)?;
        f.wrapper(behavior)?;
        assert!(f.clean(&[]).is_err());
        assert_eq!(
            std::fs::read(marker)?,
            before,
            "no fact retires between successful first and failed second clean"
        );
        if label == "false-success" {
            assert!(
                clean_compiler_family(&f.config, &f.target).is_err(),
                "whole-family migration checks physical removal too"
            );
        }
    }
    Ok(())
}

#[test]
fn origin_cleanup_external_build_directory_uses_existing_unbound_fallback() -> OriginCleanResult {
    let mut f = OriginCleanFixture::new("external")?;
    f.build(&f.target, &[])?;
    let marker = f.mark("origin-one", "external")?;
    let external = f.root.join("external-build");
    std::fs::create_dir_all(&external)?;
    std::fs::write(external.join("untouched"), "caller-owned")?;
    f.config(&format!(
        "[build]\nbuild-dir={}\n",
        serde_json::to_string(&external.to_string_lossy())?
    ))?;
    assert_eq!(f.clean(&[]).err().map(|error| error.code), Some("SAIL3112"));
    assert!(marker.is_file());
    assert_eq!(
        std::fs::read_to_string(external.join("untouched"))?,
        "caller-owned"
    );
    assert_eq!(
        clean_compiler_family(&f.config, &f.target)
            .err()
            .map(|error| error.code),
        Some("SAIL3112")
    );
    Ok(())
}

#[test]
fn origin_fingerprint_postcheck_refuses_unreadable_or_linked_authority() -> OriginCleanResult {
    let f = OriginCleanFixture::new("postcheck")?;
    let directory = f.target.join("debug/.fingerprint");
    std::fs::create_dir_all(directory.parent().ok_or("parent")?)?;
    std::fs::write(&directory, "not a readable fingerprint directory")?;
    assert!(origin_fingerprints_absent(&f.target, &BTreeSet::from(["origin-one"]), 0).is_err());
    std::fs::remove_file(&directory)?;
    std::os::unix::fs::symlink(f.root.join("absent"), &directory)?;
    assert!(origin_fingerprints_absent(&f.target, &BTreeSet::from(["origin-one"]), 0).is_err());
    Ok(())
}

#[test]
fn origin_cleanup_never_follows_a_linked_owned_target_root() -> OriginCleanResult {
    for (label, focused) in [("linked-normal", false), ("linked-focused", true)] {
        let mut f = OriginCleanFixture::new(label)?;
        f.build(&f.target, &[])?;
        f.build(&f.target.join("verus-partial"), &[])?;
        let marker = f.mark("origin-one", label)?;
        let root = if focused {
            f.target.join("verus-partial")
        } else {
            f.target.clone()
        };
        let foreign = f.root.join("foreign-artifacts");
        std::fs::rename(&root, &foreign)?;
        std::fs::write(foreign.join("sentinel"), "foreign owner")?;
        std::os::unix::fs::symlink(&foreign, &root)?;
        assert_eq!(f.clean(&[]).err().map(|error| error.code), Some("SAIL3112"));
        assert_eq!(
            std::fs::read_to_string(foreign.join("sentinel"))?,
            "foreign owner"
        );
        assert!(marker.is_file());
        assert!(origin_fingerprints_absent(&foreign, &BTreeSet::from(["origin-one"]), 0).is_err());
        assert_eq!(
            clean_compiler_family(&f.config, &f.target)
                .err()
                .map(|error| error.code),
            Some("SAIL3112")
        );
    }
    Ok(())
}

#[test]
fn origin_cleanup_forwards_only_cargo_scope_before_the_verus_separator() {
    let args = [
        "--manifest-path",
        "Cargo.toml",
        "--release",
        "-r",
        "--profile",
        "custom",
        "--profile=release",
        "--target",
        "host-tuple",
        "--target=another-triple",
        "--manifest-path=other.toml",
        "--",
        "--profile",
        "wrong",
        "--target",
        "wrong",
        "--manifest-path",
        "wrong",
    ];
    let args: Vec<_> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let mut scoped = Command::new("cargo");
    cargo_clean_arguments(&mut scoped, &args, true);
    assert_eq!(
        scoped.get_args().collect::<Vec<_>>(),
        args[..11]
            .iter()
            .map(std::ffi::OsStr::new)
            .collect::<Vec<_>>()
    );
    let mut whole = Command::new("cargo");
    cargo_clean_arguments(&mut whole, &args, false);
    assert_eq!(
        whole.get_args().collect::<Vec<_>>(),
        [
            "--manifest-path",
            "Cargo.toml",
            "--manifest-path=other.toml"
        ]
    );
}

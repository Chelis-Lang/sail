//! Independent ordinary-script inputs during a missing-generation bootstrap.
//!
//! This optional transaction never changes the proof guard or its outcome.
//! Unsupported or incomplete script roles leave the original cleaning path.

#[cfg(target_os = "linux")]
use super::super::{BuildInput, effective_value};
#[cfg(target_os = "linux")]
use crate::managed::{lease::ReadLease, precise::Observer};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// One invocation-owned optional ordinary-output transaction; no sharing or tasks.
pub(in crate::managed) struct Bootstrap {
    #[cfg(target_os = "linux")]
    observer: Observer,
    #[cfg(target_os = "linux")]
    lease: ReadLease,
    #[cfg(target_os = "linux")]
    files: Vec<PathBuf>,
    #[cfg(target_os = "linux")]
    roots: Vec<PathBuf>,
    #[cfg(target_os = "linux")]
    excluded: Vec<PathBuf>,
    #[cfg(target_os = "linux")]
    environment: std::collections::BTreeMap<String, String>,
    #[cfg(target_os = "linux")]
    hashes: Vec<String>,
    #[cfg(target_os = "linux")]
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Bootstrap {
    fn eligible_package(package: &Value) -> bool {
        package["dependencies"]
            .as_array()
            .is_some_and(|dependencies| {
                dependencies.iter().all(|dependency| {
                    dependency["kind"].is_null()
                        || dependency["kind"] == "normal"
                        || dependency["kind"] == "dev"
                })
            })
            && package["targets"].as_array().is_some_and(|targets| {
                targets
                    .iter()
                    .filter(|target| {
                        target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.len() == 1 && kinds[0] == "custom-build")
                    })
                    .count()
                    == 1
            })
    }

    /// Freshly compile supported scripts again when a guarded invocation retries.
    /// The final journal alone must contain their fresh=false compilation roles.
    pub(in crate::managed) fn retry_packages(
        data: &Value,
        mut missing: Vec<String>,
    ) -> Vec<String> {
        if let Some(packages) = data["packages"].as_array().filter(|p| p.len() <= 128) {
            for package in packages.iter().filter(|p| Self::eligible_package(p)) {
                if let Some(id) = package["id"].as_str()
                    && !super::super::never_scheduled(data, id)
                    && !missing.iter().any(|name| name == id)
                {
                    missing.push(id.to_owned());
                }
            }
        }
        missing
    }

    /// Refuse command-line overrides of the pinned Cargo compilation cwd rule.
    pub(in crate::managed) fn supported_arguments(args: &[String]) -> bool {
        !args
            .iter()
            .any(|arg| arg.starts_with("-Z") || arg == "--config" || arg.starts_with("--config="))
    }

    /// Avoid acquiring or hashing an owner when no supported script can publish.
    pub(in crate::managed) fn eligible_workload(data: &Value) -> bool {
        data["packages"].as_array().is_some_and(|packages| {
            packages.len() <= 128
                && packages.iter().any(|package| {
                    Self::eligible_package(package)
                        && package["id"]
                            .as_str()
                            .is_some_and(|id| !super::super::never_scheduled(data, id))
                })
        })
    }

    /// Acquire independent native/kernel coverage before any script cleaning.
    /// Missing-generation declaration paths are observation roots, not output authority.
    #[cfg(target_os = "linux")]
    #[allow(clippy::too_many_lines)] // Keep independent acquisition/configuration/final barriers in audit order.
    pub(in crate::managed) fn begin(
        data: &Value,
        extras: &[PathBuf],
        runtime: &[PathBuf],
    ) -> Option<Self> {
        use std::sync::{Arc, atomic::AtomicBool};
        use std::time::Instant;
        let packages = data["packages"].as_array()?;
        (packages.len() <= 128).then_some(())?;
        Self::eligible_workload(data).then_some(())?;
        // Unrepresentable effective inputs cannot be certified as absent.
        let environment = environment()?;
        (!environment.keys().any(|key| {
            key.starts_with("CARGO_UNSTABLE_")
                || key == "__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS"
        }))
        .then_some(())?;
        let workspace = Path::new(data["workspace_root"].as_str()?);
        (workspace.is_absolute()
            && workspace.as_os_str().len() <= 4096
            && workspace.canonicalize().ok()?.as_path() == workspace)
            .then_some(())?;
        let invocation = Path::new(data["sail_invocation_directory"].as_str()?);
        // A virtual workspace's manifest and lock are outside member package
        // roots. Cover these exact compiler configuration inputs separately.
        let mut roots = vec![workspace.join("Cargo.toml"), workspace.join("Cargo.lock")];
        for package in packages {
            roots.push(
                Path::new(package["manifest_path"].as_str()?)
                    .parent()?
                    .to_owned(),
            );
        }
        (extras.len() <= 4096 && runtime.len() <= 4096).then_some(())?;
        roots.extend_from_slice(extras);
        roots.extend_from_slice(runtime);
        let home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
        let configurations =
            crate::managed::input::configuration_paths(invocation, home.as_deref());
        roots.extend_from_slice(&configurations);
        roots.extend(
            [
                "/Cargo.toml",
                "/Cargo.lock",
                "/rust-toolchain",
                "/rust-toolchain.toml",
            ]
            .map(PathBuf::from),
        );
        roots.sort();
        roots.dedup();
        (roots.len() <= 4096 && roots.iter().all(|p| p.as_os_str().len() <= 4096)).then_some(())?;
        let excluded: Vec<_> = ["sail_cache_directory", "target_directory"]
            .into_iter()
            .filter_map(|key| data[key].as_str())
            .map(|p| invocation.join(p))
            .collect();
        let stop = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let mut observer =
            Observer::begin_bounded_with(&roots, &excluded, Arc::clone(&stop), started)?;
        let files = observer.files();
        (files.len() <= 4096).then_some(())?;
        let mut lease = ReadLease::acquire_bounded(
            &files,
            &|| {
                stop.load(std::sync::atomic::Ordering::Acquire)
                    || started.elapsed() >= std::time::Duration::from_secs(120)
            },
            Arc::clone(&stop),
            started,
        )?;
        let retained = observer
            .retained_bytes()?
            .checked_add(lease.retained_bytes()?)?;
        (retained <= 32 * 1024 * 1024).then_some(())?;
        let hashes = hashes(&files, &stop)?;
        // RUSTC_BOOTSTRAP=1 also enables Cargo's dev channel. Refuse its
        // unstable config table instead of assuming root-dir cannot be active.
        for path in configurations {
            if stop.load(std::sync::atomic::Ordering::Acquire)
                || started.elapsed() >= std::time::Duration::from_secs(120)
            {
                return None;
            }
            if !matches!(path.file_name()?.to_str()?, "config" | "config.toml") {
                continue;
            }
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata.is_file().then_some(())?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return None,
            }
            let text = super::bytes(&path)?;
            let config: toml::Value = toml::from_str(std::str::from_utf8(&text).ok()?).ok()?;
            (config.get("unstable").is_none() && config.get("include").is_none()).then_some(())?;
        }
        (observer.clean() && lease.clean()).then_some(())?;
        let owner = Self {
            observer,
            lease,
            files,
            roots,
            excluded,
            environment,
            hashes,
            stop,
        };
        (owner.retained_bytes()? <= 40 * 1024 * 1024).then_some(owner)
    }

    #[cfg(not(target_os = "linux"))]
    pub(in crate::managed) fn begin(_: &Value, _: &[PathBuf], _: &[PathBuf]) -> Option<Self> {
        None
    }

    #[cfg(target_os = "linux")]
    fn retained_bytes(&self) -> Option<usize> {
        // Exhaustive destructuring makes a new owned field an accounting decision.
        let Self {
            observer,
            lease,
            files,
            roots,
            excluded,
            environment,
            hashes,
            stop: _,
        } = self;
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(64)?
            .checked_add(observer.retained_bytes()?)?
            .checked_add(lease.retained_bytes()?)?
            .checked_add(crate::managed::retained::vector(files)?)?
            .checked_add(crate::managed::retained::vector(roots)?)?
            .checked_add(crate::managed::retained::vector(excluded)?)?
            .checked_add(crate::managed::retained::vector(hashes)?)?
            .checked_add(crate::managed::retained::tree(environment.len())?)?;
        for path in files.iter().chain(roots).chain(excluded) {
            bytes = bytes.checked_add(path.capacity())?;
        }
        for hash in hashes {
            bytes = bytes.checked_add(hash.capacity())?;
        }
        for (key, value) in environment {
            bytes = bytes
                .checked_add(key.capacity())?
                .checked_add(value.capacity())?;
        }
        Some(bytes)
    }

    /// Publish ordinary contracts only; original proof results and receipts remain untouched.
    #[cfg(target_os = "linux")]
    pub(in crate::managed) fn publish(
        &mut self,
        export: &Path,
        compiler: &str,
        target: &Path,
        local: &Path,
        data: &Value,
        artifacts: &Path,
    ) {
        let ready = (|| -> Option<Vec<BuildInput>> {
            let records: Vec<BuildInput> = serde_json::from_slice(&super::bytes(local)?).ok()?;
            let journal = super::bytes(artifacts)?;
            let journal = std::str::from_utf8(&journal).ok()?;
            let mut artifacts = Vec::new();
            let mut observed_bytes = 0usize;
            for (index, line) in journal.lines().enumerate() {
                (index < 4096 && line.len() <= 32768).then_some(())?;
                let artifact: Value = serde_json::from_str(line).ok()?;
                if artifact["reason"] == "build-script-executed"
                    || (artifact["reason"] == "compiler-artifact"
                        && artifact["target"]["kind"]
                            .as_array()
                            .is_some_and(|k| k.iter().any(|k| k == "custom-build")))
                {
                    observed_bytes = observed_bytes.checked_add(line.len())?;
                    (observed_bytes <= 256 * 1024 && artifacts.len() < 256).then_some(())?;
                    artifacts.push(artifact);
                }
            }
            (records.len() <= 128 && !records.is_empty()).then_some(())?;
            (!self.stop.load(std::sync::atomic::Ordering::Acquire)).then_some(())?;
            let mut packages = std::collections::BTreeSet::new();
            let checked = std::time::Instant::now();
            for record in &records {
                (!self.stop.load(std::sync::atomic::Ordering::Acquire)
                    && checked.elapsed() < std::time::Duration::from_secs(30))
                .then_some(())?;
                // Multiple host/target units and build dependencies require a separate audit.
                packages.insert(&record.package_id).then_some(())?;
                self.record(record, data, target, &artifacts)?;
            }
            (self.hashes == hashes(&self.files, &self.stop)?).then_some(())?;
            // Re-read each sealed output contract before the final input barriers.
            // No input/output read occurs after those barriers and before emission.
            let output_check = std::time::Instant::now();
            for record in &records {
                (!self.stop.load(std::sync::atomic::Ordering::Acquire)
                    && output_check.elapsed() < std::time::Duration::from_secs(30)
                    && super::output_matches(record, target))
                .then_some(())?;
            }
            (self.environment == environment()? && self.observer.clean() && self.lease.clean())
                .then_some(())?;
            Some(records)
        })();
        if let Some(records) = ready {
            // Publish exactly the checked contracts without any post-barrier input read.
            super::publish_records(export, compiler, records);
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(in crate::managed) fn publish(
        &mut self,
        _: &Path,
        _: &str,
        _: &Path,
        _: &Path,
        _: &Value,
        _: &Path,
    ) {
    }

    #[cfg(target_os = "linux")]
    fn covered(&self, path: &Path) -> Option<()> {
        let canonical = path.canonicalize().ok()?;
        // Newly learnt aliases were not watched before invocation. Canonical
        // membership alone cannot certify their invocation-time resolution.
        (path == canonical && canonical.is_file()).then_some(())?;
        self.files.binary_search(&canonical).ok().map(|_| ())
    }

    #[cfg(target_os = "linux")]
    fn record_digest(&self, record: &BuildInput) -> Option<String> {
        use sha2::{Digest as _, Sha256};
        let home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
        let mut files = std::collections::BTreeMap::new();
        for path in &record.paths {
            let canonical = path.canonicalize().ok()?;
            let i = self.files.binary_search(&canonical).ok()?;
            files.insert(canonical, self.hashes.get(i)?);
        }
        // Match the original snapshot_paths encoding, including any root-level
        // and Cargo-home configuration files. Every byte came from the same
        // continuously guarded pre-invocation inventory.
        for path in crate::managed::input::configuration_paths(Path::new("/"), home.as_deref())
            .into_iter()
            .chain(
                [
                    "/Cargo.toml",
                    "/Cargo.lock",
                    "/rust-toolchain",
                    "/rust-toolchain.toml",
                ]
                .map(PathBuf::from),
            )
            .filter(|p| p.is_file())
        {
            let i = self.files.binary_search(&path.canonicalize().ok()?).ok()?;
            files.insert(path, self.hashes.get(i)?);
        }
        let mut digest = Sha256::new();
        for (path, hash) in files {
            let path = path.as_os_str().as_encoded_bytes();
            digest.update(u64::try_from(path.len()).ok()?.to_le_bytes());
            digest.update(path);
            digest.update(hash.as_bytes());
        }
        let files = format!("{:x}", digest.finalize());
        let env: std::collections::BTreeMap<_, _> = record
            .env
            .keys()
            .map(|key| (key, effective_value(key, Some(&self.environment))))
            .collect();
        Some(crate::managed::files::digest(&super::encode(&(
            files, env,
        ))?))
    }

    #[cfg(target_os = "linux")]
    pub(in crate::managed) fn runtime_matches(&self, cache: &Path, context: &str) -> bool {
        let check = || -> Option<()> {
            let directory = cache.join("runtime-identities");
            let sha: String =
                serde_json::from_slice(&super::bytes(&directory.join(format!("{context}.link")))?)
                    .ok()?;
            (sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then_some(())?;
            let records: Vec<(String, String)> =
                serde_json::from_slice(&super::bytes(&directory.join(format!("{sha}.json")))?)
                    .ok()?;
            (records.len() <= 4096
                && crate::managed::files::digest(&super::encode(&records)?) == sha)
                .then_some(())?;
            for (path, hash) in records {
                let path = Path::new(&path).canonicalize().ok()?;
                let i = self.files.binary_search(&path).ok()?;
                (self.hashes.get(i)? == &hash).then_some(())?;
            }
            Some(())
        };
        check().is_some()
    }

    #[cfg(not(target_os = "linux"))]
    pub(in crate::managed) fn runtime_matches(&self, _: &Path, _: &str) -> bool {
        false
    }

    #[cfg(target_os = "linux")]
    #[allow(clippy::too_many_lines)] // One fail-closed role/lineage check stays in audit order.
    fn record(
        &self,
        record: &BuildInput,
        data: &Value,
        target: &Path,
        artifacts: &[Value],
    ) -> Option<()> {
        use std::os::unix::fs::MetadataExt as _;
        (record.covered
            && super::package_matches(record, data)
            && super::output_matches(record, target))
        .then_some(())?;
        let package = data["packages"]
            .as_array()?
            .iter()
            .find(|p| p["id"] == record.package_id)?;
        // Refuse dependencies of the script itself: their generated Rust and proc
        // macros could be outside the independent package-source inventory.
        package["dependencies"]
            .as_array()?
            .iter()
            .all(|d| d["kind"].is_null() || d["kind"] == "normal" || d["kind"] == "dev")
            .then_some(())?;
        // Initial bootstrap supports only explicit regular-file declarations;
        // directory-default contracts need separately bounded exact traversal.
        for path in &record.paths {
            path.is_file().then_some(())?;
            self.covered(path)?;
        }
        (self.record_digest(record)? == record.digest).then_some(())?;
        for (key, value) in &record.env {
            super::allowed_environment(key).then_some(())?;
            (effective_value(key, Some(&self.environment)) == *value).then_some(())?;
        }
        let calls: Vec<_> = artifacts
            .iter()
            .filter(|a| {
                a["reason"] == "build-script-executed" && a["package_id"] == record.package_id
            })
            .collect();
        (calls.len() == 1).then_some(())?;
        let out = Path::new(calls[0]["out_dir"].as_str()?)
            .parent()?
            .join("output");
        (record.output.as_ref()?.0 == out).then_some(())?;
        let binaries: Vec<_> = artifacts
            .iter()
            .filter(|a| {
                a["reason"] == "compiler-artifact"
                    && a["package_id"] == record.package_id
                    && a["target"]["kind"]
                        .as_array()
                        .is_some_and(|k| k.iter().any(|k| k == "custom-build"))
            })
            .collect();
        // Cargo fresh=false means rustc compiled this executable during this
        // guarded invocation. A reused executable (fresh=true) is refused.
        (binaries.len() == 1 && binaries[0]["fresh"] == false).then_some(())?;
        let binary = binaries[0];
        let kinds = binary["target"]["kind"].as_array()?;
        (kinds.len() == 1 && kinds[0] == "custom-build").then_some(())?;
        let scripts: Vec<_> = package["targets"]
            .as_array()?
            .iter()
            .filter(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|k| k == "custom-build"))
            })
            .collect();
        (scripts.len() == 1 && scripts[0]["src_path"] == binary["target"]["src_path"])
            .then_some(())?;
        self.covered(Path::new(binary["target"]["src_path"].as_str()?))?;
        let filenames = binary["filenames"].as_array()?;
        (filenames.len() == 1).then_some(())?;
        let filename = Path::new(filenames[0].as_str()?);
        let directory = filename.parent()?;
        filename
            .canonicalize()
            .ok()?
            .starts_with(target.canonicalize().ok()?)
            .then_some(())?;
        let hash = directory.file_name()?.to_str()?.rsplit_once('-')?.1;
        (hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then_some(())?;
        let stem = format!("build_script_build-{hash}");
        let linked = directory.join(&stem);
        let original = std::fs::metadata(filename).ok()?;
        let alias = std::fs::metadata(&linked).ok()?;
        (original.is_file()
            && alias.is_file()
            && original.dev() == alias.dev()
            && original.ino() == alias.ino())
        .then_some(())?;
        let dep_info = directory.join(format!("{stem}.d"));
        let bytes = super::bytes(&dep_info)?;
        let text = std::str::from_utf8(&bytes).ok()?;
        let header = text.lines().next()?.split_once(": ")?.0;
        let targets = crate::managed::source_inputs::dependencies(&format!("x: {header}"))?;
        (targets.len() == 1 && Path::new(&targets[0]) == dep_info).then_some(())?;
        // Cargo emits relative rustc prerequisites from the workspace root,
        // including member/build.rs for a workspace member. Use the same base
        // as source_inputs::update, never this process cwd or the package cwd.
        let workspace = Path::new(data["workspace_root"].as_str()?);
        (workspace.is_absolute()
            && workspace.as_os_str().len() <= 4096
            && workspace.canonicalize().ok()?.as_path() == workspace)
            .then_some(())?;
        let inputs = crate::managed::source_inputs::dependencies(text)?;
        let source = Path::new(binary["target"]["src_path"].as_str()?)
            .canonicalize()
            .ok()?;
        let mut source_seen = false;
        for input in inputs {
            if Path::new(&input).is_relative() {
                // Other Cargo source kinds use the package cwd, not the
                // workspace cwd. Admit only the pinned local-path branch.
                (package["source"].is_null() && source.starts_with(workspace)).then_some(())?;
            }
            // Keep only one prefixed path at a time; absolute paths are unchanged.
            let input = workspace.join(input);
            self.covered(&input)?;
            source_seen |= input == source;
        }
        source_seen.then_some(())?;
        // Compilation-time environment declarations must also have the exact
        // effective parent value; Cargo-injected keys are not silently certified.
        for line in text.lines().filter_map(|l| l.strip_prefix("# env-dep:")) {
            let (key, expected) = line
                .split_once('=')
                .map_or((line, None), |(k, v)| (k, Some(v)));
            let actual = effective_value(key, Some(&self.environment));
            (actual.as_deref() == expected && super::allowed_environment(key)).then_some(())?;
        }
        Some(())
    }
}

#[cfg(target_os = "linux")]
fn environment() -> Option<std::collections::BTreeMap<String, String>> {
    let mut result = std::collections::BTreeMap::new();
    let mut bytes = 0usize;
    for (key, value) in std::env::vars_os() {
        let key = key.into_string().ok()?;
        let value = value.into_string().ok()?;
        bytes = bytes
            .checked_add(key.capacity())?
            .checked_add(value.capacity())?;
        (bytes <= 1024 * 1024 && result.len() < 4096).then_some(())?;
        result.insert(key, value);
    }
    Some(result)
}

#[cfg(target_os = "linux")]
fn hashes(files: &[PathBuf], stop: &std::sync::atomic::AtomicBool) -> Option<Vec<String>> {
    use sha2::{Digest as _, Sha256};
    use std::io::Read as _;
    use std::time::{Duration, Instant};
    let started = Instant::now();
    // The observed runtime alone is 1,852,005,612 bytes. This is a streamed
    // aggregate read bound, not retained allocation or a throughput promise.
    let mut remaining = 4 * 1024 * 1024 * 1024_u64;
    let mut result = Vec::with_capacity(files.len());
    let mut buffer = vec![0_u8; 65536].into_boxed_slice();
    for file in files {
        let fd = rustix::fs::open(
            file,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .ok()?;
        let mut input = std::fs::File::from(fd);
        let metadata = input.metadata().ok()?;
        (metadata.is_file() && metadata.len() <= 256 * 1024 * 1024).then_some(())?;
        let mut hash = Sha256::new();
        let mut file_remaining = 256 * 1024 * 1024_u64;
        loop {
            (!stop.load(std::sync::atomic::Ordering::Acquire)
                && started.elapsed() < Duration::from_secs(30))
            .then_some(())?;
            let count = input.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            let count_bytes = u64::try_from(count).ok()?;
            remaining = remaining.checked_sub(count_bytes)?;
            file_remaining = file_remaining.checked_sub(count_bytes)?;
            hash.update(&buffer[..count]);
        }
        result.push(format!("{:x}", hash.finalize()));
    }
    Some(result)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "build_inputs_bootstrap_tests.rs"]
mod tests;

//! Private execution layout from original authenticated payloads.

use std::io::{Read as _, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};

use super::{BTreeMap, BTreeSet, Error, File, Path, PathBuf, Payload, Sha256, Tools};
use super::{inventory_with_toolchain, refuse};
use sha2::Digest as _;

/// A private original-bundle copy with one exact route to authenticated Rust.
/// This creates no process, proof, source closure or reusable backend result.
#[derive(Debug)]
pub struct Stage {
    root: PathBuf,
    original: PathBuf,
    rust: PathBuf,
}

fn expected() -> Result<BTreeMap<String, Payload>, Error> {
    let pin: serde_json::Value = serde_json::from_str(include_str!(
        "../../resources/engine/kani/bundle-linux.json"
    ))
    .map_err(refuse)?;
    serde_json::from_value(
        pin.get("files")
            .cloned()
            .ok_or_else(|| refuse("missing embedded original bundle inventory"))?,
    )
    .map_err(refuse)
}

impl Tools {
    /// Copy the original bundle into a new private execution directory.
    /// The only added link is `toolchain`, with the exact authenticated Rust
    /// prefix as its absolute target. The original bundle stays unchanged.
    /// Partial captures remain at the named destination on failure.
    ///
    /// # Errors
    /// `SAIL6303` if the destination exists, its parent is not private, either
    /// original prefix changed, or any copied byte/metadata/route differs.
    pub fn stage(&self, destination: &Path) -> Result<Stage, Error> {
        if !destination.is_absolute() {
            return Err(refuse("tool stage requires an absolute new directory"));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| refuse("missing stage parent"))?;
        let parent = parent.canonicalize().map_err(refuse)?;
        if !parent.is_dir()
            || std::fs::metadata(&parent)
                .map_err(refuse)?
                .permissions()
                .mode()
                & 0o077
                != 0
        {
            return Err(refuse("tool stage parent must be private"));
        }
        let name = destination
            .file_name()
            .ok_or_else(|| refuse("missing stage name"))?;
        let root = parent.join(name);
        self.recheck()?;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(refuse)?;
        for (name, record) in expected()? {
            let output = root.join(&name);
            std::fs::create_dir_all(output.parent().ok_or_else(|| refuse("bundle parent"))?)
                .map_err(refuse)?;
            copy(&self.bundle.join(&name), &output, &record)?;
        }
        // This is the original release's declared empty directory.
        std::fs::create_dir_all(root.join("scripts")).map_err(refuse)?;
        std::os::unix::fs::symlink(&self.rust, root.join("toolchain")).map_err(refuse)?;
        self.inspect_stage(&root)
    }

    /// Inspect an existing private stage against these original tool prefixes.
    /// This grants no execution, source-currentness or result-reuse authority.
    ///
    /// # Errors
    /// `SAIL6303` if privacy prerequisites, original bytes or the route differ.
    pub fn inspect_stage(&self, root: &Path) -> Result<Stage, Error> {
        let metadata = std::fs::symlink_metadata(root).map_err(refuse)?;
        let parent = root
            .parent()
            .ok_or_else(|| refuse("missing stage parent"))?;
        if !root.is_absolute()
            || !metadata.is_dir()
            || metadata.permissions().mode() & 0o077 != 0
            || std::fs::metadata(parent)
                .map_err(refuse)?
                .permissions()
                .mode()
                & 0o077
                != 0
        {
            return Err(refuse(
                "existing tool stage and parent must be private directories",
            ));
        }
        let stage = Stage {
            root: root.canonicalize().map_err(refuse)?,
            original: self.bundle.clone(),
            rust: self.rust.clone(),
        };
        stage.recheck()?;
        Ok(stage)
    }
}

impl Stage {
    /// Canonical private stage directory, retained until its owner removes it.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Original pinned driver in its required release layout.
    /// This path is not permission to trust an unchecked later execution.
    #[must_use]
    pub fn driver(&self) -> PathBuf {
        self.root.join("bin/kani-driver")
    }

    /// Reinspect both original prefixes, copied payloads and exact Rust route.
    /// Caller ownership and immutability remain required throughout later use.
    ///
    /// # Errors
    /// `SAIL6303` for any changed, missing, extra or unsupported payload/route.
    pub fn recheck(&self) -> Result<(), Error> {
        let parent = self
            .root
            .parent()
            .ok_or_else(|| refuse("missing stage parent"))?;
        for directory in [self.root.as_path(), parent] {
            let metadata = std::fs::symlink_metadata(directory).map_err(refuse)?;
            if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
                return Err(refuse("tool stage privacy or directory type changed"));
            }
        }
        Tools::authenticate(&self.original, &self.rust)?;
        let (actual, _) = inventory_with_toolchain(&self.root, &BTreeSet::new(), Some(&self.rust))?;
        if actual != expected()? {
            return Err(refuse("staged original bundle payload differs"));
        }
        // A missing route is not represented by the ordinary file inventory.
        if std::fs::read_link(self.root.join("toolchain")).map_err(refuse)? != self.rust {
            return Err(refuse("staged Rust toolchain route unavailable"));
        }
        Ok(())
    }
}

fn copy(source: &Path, destination: &Path, expected: &Payload) -> Result<(), Error> {
    let fd = rustix::fs::open(
        source,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(refuse)?;
    let source = File::from(fd);
    let metadata = source.metadata().map_err(refuse)?;
    if !metadata.is_file()
        || metadata.len() != expected.bytes
        || (metadata.permissions().mode() & 0o111 != 0) != expected.executable
    {
        return Err(refuse("original bundle file metadata differs during copy"));
    }
    let mode = if expected.executable { 0o755 } else { 0o644 };
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(destination)
        .map_err(refuse)?;
    let mut input = source.take(expected.bytes + 1);
    let mut hash = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let n = input.read(&mut buffer).map_err(refuse)?;
        if n == 0 {
            break;
        }
        length += n as u64;
        if length > expected.bytes {
            return Err(refuse("original bundle file grew during copy"));
        }
        hash.update(&buffer[..n]);
        output.write_all(&buffer[..n]).map_err(refuse)?;
    }
    if length != expected.bytes || format!("{:x}", hash.finalize()) != expected.sha256 {
        return Err(refuse("original bundle file bytes differ during copy"));
    }
    output
        .set_permissions(std::fs::Permissions::from_mode(mode))
        .map_err(refuse)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_staged_toolchain_route_is_admitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let root =
            std::env::temp_dir().join(format!("sail-kani-stage-route-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let stage = root.join("stage");
        let rust = root.join("rust");
        std::fs::create_dir(&stage)?;
        std::fs::create_dir(&rust)?;
        std::os::unix::fs::symlink(&rust, stage.join("toolchain"))?;
        assert!(super::super::inventory(&stage, &BTreeSet::new()).is_err());
        assert!(inventory_with_toolchain(&stage, &BTreeSet::new(), Some(&rust)).is_ok());
        std::fs::remove_file(stage.join("toolchain"))?;
        std::os::unix::fs::symlink(root.join("foreign"), stage.join("toolchain"))?;
        assert!(inventory_with_toolchain(&stage, &BTreeSet::new(), Some(&rust)).is_err());
        std::fs::remove_file(stage.join("toolchain"))?;
        std::fs::write(stage.join("toolchain"), b"regular file")?;
        assert!(inventory_with_toolchain(&stage, &BTreeSet::new(), Some(&rust)).is_err());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn copied_payload_requires_exact_original_bytes_and_type()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-kani-stage-copy-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let source = root.join("original");
        std::fs::write(&source, b"original")?;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))?;
        let expected = Payload {
            bytes: 8,
            executable: false,
            sha256: "0682c5f2076f099c34cfdd15a9e063849ed437a49677e6fcc5b4198c76575be5".into(),
        };
        copy(&source, &root.join("copy"), &expected)?;
        assert_eq!(std::fs::read(root.join("copy"))?, b"original");
        assert!(copy(&source, &root.join("copy"), &expected).is_err());
        std::fs::write(&source, b"modified")?;
        assert!(copy(&source, &root.join("changed"), &expected).is_err());
        std::os::unix::fs::symlink(&source, root.join("alias"))?;
        assert!(copy(&root.join("alias"), &root.join("linked"), &expected).is_err());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}

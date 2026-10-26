//! Source-bound standard-library provider for the checked record transaction.

use std::io::Write as _;
use std::path::Path;

use crate::{FileStamp, Meta};

/// Only this private provider is used by the actual disk-cache writer. Its
/// operations are narrowly reviewed file/serde forwards; the generic host
/// interface is not a capability to assert a truthful solver origin.
struct NativeWriteHost<'a> {
    temporary: &'a Path,
    destination: &'a Path,
    meta: &'a Meta,
}

impl sail_acceptance::cache_write::PersistenceHost<std::fs::File, FileStamp, std::io::Error>
    for NativeWriteHost<'_>
{
    sail_acceptance::sail_trait_impl_spec! {
        open spec fn failure_stage(&self) -> Option<u8> { None }
    }

    fn create(&mut self) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.temporary)
    }

    fn header(&mut self) -> std::io::Result<Vec<u8>> {
        serde_json::to_vec(self.meta).map_err(std::io::Error::other)
    }

    fn write_all(&mut self, file: &mut std::fs::File, bytes: &[u8]) -> std::io::Result<()> {
        file.write_all(bytes)
    }

    fn stamp(&mut self, file: &std::fs::File) -> std::io::Result<FileStamp> {
        file.metadata().map(|metadata| FileStamp::of(&metadata))
    }

    fn close(&mut self, file: std::fs::File) {
        drop(file);
    }

    fn rename(&mut self) -> std::io::Result<()> {
        std::fs::rename(self.temporary, self.destination)
    }

    fn remove(&mut self) {
        let _ = std::fs::remove_file(self.temporary);
    }

    fn invalid_header(&mut self) -> std::io::Error {
        std::io::Error::other("serialized cache header contains a newline")
    }
}

/// Preserves the record image and phase order of the original `put_scoped`
/// closure. Exclusive-create refusal leaves a preexisting name untouched.
/// No fsync is introduced: durability and protected-store origin remain
/// separate from this exact write/metadata/close/rename transaction.
pub(crate) fn write_record(
    temporary: &Path,
    destination: &Path,
    meta: &Meta,
    reply: &[u8],
) -> std::io::Result<FileStamp> {
    let mut host = NativeWriteHost {
        temporary,
        destination,
        meta,
    };
    sail_acceptance::cache_write::write_record::<_, std::fs::File, FileStamp, std::io::Error>(
        &mut host, reply,
    )
    .map(sail_acceptance::cache_write::WriteCommit::into_stamp)
}

#[cfg(test)]
mod tests {
    use super::write_record;
    use crate::{ENTRY_VERSION, FileStamp, Meta, hex, parse_entry};
    use sail_key::{QueryKey, sha256};
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("sail-persist-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        assert!(std::fs::create_dir_all(&path).is_ok());
        path
    }

    fn metadata(key: &QueryKey, reply: &[u8]) -> Meta {
        Meta {
            mode: "default".to_owned(),
            entry_version: ENTRY_VERSION,
            key: key.to_string(),
            reply_sha256: hex(&sha256(reply)),
            command: "check-sat".to_owned(),
            solver: "pinned-fixture".to_owned(),
            stored_unix: 0,
            provenance: "fixture".to_owned(),
            effort: None,
        }
    }

    fn run(temporary: &Path, destination: &Path) -> std::io::Result<FileStamp> {
        write_record(
            temporary,
            destination,
            &metadata(&QueryKey([31; 32]), b"unsat\n"),
            b"unsat\n",
        )
    }

    #[test]
    fn exclusive_create_refuses_and_keeps_a_preexisting_file() {
        let root = scratch("existing");
        let temporary = root.join("temporary");
        let destination = root.join("entry");
        assert!(std::fs::write(&temporary, b"owned by another writer").is_ok());
        let error = run(&temporary, &destination)
            .err()
            .unwrap_or_else(|| unreachable!("existing temporary must refuse"));
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(&temporary).unwrap_or_default(),
            b"owned by another writer"
        );
        assert!(!destination.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn exclusive_create_refuses_existing_and_dangling_symlinks() {
        let root = scratch("symlinks");
        let target = root.join("target");
        assert!(std::fs::write(&target, b"original target").is_ok());
        for (index, existing) in [true, false].into_iter().enumerate() {
            let temporary = root.join(format!("temporary-{index}"));
            let destination = root.join(format!("entry-{index}"));
            let link_target = if existing {
                target.clone()
            } else {
                root.join("absent")
            };
            assert!(std::os::unix::fs::symlink(&link_target, &temporary).is_ok());
            let error = run(&temporary, &destination)
                .err()
                .unwrap_or_else(|| unreachable!("existing symlink must refuse"));
            assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
            assert!(
                std::fs::symlink_metadata(&temporary)
                    .is_ok_and(|meta| meta.file_type().is_symlink())
            );
            assert!(!destination.exists());
        }
        assert_eq!(
            std::fs::read(&target).unwrap_or_default(),
            b"original target"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_failure_removes_only_the_created_temporary_file() {
        let root = scratch("rename");
        let temporary = root.join("temporary");
        let destination = root.join("entry");
        assert!(std::fs::create_dir(&destination).is_ok());
        assert!(std::fs::write(destination.join("sentinel"), b"preserved").is_ok());
        assert!(run(&temporary, &destination).is_err());
        assert!(!temporary.exists());
        assert_eq!(
            std::fs::read(destination.join("sentinel")).unwrap_or_default(),
            b"preserved"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn actual_transaction_preserves_record_format_and_observed_stamp() {
        let root = scratch("image");
        let temporary = root.join("temporary");
        let destination = root.join("entry");
        let key = QueryKey([31; 32]);
        let reply = b"unsat\n";
        let meta = metadata(&key, reply);
        let stamp = write_record(&temporary, &destination, &meta, reply)
            .unwrap_or_else(|error| unreachable!("write: {error}"));
        let bytes = std::fs::read(&destination).unwrap_or_default();
        let mut expected = serde_json::to_vec(&meta).unwrap_or_default();
        expected.push(b'\n');
        expected.extend_from_slice(reply);
        assert_eq!(bytes, expected);
        let (decoded, actual) = parse_entry(&bytes, &key.to_string())
            .unwrap_or_else(|error| unreachable!("entry: {error:?}"));
        assert_eq!(decoded, meta);
        assert_eq!(actual, reply);
        let observed = std::fs::metadata(&destination)
            .unwrap_or_else(|error| unreachable!("metadata: {error}"));
        assert_eq!(stamp, FileStamp::of(&observed));
        assert!(!temporary.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}

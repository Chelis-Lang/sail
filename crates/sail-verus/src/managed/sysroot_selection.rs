//! Bounded content identity for optional local sysroot handoff selection.

use std::collections::BTreeMap;
use std::io::{self, Write};

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

// Optional reuse declines larger selections; ordinary context construction is
// unchanged. Stream into the digest, retaining no serialized selection buffer.
const MAX_SELECTION_BYTES: usize = 256 * 1024;

struct Environment<'a>(&'a BTreeMap<Vec<u8>, Vec<u8>>);

impl Serialize for Environment<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Environment keys are arbitrary OS bytes, not JSON object keys. Keep
        // their original sorted byte pairs, including non-UTF8 values.
        serializer.collect_seq(self.0.iter())
    }
}

struct SelectionDigest {
    digest: Sha256,
    bytes: usize,
}

impl Write for SelectionDigest {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let size = self
            .bytes
            .checked_add(bytes.len())
            .filter(|size| *size <= MAX_SELECTION_BYTES)
            .ok_or_else(|| io::Error::other("optional sysroot selection exceeds bound"))?;
        self.digest.update(bytes);
        self.bytes = size;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn selection(
    args: &[String],
    engine: Option<&crate::engine::Request>,
    identity: &[(String, String)],
    environment: &BTreeMap<Vec<u8>, Vec<u8>>,
) -> Option<String> {
    let mut writer = SelectionDigest {
        digest: Sha256::new(),
        bytes: 0,
    };
    serde_json::to_writer(
        &mut writer,
        &(1, args, engine, identity, Environment(environment)),
    )
    .ok()?;
    Some(format!("{:x}", writer.digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_byte_environment_uses_exact_sorted_pairs() -> Result<(), serde_json::Error> {
        let environment = BTreeMap::from([
            (b"RUSTUP_HOME".to_vec(), b"/opt/rustup".to_vec()),
            (vec![0xff, b'A'], vec![0xfe, 0]),
        ]);
        // The original activation rejects every nonempty byte-keyed map.
        assert!(serde_json::to_vec(&environment).is_err());
        let args = vec!["--lib".into()];
        let identity = vec![("/opt/verus/z3".into(), "solver-content".into())];
        let pairs = environment.iter().collect::<Vec<_>>();
        let expected = serde_json::to_vec(&(
            1,
            &args,
            Option::<&crate::engine::Request>::None,
            &identity,
            pairs,
        ))?;
        assert_eq!(
            selection(&args, None, &identity, &environment),
            Some(super::super::files::digest(&expected))
        );
        let mut changed = environment.clone();
        changed.insert(b"RUSTUP_HOME".to_vec(), b"/other/rustup".to_vec());
        assert_ne!(
            selection(&args, None, &identity, &changed),
            selection(&args, None, &identity, &environment)
        );
        assert_ne!(
            selection(&["--bin".into()], None, &identity, &environment),
            selection(&args, None, &identity, &environment)
        );
        Ok(())
    }

    #[test]
    fn oversized_optional_selection_declines_without_changing_normal_context() {
        let environment =
            BTreeMap::from([(b"RUSTFLAGS".to_vec(), vec![b'x'; MAX_SELECTION_BYTES])]);
        assert!(selection(&[], None, &[], &environment).is_none());
        assert!(selection(&[], None, &[], &BTreeMap::new()).is_some());
    }
}

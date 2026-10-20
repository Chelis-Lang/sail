//! The bundled tier: the vstd replies a release archive ships (ADR 0062).
//!
//! A release carries `replies/vstd.pack`, a pack of signed shared entries
//! ([`sail_cache::pack`]), and `replies/release.pub`, the public half of a
//! one-off key the release factory signed them with and then discarded. Both
//! files are listed, with their SHA-256 and size, in the archive's
//! `release-files.json`. The archive's checksum is the trust boundary for
//! everything in it, so the daemon trusts that key, **for that pack only**:
//! - the key and the pack are used only if their bytes match the index;
//! - the key is never added to the trust directory, so it authorizes no
//!   other store, and the trust directory's keys authorize nothing here;
//! - every entry still passes every check of
//!   [`verify_entry`](sail_cache::shared::verify_entry) (signature, reply
//!   hash, solver name, version and binary, platform class, mode, reply
//!   shape), and `unknown` answers the serving policy.
//!
//! Anything else is a miss, and the pinned solver answers. The tier loads on
//! a helper thread when the daemon starts; lookups before it is ready miss.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use sail_cache::pack::{MAX_PACK_BYTES, Pack};
use sail_cache::shared::{PublicKey, Rejection, Solver, TrustedKeys, Verified, verify_entry};
use sail_key::QueryKey;

/// The release index, relative to the release root.
pub const INDEX: &str = "release-files.json";
/// The pack, relative to the release root.
pub const PACK: &str = "replies/vstd.pack";
/// The release's public key, relative to the release root.
pub const KEY: &str = "replies/release.pub";
/// The provenance prefix of local copies of bundled entries.
pub const PROVENANCE_PREFIX: &str = "bundled:";

/// The largest index or key file read.
const SMALL_LIMIT: u64 = 8 * 1024 * 1024;

/// Where the bundled tier comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledConfig {
    /// The release root: the directory holding `release-files.json`,
    /// `bin/` and `replies/`.
    pub root: PathBuf,
}

impl BundledConfig {
    /// The release root of the running executable (`<root>/bin/sail`),
    /// when it ships a pack and `SAIL_BUNDLED_REPLIES` is not `0`.
    #[must_use]
    pub fn discover() -> Option<Self> {
        if std::env::var_os("SAIL_BUNDLED_REPLIES").is_some_and(|v| v == "0") {
            return None;
        }
        let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
        let root = exe.parent()?.parent()?.to_path_buf();
        (root.join(PACK).is_file() && root.join(INDEX).is_file()).then_some(Self { root })
    }
}

/// A loaded, index-checked pack and the one key trusted for it.
#[derive(Debug)]
struct Loaded {
    pack: Pack,
    keys: TrustedKeys,
    key_id: String,
}

/// What one bundled lookup found.
#[derive(Debug)]
pub(crate) enum Lookup {
    /// A verified entry.
    Hit(Verified),
    /// No entry, or the tier is not (yet) available.
    Miss,
    /// An entry that failed a check.
    Rejected(Rejection),
}

/// The bundled tier.
#[derive(Debug)]
pub(crate) struct BundledTier {
    root: PathBuf,
    state: OnceLock<Result<Loaded, String>>,
}

impl BundledTier {
    pub(crate) fn new(config: &BundledConfig) -> Self {
        Self {
            root: config.root.clone(),
            state: OnceLock::new(),
        }
    }

    /// Loads the tier; later calls do nothing. Run on a helper thread.
    pub(crate) fn load(&self) -> Option<&str> {
        let state = self.state.get_or_init(|| load(&self.root));
        state.as_ref().err().map(String::as_str)
    }

    /// Looks `key` up for a session whose solver is `local`.
    pub(crate) fn lookup(&self, key: &QueryKey, local: &Solver) -> Lookup {
        let Some(Ok(loaded)) = self.state.get() else {
            return Lookup::Miss;
        };
        let Some(data) = loaded.pack.get(key) else {
            return Lookup::Miss;
        };
        match verify_entry(data, key, &loaded.keys, local) {
            Ok(verified) => Lookup::Hit(verified),
            Err(reason) => Lookup::Rejected(reason),
        }
    }

    /// A one-line description for `INFO` and the log.
    pub(crate) fn describe(&self) -> String {
        let pack = self.root.join(PACK);
        match self.state.get() {
            None => format!("{} (loading)", pack.display()),
            Some(Ok(l)) => format!(
                "{} ({} entries, key {})",
                pack.display(),
                l.pack.len(),
                l.key_id
            ),
            Some(Err(why)) => format!("{} (not used: {why})", pack.display()),
        }
    }
}

/// Reads at most `limit` bytes of `path`; `None` when it is longer.
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if data.len() as u64 > limit {
        return Err(format!("{} is too large", path.display()));
    }
    Ok(data)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Reads `name` under `root`, requiring the size and SHA-256 that `files`
/// (the index's `files` map) records for it.
fn indexed(
    root: &Path,
    files: &serde_json::Map<String, serde_json::Value>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let member = files
        .get(name)
        .ok_or_else(|| format!("{name} is not in {INDEX}"))?;
    let (Some(sha256), Some(bytes)) = (
        member.get("sha256").and_then(serde_json::Value::as_str),
        member.get("bytes").and_then(serde_json::Value::as_u64),
    ) else {
        return Err(format!("{INDEX} has no digest for {name}"));
    };
    if bytes > limit {
        return Err(format!("{name} is too large"));
    }
    let data = read_bounded(&root.join(name), limit)?;
    if data.len() as u64 != bytes || hex(&sail_key::sha256(&data)) != sha256 {
        return Err(format!("{name} does not match {INDEX}"));
    }
    Ok(data)
}

fn load(root: &Path) -> Result<Loaded, String> {
    let index = read_bounded(&root.join(INDEX), SMALL_LIMIT)?;
    let index: serde_json::Value =
        serde_json::from_slice(&index).map_err(|_| format!("{INDEX} is not JSON"))?;
    let files = index
        .get("files")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| format!("{INDEX} has no file list"))?;
    let key = indexed(root, files, KEY, SMALL_LIMIT)?;
    let key = std::str::from_utf8(&key)
        .ok()
        .and_then(PublicKey::parse)
        .ok_or_else(|| format!("{KEY} is not a sail public key"))?;
    let pack = indexed(root, files, PACK, MAX_PACK_BYTES)?;
    let pack = Pack::parse(pack).ok_or_else(|| format!("{PACK} is malformed"))?;
    let mut keys = TrustedKeys::default();
    keys.insert(key);
    Ok(Loaded {
        pack,
        keys,
        key_id: key.id(),
    })
}

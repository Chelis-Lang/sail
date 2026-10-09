//! The shared tier (charter rule 9, ADR 0017).
//!
//! A shared store is a directory of content-addressed, **signed** entry
//! files that teams and CI share (a local path, an NFS mount, a synced
//! folder). Nothing in it is trusted because of where it is: an entry is
//! used only if it carries a valid ed25519 signature from a key the reader
//! trusts, and the signature binds everything the reader relies on.
//!
//! **Layout.** `<store>/v1/<platform>/<solver binary sha256>/<k0k1>/<key>`,
//! so entries are scoped by solver binary and platform class (ADR 0010,
//! item 6). The path is only a lookup hint: every field it encodes is also
//! in the signed header and checked there.
//!
//! **Entry file.** One JSON header line, then the exact reply bytes, like
//! the local disk tier. The header holds the v2 key, the reply's SHA-256,
//! the command, the mode, the solver identity (name, version, binary
//! SHA-256, platform), the effort (ADR 0009), the format version, the
//! publication time, the signer's key id and the signature. The signature
//! covers a length-prefixed encoding of every one of those fields (see
//! [`Header::signed_bytes`]), never the JSON text, so no JSON
//! canonicalization is involved.
//!
//! **Keys.** [`SecretKey`] and [`PublicKey`] are stored as one line of
//! text: `sail-ed25519-secret-v1 <hex>` and `sail-ed25519-public-v1 <hex>`.
//! A key id is the first 16 hex digits of the SHA-256 of the public key.
//! Readers trust the public keys in `<cache root>/trust/*.pub`
//! ([`TrustedKeys::load_dir`]).

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use sail_key::{QueryKey, sha256};
use serde::{Deserialize, Serialize};

use crate::maint::FileKind;
use crate::{DiskCache, Effort, hex, shape_ok};

/// The `format` field of every shared entry header.
pub const FORMAT: &str = "sail-shared-entry";

/// Version of the shared entry format. It is signed, so an entry can never
/// be read under another version's rules.
pub const SHARED_VERSION: u32 = 2;

/// The only mode shared entries exist for today. Opt-in modes have keys of
/// their own (charter rule 8) and would need their own trust decision.
pub const MODE_DEFAULT: &str = "default";

/// Largest shared entry file read, in bytes. Larger files are rejected
/// without being read.
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// Domain separation for signatures.
const DOMAIN: &[u8] = b"sail-shared/entry/v1";

/// The store's top-level directory for this format.
const STORE_DIR: &str = "v1";

const SECRET_TAG: &str = "sail-ed25519-secret-v1";
const PUBLIC_TAG: &str = "sail-ed25519-public-v1";

/// Sequence number for temporary file names in a store.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        let pair = text.get(2 * i..2 * i + 2)?;
        if !pair.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        *b = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

/// The identity of the solver an entry was produced by: the fields of
/// `sail_key::SolverIdentity`, with the binary hash as hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Solver {
    /// Solver family, for example `z3`.
    pub name: String,
    /// The version the solver reports.
    pub version: String,
    /// SHA-256 of the solver binary, as lowercase hex.
    pub binary_sha256: String,
    /// Platform class, for example `macos-aarch64`.
    pub platform: String,
}

impl Solver {
    /// The summary the disk tier records in [`crate::Meta::solver`]:
    /// `name version sha256 platform`.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} {} {} {}",
            self.name, self.version, self.binary_sha256, self.platform
        )
    }

    /// Parses a [`Solver::summary`]. Entries stored before the daemon
    /// recorded identities have an empty summary, which gives `None`.
    #[must_use]
    pub fn parse_summary(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        let s = Self {
            name: words.next()?.to_owned(),
            version: words.next()?.to_owned(),
            binary_sha256: words.next()?.to_owned(),
            platform: words.next()?.to_owned(),
        };
        (words.next().is_none() && s.well_formed()).then_some(s)
    }

    /// Whether every field is non-empty and safe as a path component, and
    /// the binary hash is 64 lowercase hex digits.
    #[must_use]
    pub fn well_formed(&self) -> bool {
        let safe = |s: &str| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
        };
        safe(&self.name)
            && safe(&self.version)
            && safe(&self.platform)
            && self.binary_sha256.len() == 64
            && self
                .binary_sha256
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }
}

/// The header line of a shared entry file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// Always [`FORMAT`].
    pub format: String,
    /// The entry format version, [`SHARED_VERSION`].
    pub entry_version: u32,
    /// The v2 key (ADR 0008), as hex.
    pub key: String,
    /// SHA-256 of the reply bytes, as hex.
    pub reply_sha256: String,
    /// The command the reply answers, for example `check-sat`.
    pub command: String,
    /// The mode the key was derived under: [`MODE_DEFAULT`].
    pub mode: String,
    /// The solver that produced the reply.
    pub solver: Solver,
    /// Effort for `check-sat` replies (ADR 0009).
    #[serde(default)]
    pub effort: Option<Effort>,
    /// Unix time the entry was published.
    pub published_unix: u64,
    /// The signer's key id ([`PublicKey::id`]).
    pub signer: String,
    /// The ed25519 signature over [`Header::signed_bytes`], as hex.
    pub signature: String,
}

fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn opt_u64(out: &mut Vec<u8>, v: Option<u64>) {
    match v {
        None => out.push(0),
        Some(n) => {
            out.push(1);
            out.extend_from_slice(&n.to_le_bytes());
        }
    }
}

impl Header {
    /// The bytes the signature covers: a domain tag, then every header
    /// field except the signature, each length-prefixed (or tagged, for
    /// optional numbers), so distinct headers never encode alike.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(512);
        field(&mut out, DOMAIN);
        field(&mut out, self.format.as_bytes());
        out.extend_from_slice(&self.entry_version.to_le_bytes());
        field(&mut out, self.key.as_bytes());
        field(&mut out, self.reply_sha256.as_bytes());
        field(&mut out, self.command.as_bytes());
        field(&mut out, self.mode.as_bytes());
        field(&mut out, self.solver.name.as_bytes());
        field(&mut out, self.solver.version.as_bytes());
        field(&mut out, self.solver.binary_sha256.as_bytes());
        field(&mut out, self.solver.platform.as_bytes());
        match &self.effort {
            None => out.push(0),
            Some(e) => {
                out.push(1);
                opt_u64(&mut out, e.limit);
                opt_u64(&mut out, e.used);
                match &e.reason {
                    None => out.push(0),
                    Some(r) => {
                        out.push(1);
                        field(&mut out, r.as_bytes());
                    }
                }
            }
        }
        out.extend_from_slice(&self.published_unix.to_le_bytes());
        field(&mut out, self.signer.as_bytes());
        out
    }
}

/// An ed25519 signing key. Its bytes are wiped when it is dropped, and
/// `Debug` never prints them.
pub struct SecretKey(SigningKey);

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretKey({})", self.public().id())
    }
}

impl SecretKey {
    /// A new key from the operating system's random source.
    ///
    /// # Errors
    ///
    /// The random source cannot be read.
    pub fn generate() -> std::io::Result<Self> {
        let mut seed = [0u8; 32];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
        let key = Self::from_seed(seed);
        seed.fill(0);
        Ok(key)
    }

    /// The key with this 32-byte seed (for tests and fixtures).
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&seed))
    }

    /// Its public key.
    #[must_use]
    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    /// The key file text: `sail-ed25519-secret-v1 <hex>`.
    #[must_use]
    pub fn to_text(&self) -> String {
        format!("{SECRET_TAG} {}\n", hex(self.0.as_bytes()))
    }

    /// Parses [`SecretKey::to_text`].
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        (words.next()? == SECRET_TAG).then_some(())?;
        let seed = unhex::<32>(words.next()?)?;
        words.next().is_none().then(|| Self::from_seed(seed))
    }

    /// Reads a key file.
    ///
    /// # Errors
    ///
    /// The file cannot be read or is not a secret key.
    pub fn read_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text).ok_or_else(|| format!("{} is not a sail secret key", path.display()))
    }

    /// Writes this key to `path` (readable by its owner only) and its
    /// public key to `<path>.pub`. Refuses to overwrite either file.
    ///
    /// # Errors
    ///
    /// Either file exists, or cannot be written.
    pub fn write_files(&self, path: &Path) -> std::io::Result<PathBuf> {
        let mut public = path.as_os_str().to_owned();
        public.push(".pub");
        let public = PathBuf::from(public);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.mode(0o600);
        }
        if public.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{} exists", public.display()),
            ));
        }
        opts.open(path)?.write_all(self.to_text().as_bytes())?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&public)?
            .write_all(self.public().to_text().as_bytes())?;
        Ok(public)
    }

    fn sign(&self, message: &[u8]) -> Signature {
        self.0.sign(message)
    }
}

/// An ed25519 public key that verifies shared entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    /// The key id: the first 16 hex digits of the SHA-256 of the key.
    /// Entries name their signer by id; a reader verifies with the trusted
    /// key of that id, so a colliding id never verifies a signature.
    #[must_use]
    pub fn id(&self) -> String {
        hex(&sha256(self.0.as_bytes())).chars().take(16).collect()
    }

    /// The key file text: `sail-ed25519-public-v1 <hex>`.
    #[must_use]
    pub fn to_text(&self) -> String {
        format!("{PUBLIC_TAG} {}\n", hex(self.0.as_bytes()))
    }

    /// Parses [`PublicKey::to_text`] (anything after the key on its line,
    /// such as a comment, is ignored). Weak keys (small-order points) are
    /// refused.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        (words.next()? == PUBLIC_TAG).then_some(())?;
        let bytes = unhex::<32>(words.next()?)?;
        VerifyingKey::from_bytes(&bytes)
            .ok()
            .filter(|k| !k.is_weak())
            .map(Self)
    }

    /// Reads a public key file.
    ///
    /// # Errors
    ///
    /// The file cannot be read or is not a public key.
    pub fn read_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text).ok_or_else(|| format!("{} is not a sail public key", path.display()))
    }
}

/// The trust directory of a cache root: `<root>/trust`.
#[must_use]
pub fn trust_dir(root: &Path) -> PathBuf {
    root.join("trust")
}

/// Public keys whose signatures a reader accepts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedKeys {
    keys: BTreeMap<String, PublicKey>,
}

impl TrustedKeys {
    /// Every key in `dir/*.pub`, and a description of each `.pub` file that
    /// is not a valid key. A missing directory trusts nothing.
    #[must_use]
    pub fn load_dir(dir: &Path) -> (Self, Vec<String>) {
        let mut keys = Self::default();
        let mut problems = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return (keys, problems);
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "pub") {
                continue;
            }
            match PublicKey::read_file(&path) {
                Ok(k) => keys.insert(k),
                Err(msg) => problems.push(msg),
            }
        }
        (keys, problems)
    }

    /// Trusts `key`.
    pub fn insert(&mut self, key: PublicKey) {
        self.keys.insert(key.id(), key);
    }

    /// The trusted key with this id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&PublicKey> {
        self.keys.get(id)
    }

    /// The ids of every trusted key.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    /// How many keys are trusted.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether no key is trusted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Trusts `key` for the cache at `root`: writes `<root>/trust/<id>.pub`
/// atomically, and returns its path.
///
/// # Errors
///
/// I/O errors.
pub fn add_trusted(root: &Path, key: &PublicKey) -> std::io::Result<PathBuf> {
    let dir = trust_dir(root);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.pub", key.id()));
    let tmp = dir.join(format!(".tmp-{}-{}", std::process::id(), key.id()));
    std::fs::write(&tmp, key.to_text())?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(path)
}

/// Why a shared entry was not used. Every rejection is a miss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rejection {
    /// The file could not be opened or read.
    Unreadable,
    /// Not a regular file.
    NotAFile,
    /// Larger than [`MAX_ENTRY_BYTES`].
    TooLarge,
    /// No header line, or not a valid header.
    BadHeader,
    /// Another format or format version.
    WrongFormat,
    /// The header names another key.
    KeyMismatch,
    /// The signer is not a trusted key.
    UntrustedSigner,
    /// The signature does not verify against the trusted key.
    BadSignature,
    /// The reply does not hash to the signed hash.
    HashMismatch,
    /// Signed for another solver (name, version or binary).
    OtherSolver,
    /// Signed for this solver on another platform class. Cross-class hits
    /// are only for a named opt-in mode (ADR 0010), which does not exist.
    OtherPlatform,
    /// Signed for a mode other than default.
    OtherMode,
    /// The reply does not have the shape its command requires.
    BadShape,
    /// An `unknown` answer the serving policy would not serve (ADR 0009).
    UnservableUnknown,
}

impl Rejection {
    /// A short name for logs and reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::NotAFile => "not-a-file",
            Self::TooLarge => "too-large",
            Self::BadHeader => "bad-header",
            Self::WrongFormat => "wrong-format",
            Self::KeyMismatch => "key-mismatch",
            Self::UntrustedSigner => "untrusted-signer",
            Self::BadSignature => "bad-signature",
            Self::HashMismatch => "hash-mismatch",
            Self::OtherSolver => "other-solver",
            Self::OtherPlatform => "other-platform",
            Self::OtherMode => "other-mode",
            Self::BadShape => "bad-shape",
            Self::UnservableUnknown => "unservable-unknown",
        }
    }
}

/// A shared entry that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The command the reply answers.
    pub command: String,
    /// The exact reply bytes.
    pub reply: Vec<u8>,
    /// The signed effort.
    pub effort: Option<Effort>,
    /// The signer's key id.
    pub signer: String,
}

impl Verified {
    /// The provenance a local copy records (charter rule 7):
    /// `shared:<key id>`.
    #[must_use]
    pub fn provenance(&self) -> String {
        format!("{PROVENANCE_PREFIX}{}", self.signer)
    }
}

/// Prefix of the provenance of every local entry copied from the shared
/// tier. Such entries are never published again.
pub const PROVENANCE_PREFIX: &str = "shared:";

/// The provenance of replies the local daemon recorded from the pinned
/// solver: the only ones `publish` exports.
pub const PINNED_SOLVER: &str = "pinned-solver";

/// Builds a signed shared entry file for `reply`, the reply of `command`
/// under `key`, produced by `solver`.
#[must_use]
pub fn sign_entry(
    secret: &SecretKey,
    key: &QueryKey,
    reply: &[u8],
    command: &str,
    solver: &Solver,
    effort: Option<&Effort>,
    published_unix: u64,
) -> Vec<u8> {
    let mut header = Header {
        format: FORMAT.to_owned(),
        entry_version: SHARED_VERSION,
        key: key.to_string(),
        reply_sha256: hex(&sha256(reply)),
        command: command.to_owned(),
        mode: MODE_DEFAULT.to_owned(),
        solver: solver.clone(),
        effort: effort.cloned(),
        published_unix,
        signer: secret.public().id(),
        signature: String::new(),
    };
    header.signature = hex(&secret.sign(&header.signed_bytes()).to_bytes());
    let mut out = serde_json::to_vec(&header).unwrap_or_default();
    out.push(b'\n');
    out.extend_from_slice(reply);
    out
}

/// Checks a shared entry file's contents for `key`, in this order:
/// the header parses and has this format and version; it names `key`; its
/// signer is trusted and the signature verifies over every header field;
/// the reply hashes to the signed hash; the signed solver is `local`'s
/// (name, version and binary), on `local`'s platform class; the mode is
/// default; and the reply has the shape its command requires.
///
/// # Errors
///
/// The first check that fails. The caller treats every one as a miss.
pub fn verify_entry(
    data: &[u8],
    key: &QueryKey,
    trusted: &TrustedKeys,
    local: &Solver,
) -> Result<Verified, Rejection> {
    let nl = data
        .iter()
        .position(|&b| b == b'\n')
        .ok_or(Rejection::BadHeader)?;
    let header: Header = data
        .get(..nl)
        .and_then(|h| serde_json::from_slice(h).ok())
        .ok_or(Rejection::BadHeader)?;
    if header.format != FORMAT || header.entry_version != SHARED_VERSION {
        return Err(Rejection::WrongFormat);
    }
    if header.key != key.to_string() {
        return Err(Rejection::KeyMismatch);
    }
    let signer = trusted
        .get(&header.signer)
        .ok_or(Rejection::UntrustedSigner)?;
    let signature = unhex::<64>(&header.signature)
        .map(|b| Signature::from_bytes(&b))
        .ok_or(Rejection::BadSignature)?;
    signer
        .0
        .verify_strict(&header.signed_bytes(), &signature)
        .map_err(|_| Rejection::BadSignature)?;
    let reply = data.get(nl + 1..).unwrap_or_default();
    if header.reply_sha256 != hex(&sha256(reply)) {
        return Err(Rejection::HashMismatch);
    }
    let s = &header.solver;
    if s.name != local.name || s.version != local.version || s.binary_sha256 != local.binary_sha256
    {
        return Err(Rejection::OtherSolver);
    }
    if s.platform != local.platform {
        return Err(Rejection::OtherPlatform);
    }
    if header.mode != MODE_DEFAULT {
        return Err(Rejection::OtherMode);
    }
    if !shape_ok(&header.command, reply) {
        return Err(Rejection::BadShape);
    }
    Ok(Verified {
        command: header.command,
        reply: reply.to_vec(),
        effort: header.effort,
        signer: header.signer,
    })
}

/// A shared store: a directory reachable as a path.
#[derive(Debug, Clone)]
pub struct SharedStore {
    root: PathBuf,
}

impl SharedStore {
    /// The store at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Its root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the entry for `key` produced by `solver` lives, or `None` if
    /// the solver's fields are not safe path components.
    #[must_use]
    pub fn entry_path(&self, solver: &Solver, key: &QueryKey) -> Option<PathBuf> {
        if !solver.well_formed() {
            return None;
        }
        let k = key.to_string();
        Some(
            self.root
                .join(STORE_DIR)
                .join(&solver.platform)
                .join(&solver.binary_sha256)
                .join(k.get(..2).unwrap_or("xx"))
                .join(k),
        )
    }

    /// Reads the file at `path`: `Ok(None)` if there is none. The file is
    /// opened before it is examined, so a store that hangs on open hangs
    /// here; callers bound this with a timeout.
    ///
    /// # Errors
    ///
    /// Not a regular file, too large, or unreadable.
    pub fn read(path: &Path) -> Result<Option<Vec<u8>>, Rejection> {
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Rejection::Unreadable),
        };
        let meta = file.metadata().map_err(|_| Rejection::Unreadable)?;
        if !meta.is_file() {
            return Err(Rejection::NotAFile);
        }
        if meta.len() > MAX_ENTRY_BYTES {
            return Err(Rejection::TooLarge);
        }
        let mut data = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
        // Read at most one byte past the limit, in case the file grew.
        (&mut file)
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|_| Rejection::Unreadable)?;
        if data.len() as u64 > MAX_ENTRY_BYTES {
            return Err(Rejection::TooLarge);
        }
        Ok(Some(data))
    }

    /// Writes `data` to `path` atomically: a temporary file in the same
    /// directory, then a rename.
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn write(path: &Path, data: &[u8]) -> std::io::Result<()> {
        let dir = path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "entry path has no parent")
        })?;
        std::fs::create_dir_all(dir)?;
        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".tmp-{}-{seq}", std::process::id()));
        let written = std::fs::write(&tmp, data).and_then(|()| std::fs::rename(&tmp, path));
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written
    }
}

/// What `publish` exports.
#[derive(Debug, Clone, Default)]
pub struct PublishOptions {
    /// Only entries for these commands (empty: every command).
    pub commands: Vec<String>,
    /// Only entries whose solver binary SHA-256 starts with this hex.
    pub solver_sha256: Option<String>,
    /// Count what would be published, and write nothing.
    pub dry_run: bool,
}

/// What `publish` did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishReport {
    /// Valid local entries examined.
    pub examined: u64,
    /// Entries signed and written (or, in a dry run, that would be).
    pub published: u64,
    /// Entries the store already holds, signed by this key.
    pub present: u64,
    /// Entries that came from the shared tier: never published again.
    pub from_shared: u64,
    /// Entries with any other provenance than the local pinned solver.
    pub other_provenance: u64,
    /// Entries without a recorded solver identity (stored before the
    /// daemon recorded one).
    pub unattributed: u64,
    /// `unknown` answers the serving policy would not serve (ADR 0009).
    pub unknown_held: u64,
    /// Entries left out by the filters.
    pub filtered: u64,
    /// Invalid local files (see `sail cache verify`).
    pub invalid: u64,
    /// Entries that could not be written.
    pub failed: u64,
}

/// Exports `disk`'s entries to `store`, signed with `secret`.
///
/// Only entries the local daemon recorded from the pinned solver
/// (provenance [`PINNED_SOLVER`]) with a recorded solver identity are
/// exported; entries that came from the shared tier never are. An
/// `unknown` answer is exported only if `unknown_ok` accepts its effort
/// (the serving policy, ADR 0009). An entry the store already holds,
/// signed by this key, is left alone.
pub fn publish(
    disk: &DiskCache,
    store: &SharedStore,
    secret: &SecretKey,
    options: &PublishOptions,
    unknown_ok: &dyn Fn(Option<&Effort>) -> bool,
) -> PublishReport {
    let mut r = PublishReport::default();
    let mut ours = TrustedKeys::default();
    ours.insert(secret.public());
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    for listed in disk.list() {
        let FileKind::Entry(key) = listed.kind else {
            continue;
        };
        let Ok((meta, reply)) = DiskCache::check_file(&listed.path, &key) else {
            r.invalid += 1;
            continue;
        };
        r.examined += 1;
        if meta.mode != MODE_DEFAULT {
            r.filtered += 1;
            continue;
        }
        if meta.provenance.starts_with(PROVENANCE_PREFIX) {
            r.from_shared += 1;
            continue;
        }
        if meta.provenance != PINNED_SOLVER {
            r.other_provenance += 1;
            continue;
        }
        let Some(solver) = Solver::parse_summary(&meta.solver) else {
            r.unattributed += 1;
            continue;
        };
        if (!options.commands.is_empty() && !options.commands.contains(&meta.command))
            || options
                .solver_sha256
                .as_ref()
                .is_some_and(|p| !solver.binary_sha256.starts_with(p.as_str()))
        {
            r.filtered += 1;
            continue;
        }
        if meta.command.starts_with("check-sat")
            && reply.trim_ascii() == b"unknown"
            && !unknown_ok(meta.effort.as_ref())
        {
            r.unknown_held += 1;
            continue;
        }
        let Some(path) = store.entry_path(&solver, &key) else {
            r.unattributed += 1;
            continue;
        };
        let present = SharedStore::read(&path)
            .ok()
            .flatten()
            .is_some_and(|data| verify_entry(&data, &key, &ours, &solver).is_ok());
        if present {
            r.present += 1;
            continue;
        }
        if options.dry_run {
            r.published += 1;
            continue;
        }
        let data = sign_entry(
            secret,
            &key,
            &reply,
            &meta.command,
            &solver,
            meta.effort.as_ref(),
            now,
        );
        match SharedStore::write(&path, &data) {
            Ok(()) => r.published += 1,
            Err(_) => r.failed += 1,
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solver() -> Solver {
        Solver {
            name: "z3".to_owned(),
            version: "4.16.0".to_owned(),
            binary_sha256: "ab".repeat(32),
            platform: "macos-aarch64".to_owned(),
        }
    }

    fn trusting(k: &SecretKey) -> TrustedKeys {
        let mut t = TrustedKeys::default();
        t.insert(k.public());
        t
    }

    const KEY: QueryKey = QueryKey([7; 32]);

    /// Key byte, reply, command, solver summary, provenance and effort.
    type Put<'a> = (u8, &'a [u8], &'a str, &'a str, &'a str, Option<&'a Effort>);

    #[test]
    fn a_signed_entry_verifies_and_every_field_is_bound() {
        let sk = SecretKey::from_seed([1; 32]);
        let trusted = trusting(&sk);
        let effort = Effort {
            limit: Some(100),
            used: Some(3),
            reason: Some("(:reason-unknown \"incomplete quantifiers\")".to_owned()),
        };
        let data = sign_entry(
            &sk,
            &KEY,
            b"unknown\n",
            "check-sat",
            &solver(),
            Some(&effort),
            9,
        );
        let v = verify_entry(&data, &KEY, &trusted, &solver());
        assert_eq!(
            v.as_ref().map(|v| (v.reply.as_slice(), v.effort.as_ref())),
            Ok((&b"unknown\n"[..], Some(&effort)))
        );
        assert_eq!(
            v.map(|v| v.provenance()),
            Ok(format!("shared:{}", sk.public().id()))
        );

        // Change each header field after signing: the signature fails.
        let nl = data.iter().position(|&b| b == b'\n').unwrap_or(0);
        let header: serde_json::Value =
            serde_json::from_slice(data.get(..nl).unwrap_or_default()).unwrap_or_default();
        let reply = data.get(nl + 1..).unwrap_or_default();
        let edits: &[(&str, serde_json::Value)] = &[
            ("command", "get-model".into()),
            ("mode", "opt-in".into()),
            ("published_unix", 10.into()),
            (
                "effort",
                serde_json::json!({"limit": 100, "used": 99, "reason": null}),
            ),
            ("effort", serde_json::Value::Null),
        ];
        for (name, value) in edits {
            let mut h = header.clone();
            h[*name] = value.clone();
            let mut forged = serde_json::to_vec(&h).unwrap_or_default();
            forged.push(b'\n');
            forged.extend_from_slice(reply);
            assert_eq!(
                verify_entry(&forged, &KEY, &trusted, &solver()),
                Err(Rejection::BadSignature),
                "{name}"
            );
        }
        for (field, value) in [
            ("name", "cvc5"),
            ("version", "4.16.1"),
            ("platform", "linux-x86_64"),
        ] {
            let mut h = header.clone();
            h["solver"][field] = value.into();
            let mut forged = serde_json::to_vec(&h).unwrap_or_default();
            forged.push(b'\n');
            forged.extend_from_slice(reply);
            assert_eq!(
                verify_entry(&forged, &KEY, &trusted, &solver()),
                Err(Rejection::BadSignature),
                "{field}"
            );
        }
    }

    #[test]
    fn forged_untrusted_corrupt_and_foreign_entries_are_rejected() {
        let sk = SecretKey::from_seed([1; 32]);
        let other = SecretKey::from_seed([2; 32]);
        let trusted = trusting(&sk);
        let good = sign_entry(&sk, &KEY, b"unsat\n", "check-sat", &solver(), None, 1);
        // Even a trusted signature must not admit the old publication rules.
        let end = good.iter().position(|&b| b == b'\n').unwrap_or(0);
        let mut old_header: Header = serde_json::from_slice(&good[..end])
            .unwrap_or_else(|e| unreachable!("valid fixture: {e}"));
        old_header.entry_version = 1;
        old_header.signature = hex(&sk.sign(&old_header.signed_bytes()).to_bytes());
        let mut old = serde_json::to_vec(&old_header).unwrap_or_default();
        old.extend_from_slice(&good[end..]);
        assert_eq!(
            verify_entry(&old, &KEY, &trusted, &solver()),
            Err(Rejection::WrongFormat)
        );

        // A forgery with a valid hash and no valid signature.
        let mut forged = sign_entry(&other, &KEY, b"sat\n", "check-sat", &solver(), None, 1);
        assert_eq!(
            verify_entry(&forged, &KEY, &trusted, &solver()),
            Err(Rejection::UntrustedSigner)
        );
        // ... claiming the trusted signer's id.
        let text =
            String::from_utf8_lossy(&forged).replace(&other.public().id(), &sk.public().id());
        forged = text.into_bytes();
        assert_eq!(
            verify_entry(&forged, &KEY, &trusted, &solver()),
            Err(Rejection::BadSignature)
        );
        // The reply swapped under a valid header.
        let swapped = String::from_utf8_lossy(&good).replace("\nunsat\n", "\nsat\n");
        assert_eq!(
            verify_entry(swapped.as_bytes(), &KEY, &trusted, &solver()),
            Err(Rejection::HashMismatch)
        );
        // Truncated, empty, garbage.
        let half = good.get(..good.len() / 2).unwrap_or_default();
        assert_eq!(
            verify_entry(half, &KEY, &trusted, &solver()),
            Err(Rejection::BadHeader)
        );
        assert_eq!(
            verify_entry(b"", &KEY, &trusted, &solver()),
            Err(Rejection::BadHeader)
        );
        assert!(verify_entry(b"\x00\xff garbage\nunsat\n", &KEY, &trusted, &solver()).is_err());
        // Another key.
        assert_eq!(
            verify_entry(&good, &QueryKey([8; 32]), &trusted, &solver()),
            Err(Rejection::KeyMismatch)
        );
        // Another solver binary, another platform class.
        let mut s = solver();
        s.binary_sha256 = "cd".repeat(32);
        assert_eq!(
            verify_entry(&good, &KEY, &trusted, &s),
            Err(Rejection::OtherSolver)
        );
        let mut p = solver();
        p.platform = "linux-x86_64".to_owned();
        assert_eq!(
            verify_entry(&good, &KEY, &trusted, &p),
            Err(Rejection::OtherPlatform)
        );
        // A signed reply of the wrong shape.
        let bad = sign_entry(
            &sk,
            &KEY,
            b"(error \"x\")\n",
            "check-sat",
            &solver(),
            None,
            1,
        );
        assert_eq!(
            verify_entry(&bad, &KEY, &trusted, &solver()),
            Err(Rejection::BadShape)
        );
        // Nothing trusted.
        assert_eq!(
            verify_entry(&good, &KEY, &TrustedKeys::default(), &solver()),
            Err(Rejection::UntrustedSigner)
        );
    }

    #[test]
    fn keys_round_trip_through_text_and_files() {
        let sk = SecretKey::from_seed([3; 32]);
        let parsed = SecretKey::parse(&sk.to_text()).map(|k| k.public());
        assert_eq!(parsed, Some(sk.public()));
        assert_eq!(PublicKey::parse(&sk.public().to_text()), Some(sk.public()));
        assert_eq!(
            PublicKey::parse(&format!("{} ci release", sk.public().to_text().trim())),
            Some(sk.public())
        );
        assert!(PublicKey::parse(&sk.to_text()).is_none());
        assert!(SecretKey::parse(&sk.public().to_text()).is_none());
        assert!(PublicKey::parse("sail-ed25519-public-v1 zz").is_none());
        // The identity point is weak and refused.
        let weak = format!("{PUBLIC_TAG} 01{}", "00".repeat(31));
        assert!(PublicKey::parse(&weak).is_none());
        assert!(!format!("{sk:?}").contains(&hex(&[3; 32])));
        assert_eq!(sk.public().id().len(), 16);

        let dir = std::env::temp_dir().join(format!("sail-shared-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(&dir).is_ok());
        let generated = SecretKey::generate();
        assert!(generated.is_ok());
        if let Ok(g) = generated {
            let file = dir.join("ci.key");
            let public = g.write_files(&file);
            assert!(public.is_ok());
            assert!(g.write_files(&file).is_err(), "never overwrites a key");
            assert_eq!(
                SecretKey::read_file(&file).map(|k| k.public()),
                Ok(g.public())
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let mode = std::fs::metadata(&file).map(|m| m.permissions().mode() & 0o777);
                assert_eq!(mode.ok(), Some(0o600));
            }
            let root = dir.join("cache");
            assert!(add_trusted(&root, &g.public()).is_ok());
            assert!(std::fs::write(trust_dir(&root).join("junk.pub"), "nope").is_ok());
            let (keys, problems) = TrustedKeys::load_dir(&trust_dir(&root));
            assert_eq!(keys.ids(), vec![g.public().id()]);
            assert_eq!(problems.len(), 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn solver_summaries_round_trip_and_unsafe_ones_are_refused() {
        assert_eq!(Solver::parse_summary(&solver().summary()), Some(solver()));
        assert_eq!(Solver::parse_summary(""), None);
        assert_eq!(Solver::parse_summary("z3 4.16.0 abc macos"), None);
        let mut s = solver();
        s.platform = "../etc".to_owned();
        assert!(!s.well_formed());
        assert!(SharedStore::new("/s").entry_path(&s, &KEY).is_none());
    }

    #[test]
    fn publish_exports_only_local_pinned_solver_entries() {
        let dir = std::env::temp_dir().join(format!("sail-shared-pub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let disk = DiskCache::new(dir.join("cache"));
        let store = SharedStore::new(dir.join("store"));
        let sk = SecretKey::from_seed([4; 32]);
        let id = solver().summary();
        let far = Effort {
            limit: Some(100),
            used: Some(1),
            reason: Some("(:reason-unknown \"incomplete quantifiers\")".to_owned()),
        };
        let near = Effort {
            used: Some(90),
            ..far.clone()
        };
        let puts: &[Put] = &[
            (1, b"unsat\n", "check-sat", &id, PINNED_SOLVER, None),
            (
                2,
                b"sat\n",
                "check-sat",
                &id,
                "shared:0123456789abcdef",
                None,
            ),
            (3, b"unsat\n", "check-sat", "", PINNED_SOLVER, None),
            (
                4,
                b"unknown\n",
                "check-sat",
                &id,
                PINNED_SOLVER,
                Some(&near),
            ),
            (5, b"unknown\n", "check-sat", &id, PINNED_SOLVER, Some(&far)),
            (6, b"x\n", "echo", &id, PINNED_SOLVER, None),
        ];
        for (k, reply, cmd, solver, prov, effort) in puts {
            let r = disk.put(&QueryKey([*k; 32]), reply, cmd, solver, prov, *effort);
            assert!(r.is_ok());
        }
        assert!(
            disk.put_scoped(
                &QueryKey([7; 32]),
                b"unsat\n",
                "check-sat",
                &crate::StoreContext {
                    solver: &id,
                    provenance: PINNED_SOLVER,
                    mode: "accept-first-proof"
                },
                None
            )
            .is_ok()
        );
        let policy = |e: Option<&Effort>| e.is_some_and(|e| e.used.unwrap_or(u64::MAX) * 2 <= 100);
        let opts = PublishOptions {
            commands: vec!["check-sat".to_owned()],
            ..PublishOptions::default()
        };
        let r = publish(&disk, &store, &sk, &opts, &policy);
        assert_eq!(
            (
                r.examined,
                r.published,
                r.from_shared,
                r.unattributed,
                r.unknown_held,
                r.filtered
            ),
            (7, 2, 1, 1, 1, 2)
        );
        // Published entries verify for a reader trusting the key.
        let trusted = trusting(&sk);
        for k in [1u8, 5] {
            let key = QueryKey([k; 32]);
            let data = store
                .entry_path(&solver(), &key)
                .and_then(|p| SharedStore::read(&p).ok().flatten());
            let v = data.map(|d| verify_entry(&d, &key, &trusted, &solver()));
            assert!(matches!(v, Some(Ok(_))), "key {k}");
        }
        // A second run finds them present.
        let again = publish(&disk, &store, &sk, &opts, &policy);
        assert_eq!((again.published, again.present), (0, 2));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

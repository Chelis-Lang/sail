//! `sail cache keygen`, `sail cache trust` and `sail cache publish`: key
//! management and publishing for the shared tier (ADR 0017); and the
//! release factory's `sail cache bundle-replies` (ADR 0062).
//!
//! Exit codes: 0 success; 2 usage error; 1 any other failure
//! (`SAIL0601`..`SAIL0605`).

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use sail_cache::DiskCache;
use sail_cache::shared::{
    PublicKey, PublishOptions, SecretKey, SharedStore, TrustedKeys, add_trusted, publish as export,
    trust_dir,
};

use crate::ops::CacheArgs;

const EXIT_USAGE: u8 = 2;

fn usage_error(err: &mut dyn std::io::Write, msg: &str) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0002]: {msg}");
    ExitCode::from(EXIT_USAGE)
}

fn failure(err: &mut dyn std::io::Write, code: &str, msg: &str) -> ExitCode {
    let _ = writeln!(err, "error[{code}]: {msg}");
    ExitCode::from(1)
}

/// `sail cache keygen --out FILE`.
pub fn keygen(
    a: &CacheArgs,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let Some(path) = &a.out else {
        return usage_error(err, "`sail cache keygen` needs --out FILE");
    };
    let key = match SecretKey::generate() {
        Ok(k) => k,
        Err(e) => return failure(err, "SAIL0601", &format!("cannot generate a key: {e}")),
    };
    match key.write_files(path) {
        Ok(public) => {
            let _ = writeln!(
                out,
                "key {}\n  secret {} (keep it private: whoever holds it can publish entries every trusting machine serves)\n  public {} (give this to readers: `sail cache trust add {}`)",
                key.public().id(),
                path.display(),
                public.display(),
                public.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => failure(
            err,
            "SAIL0602",
            &format!("cannot write {}: {e}", path.display()),
        ),
    }
}

/// `sail cache trust <add FILE | list | remove ID>`.
pub fn trust(
    root: &Path,
    a: &CacheArgs,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let dir = trust_dir(root);
    match a.rest.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["add", file] => {
            let key = match PublicKey::read_file(Path::new(file)) {
                Ok(k) => k,
                Err(e) => return failure(err, "SAIL0603", &e),
            };
            match add_trusted(root, &key) {
                Ok(path) => {
                    let _ = writeln!(
                        out,
                        "trusted key {} ({}); a running daemon picks it up within seconds",
                        key.id(),
                        path.display()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => failure(
                    err,
                    "SAIL0602",
                    &format!("cannot write to {}: {e}", dir.display()),
                ),
            }
        }
        ["list"] => {
            let (keys, problems) = TrustedKeys::load_dir(&dir);
            if a.json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::json!({
                        "trust_dir": dir.display().to_string(),
                        "keys": keys.ids(),
                        "problems": problems,
                    })
                );
                return ExitCode::SUCCESS;
            }
            let mut t = format!("trusted keys in {}: {}\n", dir.display(), keys.len());
            for id in keys.ids() {
                let _ = writeln!(t, "  {id}");
            }
            for p in problems {
                let _ = writeln!(t, "  ignored: {p}");
            }
            let _ = write!(out, "{t}");
            ExitCode::SUCCESS
        }
        ["remove", id] => {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
                return usage_error(err, &format!("`{id}` is not a key id"));
            }
            let path = dir.join(format!("{id}.pub"));
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    let _ = writeln!(out, "no longer trusting key {id}");
                    ExitCode::SUCCESS
                }
                Err(e) => failure(
                    err,
                    "SAIL0604",
                    &format!("cannot remove {}: {e}", path.display()),
                ),
            }
        }
        _ => usage_error(
            err,
            "usage: sail cache trust add FILE.pub | trust list | trust remove ID",
        ),
    }
}

/// `sail cache publish --to STORE --key FILE [--filter ...] [--dry-run]`.
pub fn publish(
    disk: &DiskCache,
    a: &CacheArgs,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let (Some(to), Some(key_file)) = (&a.to, &a.key) else {
        return usage_error(err, "`sail cache publish` needs --to STORE and --key FILE");
    };
    let mut options = PublishOptions {
        dry_run: a.dry_run,
        ..PublishOptions::default()
    };
    for f in &a.filters {
        match f.split_once('=') {
            Some(("command", v)) if !v.is_empty() => options.commands.push(v.to_owned()),
            Some(("solver", v)) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_hexdigit()) => {
                options.solver_sha256 = Some(v.to_ascii_lowercase());
            }
            _ => {
                return usage_error(
                    err,
                    &format!("bad filter `{f}` (use command=NAME or solver=SHA256-PREFIX)"),
                );
            }
        }
    }
    let secret = match SecretKey::read_file(key_file) {
        Ok(k) => k,
        Err(e) => return failure(err, "SAIL0603", &e),
    };
    let store = SharedStore::new(to);
    let policy = |e: Option<&sail_cache::Effort>| {
        let wire = e.map(|e| sail_api::protocol::Effort {
            limit: e.limit,
            used: e.used,
            reason: e.reason.clone(),
        });
        sail_api::policy::unknown_servable(wire.as_ref())
    };
    let r = export(disk, &store, &secret, &options, &policy);
    if a.json {
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "root": disk.root().display().to_string(),
                "store": to.display().to_string(),
                "signer": secret.public().id(),
                "dry_run": a.dry_run,
                "publish": r,
            }))
            .unwrap_or_default()
        );
    } else {
        let _ = writeln!(
            out,
            "{} {} entries from {} to {} (key {}); {} already there\n  not published: {} from the shared tier, {} other provenance, {} without a solver identity, {} unknown answers the serving policy would not serve, {} filtered, {} invalid",
            if a.dry_run {
                "would publish"
            } else {
                "published"
            },
            r.published,
            disk.root().display(),
            to.display(),
            secret.public().id(),
            r.present,
            r.from_shared,
            r.other_provenance,
            r.unattributed,
            r.unknown_held,
            r.filtered,
            r.invalid,
        );
    }
    if r.failed > 0 {
        return failure(
            err,
            "SAIL0605",
            &format!(
                "{} entries could not be written to {}",
                r.failed,
                to.display()
            ),
        );
    }
    ExitCode::SUCCESS
}

/// `sail cache bundle-replies --out DIR`: the release factory's step
/// (ADR 0062). Signs every exportable local entry with a fresh key into
/// `DIR/vstd.pack`, writes the key's public half to `DIR/release.pub`, and
/// keeps the secret only in memory, so it is gone when the command exits.
/// `DIR` must not exist.
pub fn bundle_replies(
    disk: &DiskCache,
    a: &CacheArgs,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let Some(dir) = &a.out else {
        return usage_error(err, "`sail cache bundle-replies` needs --out DIR");
    };
    let secret = match SecretKey::generate() {
        Ok(k) => k,
        Err(e) => return failure(err, "SAIL0601", &format!("cannot generate a key: {e}")),
    };
    let policy = |e: Option<&sail_cache::Effort>| {
        let wire = e.map(|e| sail_api::protocol::Effort {
            limit: e.limit,
            used: e.used,
            reason: e.reason.clone(),
        });
        sail_api::policy::unknown_servable(wire.as_ref())
    };
    let (pack, r) = sail_cache::pack::build(disk, &secret, &PublishOptions::default(), &policy);
    let public = secret.public();
    drop(secret);
    let written = std::fs::create_dir(dir).and_then(|()| {
        write_new(&dir.join("vstd.pack"), &pack)?;
        write_new(&dir.join("release.pub"), public.to_text().as_bytes())
    });
    if let Err(e) = written {
        return failure(
            err,
            "SAIL0605",
            &format!("cannot write {}: {e}", dir.display()),
        );
    }
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "root": disk.root().display().to_string(),
            "out": dir.display().to_string(),
            "signer": public.id(),
            "pack_bytes": pack.len(),
            "publish": r,
        }))
        .unwrap_or_default()
    );
    if r.published == 0 || r.failed > 0 {
        return failure(
            err,
            "SAIL0605",
            &format!(
                "{} entries packed, {} left out as too large",
                r.published, r.failed
            ),
        );
    }
    ExitCode::SUCCESS
}

/// Writes a new file (never replacing one), readable by everyone.
fn write_new(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(data)?;
    file.sync_all()
}

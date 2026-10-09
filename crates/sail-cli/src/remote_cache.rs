//! Optional team-cache commands. Ordinary verification needs none of this configuration.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sail_cache::DiskCache;
use sail_cache::maint::FileKind;
use sail_cache::shared::{PublicKey, PublishOptions, SecretKey, SharedStore, Solver, add_trusted};
use sail_daemon::http_server::{self, ServerConfig};
use sail_daemon::remote::{HttpClient, Published, RemoteConfig};

static STAGING: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent CLI switches")]
struct Args {
    root: Option<PathBuf>,
    token: Option<PathBuf>,
    ca: Option<PathBuf>,
    trust: Option<PathBuf>,
    store: Option<PathBuf>,
    listen: Option<String>,
    to: Option<String>,
    key: Option<PathBuf>,
    json: bool,
    dry: bool,
    proxy: bool,
    filters: Vec<String>,
    positional: Vec<String>,
}

fn parse(args: &[String], command: &str) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("SAIL0002: {arg} needs a value"))
        };
        match arg.as_str() {
            "--cache" => parsed.root = Some(value()?.into()),
            "--token-file" => parsed.token = Some(value()?.into()),
            "--ca-file" => parsed.ca = Some(value()?.into()),
            "--trust" => parsed.trust = Some(value()?.into()),
            "--store" => parsed.store = Some(value()?.into()),
            "--listen" => parsed.listen = Some(value()?),
            "--to" => parsed.to = Some(value()?),
            "--key" => parsed.key = Some(value()?.into()),
            "--filter" => parsed.filters.push(value()?),
            "--json" => parsed.json = true,
            "--dry-run" => parsed.dry = true,
            "--behind-tls-proxy" => parsed.proxy = true,
            text if text == command => {}
            text if !text.starts_with('-') => parsed.positional.push(text.to_owned()),
            _ => return Err(format!("SAIL0002: unknown remote-cache argument {arg}")),
        }
    }
    Ok(parsed)
}

/// Handles remote commands; `None` leaves existing directory-cache commands untouched.
pub fn cache(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Option<ExitCode> {
    let command = subcommand(args)?;
    if !matches!(command, "serve" | "connect" | "disconnect" | "publish") {
        return None;
    }
    if command == "publish"
        && !args.windows(2).any(|pair| {
            pair[0] == "--to" && (pair[1].starts_with("https://") || pair[1].starts_with("http://"))
        })
    {
        return None;
    }
    let result = parse(args, command).and_then(|parsed| run(command, &parsed, out));
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let (code, detail) = message.split_once(": ").unwrap_or(("SAIL0650", &message));
            let _ = writeln!(err, "error[{code}]: {detail}");
            ExitCode::from(1)
        }
    })
}

fn root(args: &Args) -> Result<PathBuf, String> {
    args.root.clone().map_or_else(
        || {
            sail_verus::cache_dir()
                .map(|p| p.join("results"))
                .map_err(|e| e.to_string())
        },
        Ok,
    )
}

fn run(command: &str, args: &Args, out: &mut dyn std::io::Write) -> Result<(), String> {
    match command {
        "serve" => serve(args, out),
        "connect" => {
            if args.positional.len() != 1 {
                return Err("SAIL0002: cache connect needs one endpoint".to_owned());
            }
            let token = args
                .token
                .clone()
                .ok_or_else(|| "SAIL0002: cache connect needs --token-file FILE".to_owned())?;
            let trust = args
                .trust
                .as_ref()
                .ok_or_else(|| "SAIL0002: cache connect needs --trust FILE.pub".to_owned())?;
            let public = PublicKey::read_file(trust)
                .map_err(|_| "SAIL0650: invalid trusted public key file".to_owned())?;
            let config = RemoteConfig {
                endpoint: args.positional[0].clone(),
                token_file: std::path::absolute(token)
                    .map_err(|_| "SAIL0650: cannot resolve token path".to_owned())?,
                ca_file: args
                    .ca
                    .as_ref()
                    .map(std::path::absolute)
                    .transpose()
                    .map_err(|_| "SAIL0650: cannot resolve CA path".to_owned())?,
            };
            let root = root(args)?;
            sail_daemon::remote::connect(&root, &config)?;
            add_trusted(&root, &public)
                .map_err(|_| "SAIL0650: cannot save signer trust".to_owned())?;
            let _ = writeln!(
                out,
                "connected shared cache {}; signer {}; running daemons reload within five seconds",
                config.endpoint,
                public.id()
            );
            Ok(())
        }
        "disconnect" => {
            sail_daemon::remote::disconnect(&root(args)?)?;
            let _ = writeln!(
                out,
                "disconnected shared cache; local entries and signing trust retained"
            );
            Ok(())
        }
        _ => publish(args, out),
    }
}

fn serve(args: &Args, out: &mut dyn std::io::Write) -> Result<(), String> {
    let store = args
        .store
        .as_ref()
        .ok_or_else(|| "SAIL0002: cache serve needs --store DIR".to_owned())?;
    let mut config = ServerConfig::local(store);
    if let Some(listen) = &args.listen {
        config.listen = listen
            .parse()
            .map_err(|_| "SAIL0002: --listen needs IP:PORT".to_owned())?;
    }
    config.behind_tls_proxy = args.proxy;
    let server = http_server::bind(&config)?;
    let address = server
        .local_addr()
        .map_err(|_| "SAIL0651: cannot read server address".to_owned())?;
    let _ = writeln!(
        out,
        "shared cache listening on http://{address}\n  read token file {}\n  write token file {}\n  credentials remain private; readers also need the publisher's public signing key",
        server.read_token_file.display(),
        server.write_token_file.display()
    );
    let _ = out.flush();
    let (stop, thread) = server
        .spawn()
        .map_err(|_| "SAIL0651: cannot start server thread".to_owned())?;
    let stopping = stop.clone();
    if ctrlc::set_handler(move || stopping.stop()).is_err() {
        stop.stop();
        let _ = thread.join();
        return Err("SAIL0651: cannot install shutdown handler".to_owned());
    }
    thread
        .join()
        .map_err(|_| "SAIL0651: server worker stopped unexpectedly".to_owned())?
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn policy(e: Option<&sail_cache::Effort>) -> bool {
    let effort = e.map(|e| sail_api::protocol::Effort {
        limit: e.limit,
        used: e.used,
        reason: e.reason.clone(),
    });
    sail_api::policy::unknown_servable(effort.as_ref())
}

fn publish(args: &Args, out: &mut dyn std::io::Write) -> Result<(), String> {
    let to = args
        .to
        .as_ref()
        .ok_or_else(|| "SAIL0002: publish needs --to URL".to_owned())?;
    let token = args
        .token
        .as_ref()
        .ok_or_else(|| "SAIL0002: remote publish needs --token-file FILE".to_owned())?;
    let key = args
        .key
        .as_ref()
        .ok_or_else(|| "SAIL0002: publish needs --key FILE".to_owned())?;
    let secret =
        SecretKey::read_file(key).map_err(|_| "SAIL0650: invalid signing key file".to_owned())?;
    let client = HttpClient::new(
        &RemoteConfig {
            endpoint: to.clone(),
            token_file: token.clone(),
            ca_file: args.ca.clone(),
        },
        Duration::from_secs(30),
    )?;
    let options = publication_options(args)?;
    let disk = DiskCache::new(root(args)?);
    // Reuse the single existing export policy instead of reproducing eligibility rules.
    let stage = Stage(std::env::temp_dir().join(format!(
        "sail-publish-{}-{}",
        std::process::id(),
        STAGING.fetch_add(1, Ordering::Relaxed)
    )));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(&stage.0)
        .map_err(|_| "SAIL0652: cannot create publication staging directory".to_owned())?;
    let store = SharedStore::new(&stage.0);
    let mut report = sail_cache::shared::publish(&disk, &store, &secret, &options, &policy);
    let mut conflicts = 0;
    if !args.dry {
        let (published, present, conflicting, failed) =
            upload_stage(&disk, &store, &client, &secret.public());
        report.published = published;
        report.present += present;
        report.failed += failed;
        conflicts = conflicting;
    }
    if args.json {
        let _ = writeln!(
            out,
            "{}",
            serde_json::json!({"endpoint": to, "signer": secret.public().id(), "dry_run":args.dry,"publish":report,"conflicts":conflicts})
        );
    } else {
        let _ = writeln!(
            out,
            "{} {} entries; {} identical, {} existing publications retained, {} failed; {} held unknown, {} shared-origin entries skipped",
            if args.dry {
                "would publish"
            } else {
                "published"
            },
            report.published,
            report.present,
            conflicts,
            report.failed,
            report.unknown_held,
            report.from_shared
        );
    }
    if report.failed > 0 {
        return Err(
            "SAIL0652: some publications failed; completed uploads remain available".to_owned(),
        );
    }
    Ok(())
}

fn publication_options(args: &Args) -> Result<PublishOptions, String> {
    let mut options = PublishOptions {
        dry_run: args.dry,
        ..PublishOptions::default()
    };
    for filter in &args.filters {
        match filter.split_once('=') {
            Some(("command", value)) if !value.is_empty() => {
                options.commands.push(value.to_owned());
            }
            Some(("solver", value))
                if !value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                options.solver_sha256 = Some(value.to_ascii_lowercase());
            }
            _ => return Err("SAIL0002: use --filter command=NAME or solver=HEX-PREFIX".to_owned()),
        }
    }
    Ok(options)
}

fn subcommand(args: &[String]) -> Option<&str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--cache" | "--socket" | "--to" | "--key" | "--out" | "--filter" | "--token-file"
            | "--ca-file" | "--trust" | "--store" | "--listen" => {
                iter.next()?;
            }
            text if text.starts_with('-') => {}
            text => return Some(text),
        }
    }
    None
}

fn upload_stage(
    disk: &DiskCache,
    store: &SharedStore,
    client: &HttpClient,
    public: &PublicKey,
) -> (u64, u64, u64, u64) {
    let counts: [AtomicU64; 4] = std::array::from_fn(|_| AtomicU64::new(0));
    let (send, receive) = std::sync::mpsc::sync_channel(8);
    let receiver = std::sync::Mutex::new(receive);
    std::thread::scope(|scope| {
        let mut workers = 0;
        for _ in 0..4 {
            let receive = &receiver;
            let counts = &counts;
            if std::thread::Builder::new()
                .name("sail-publish".to_owned())
                .spawn_scoped(scope, move || {
                    loop {
                        let task: Option<(_, _, PathBuf)> =
                            receive.lock().ok().and_then(|r| r.recv().ok());
                        let Some((key, solver, path)) = task else {
                            return;
                        };
                        let result = SharedStore::read(&path)
                            .ok()
                            .flatten()
                            .map(|data| client.put(&solver, &key, data, public));
                        let index = match result {
                            Some(Ok(Published::Created)) => 0,
                            Some(Ok(Published::Present)) => 1,
                            Some(Ok(Published::Conflict)) => 2,
                            _ => 3,
                        };
                        counts[index].fetch_add(1, Ordering::Relaxed);
                    }
                })
                .is_ok()
            {
                workers += 1;
            }
        }
        if workers == 0 {
            counts[3].fetch_add(1, Ordering::Relaxed);
            return;
        }
        for entry in disk.list() {
            let FileKind::Entry(key) = entry.kind else {
                continue;
            };
            let Ok((meta, _)) = DiskCache::check_file(&entry.path, &key) else {
                continue;
            };
            let Some(solver) = Solver::parse_summary(&meta.solver) else {
                continue;
            };
            let Some(path) = store.entry_path(&solver, &key).filter(|p| p.is_file()) else {
                continue;
            };
            if send.send((key, solver, path)).is_err() {
                counts[3].fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
        drop(send);
    });
    (
        counts[0].load(Ordering::Relaxed),
        counts[1].load(Ordering::Relaxed),
        counts[2].load(Ordering::Relaxed),
        counts[3].load(Ordering::Relaxed),
    )
}

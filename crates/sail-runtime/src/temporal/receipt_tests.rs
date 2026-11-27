//! Exact receipt identities, selective admission and disk faults with counted dispatches.
use super::*;
use sail_api::temporal::{Completion, Model, Operation, Outcome as Verdict, RequestedChecks};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};

type Edit = fn(&mut Job);
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[derive(Debug)]
struct Counting {
    calls: AtomicU32,
    verdict: Verdict,
    policy: &'static str,
}

impl TemporalBackend for Counting {
    fn replay_policy(&self) -> Option<ReplayPolicy> {
        Some(ReplayPolicy {
            identity: self.policy,
            validate: sail_api::temporal::validate_response,
        })
    }

    fn run(&self, _: &Worker, request: &Request, _: Duration, _: &AtomicBool) -> Dispatch {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut response = Response::new(
            self.verdict,
            serde_json::json!({
                "schema_version":sail_api::temporal::DIVE_REPORT_SCHEMA_VERSION, "command":"check", "verdict":"success",
                "options":{"profile":"dive","store":"exact"}, "statistics":{"left_on_queue":0}
            }),
        );
        response.request_sha256 = sail_api::temporal::request_sha256(request).unwrap_or_default();
        response.completion = Completion::Safety;
        response.requested_checks = Some(RequestedChecks {
            invariants: Vec::new(),
            properties: Vec::new(),
            postconditions: Vec::new(),
            possible: Vec::new(),
            deadlock: true,
        });
        Dispatch::Completed(Box::new(response))
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "sail-temporal-receipt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn job() -> Job {
    Job {
        obligation_id: "one".into(),
        requirement_ids: vec![],
        request: Request {
            version: sail_api::temporal::VERSION,
            model: Model {
                files: BTreeMap::from([
                    (
                        "M.tla".into(),
                        "---- MODULE M ----\nEXTENDS N\n====\n".into(),
                    ),
                    (
                        "N.tla".into(),
                        "---- MODULE N ----\nEXTENDS Naturals\n====\n".into(),
                    ),
                    ("M.cfg".into(), "INIT Init\nNEXT Next\n".into()),
                ]),
                root: "M.tla".into(),
                config: "M.cfg".into(),
            },
            operation: Operation::Check {
                max_states: 20,
                max_depth: 10,
            },
        },
        wall_time_ms: 10_000,
    }
}

#[test]
fn all_model_and_policy_inputs_change_the_exact_receipt() -> TestResult {
    let baseline = job();
    let worker = Worker {
        executable: PathBuf::new(),
        sha256: "a".repeat(64),
    };
    let receipt = |job: &Job| {
        let policy = ReplayPolicy {
            identity: "first-backend",
            validate: sail_api::temporal::validate_response,
        };
        receipts::Receipt::for_check(
            Some(Path::new("cache")),
            job,
            &sail_api::temporal::request_sha256(&job.request).ok()?,
            "dive",
            &worker,
            Some(policy),
        )
        .map(|r| r.key())
    };
    let original = receipt(&baseline).ok_or("missing key")?;
    let edits: &[Edit] = &[
        |j| {
            j.request
                .model
                .files
                .entry("M.tla".into())
                .or_default()
                .push(' ');
        },
        |j| {
            j.request
                .model
                .files
                .entry("N.tla".into())
                .or_default()
                .push(' ');
        },
        |j| {
            j.request
                .model
                .files
                .entry("M.cfg".into())
                .or_default()
                .push(' ');
        },
        |j| {
            j.request
                .model
                .files
                .insert("extra.tla".into(), String::new());
        },
        |j| j.request.model.root = "N.tla".into(),
        |j| j.request.model.config = "other.cfg".into(),
        |j| j.request.version += 1,
        |j| {
            j.request.operation = Operation::Check {
                max_states: 21,
                max_depth: 10,
            }
        },
        |j| {
            j.request.operation = Operation::Check {
                max_states: 20,
                max_depth: 11,
            }
        },
        |j| j.wall_time_ms += 1,
    ];
    for &edit in edits {
        let mut changed = baseline.clone();
        edit(&mut changed);
        assert_ne!(receipt(&changed).as_deref(), Some(original.as_str()));
    }
    let mut simulate = baseline;
    simulate.request.operation = Operation::Simulate {
        traces: 1,
        depth: 1,
    };
    assert!(receipt(&simulate).is_none());
    let context = receipts::context("worker", "backend", "platform", "adapter");
    for (worker, backend, platform, adapter) in [
        ("other", "backend", "platform", "adapter"),
        ("worker", "other", "platform", "adapter"),
        ("worker", "backend", "other", "adapter"),
        ("worker", "backend", "platform", "other"),
    ] {
        assert_ne!(
            context,
            receipts::context(worker, backend, platform, adapter)
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn actual_dispatch_count_tracks_eligibility_damage_legacy_and_cancel() -> TestResult {
    let temp = Scratch::new()?;
    let worker = Worker {
        executable: temp.0.join("worker"),
        sha256: crate::hash(b"worker"),
    };
    std::fs::write(&worker.executable, b"worker")?;
    let cache = temp.0.join("runtime");
    let mut engine = Counting {
        calls: AtomicU32::new(0),
        verdict: Verdict::Holds,
        policy: "first",
    };
    let token = Cancellation::default();
    let job = job();
    let run = |engine: &Counting, dir, token: &Cancellation| {
        execute(&job, Some(engine), Some(&worker), None, dir, token)
    };
    assert!(!run(&engine, Some(&cache), &token)?.cache_hit);
    let replay = run(&engine, Some(&cache), &token)?;
    let key = replay.cache_key.ok_or("expected exact hit")?;
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert!(!run(&engine, None, &token)?.cache_hit);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    let path = cache
        .join("temporal-v1/v2")
        .join(key.get(..2).ok_or("short key")?)
        .join(&key);
    std::fs::write(&path, b"truncated")?;
    assert!(!run(&engine, Some(&cache), &token)?.cache_hit);
    assert!(run(&engine, Some(&cache), &token)?.cache_hit);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 3);
    engine.policy = "another-adapter";
    engine.verdict = Verdict::Stopped;
    for _ in 0..2 {
        assert!(!run(&engine, Some(&cache), &token)?.cache_hit);
    }
    engine.verdict = Verdict::Refuted; // The stub has no counterexample: never record it.
    for _ in 0..2 {
        assert!(!run(&engine, Some(&cache), &token)?.cache_hit);
    }
    assert_eq!(engine.calls.load(Ordering::SeqCst), 7);
    engine.policy = "first";
    let cancel = Cancellation::default();
    cancel.cancel();
    let stopped = run(&engine, Some(&cache), &cancel)?;
    assert_eq!(stopped.outcome, Outcome::Cancelled);
    assert!(stopped.response.is_none() && !stopped.cache_hit);
    std::fs::write(&worker.executable, b"changed")?;
    let changed = run(&engine, Some(&cache), &token)?;
    assert_eq!(changed.outcome, Outcome::Unavailable);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 7);
    Ok(())
}

#[cfg(unix)]
#[test]
fn special_files_never_supply_evidence_or_redirect_receipt_writes() -> TestResult {
    use std::os::unix::fs::symlink;
    let root = Scratch::new()?;
    let worker = Worker {
        executable: root.0.join("worker"),
        sha256: crate::hash(b"worker"),
    };
    std::fs::write(&worker.executable, b"worker")?;
    let cache = root.0.join("runtime");
    let job = job();
    let engine = Counting {
        calls: AtomicU32::new(0),
        verdict: Verdict::Holds,
        policy: "test",
    };
    let run = || {
        execute(
            &job,
            Some(&engine),
            Some(&worker),
            None,
            Some(&cache),
            &Cancellation::default(),
        )
    };
    assert!(!run()?.cache_hit);
    let key = run()?.cache_key.ok_or("expected receipt")?;
    let bucket = cache
        .join("temporal-v1")
        .join("v2")
        .join(key.get(..2).ok_or("short key")?);
    let path = bucket.join(key);
    let decoy = root.0.join("untouched");
    std::fs::rename(&path, &decoy)?;
    let decoy_bytes = std::fs::read(&decoy)?;
    symlink(&decoy, &path)?;
    assert!(!run()?.cache_hit);
    assert_eq!(std::fs::read(&decoy)?, decoy_bytes);
    assert!(std::fs::symlink_metadata(&path)?.is_symlink());
    std::fs::remove_file(&path)?;
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()?
            .success()
    );
    assert!(!run()?.cache_hit);
    std::fs::remove_file(&path)?;
    std::fs::File::create(&path)?.set_len(4 * 1024 * 1024 * 1024)?;
    assert!(!run()?.cache_hit);
    assert!(run()?.cache_hit);
    std::fs::remove_file(&path)?;
    std::fs::remove_dir(&bucket)?;
    symlink(&root.0, &bucket)?;
    assert!(!run()?.cache_hit);
    assert!(!root.0.join(path.file_name().ok_or("no filename")?).exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn concurrent_cold_writers_only_publish_complete_reusable_evidence() -> TestResult {
    let root = Scratch::new()?;
    let worker = Worker {
        executable: root.0.join("worker"),
        sha256: crate::hash(b"worker"),
    };
    std::fs::write(&worker.executable, b"worker")?;
    let cache = root.0.join("runtime");
    let job = job();
    let engine = Counting {
        calls: AtomicU32::new(0),
        verdict: Verdict::Holds,
        policy: "test",
    };
    let run = || {
        execute(
            &job,
            Some(&engine),
            Some(&worker),
            None,
            Some(&cache),
            &Cancellation::default(),
        )
    };
    let gate = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    gate.wait();
                    run()
                })
            })
            .collect();
        for handle in handles {
            let completed = handle.join().map_err(|_| "thread panicked")??;
            assert_eq!(
                completed.response.ok_or("missing evidence")?.outcome,
                Verdict::Holds
            );
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })?;
    let calls = engine.calls.load(Ordering::SeqCst);
    assert!((1..=8).contains(&calls));
    assert!(run()?.cache_hit);
    assert_eq!(engine.calls.load(Ordering::SeqCst), calls);
    Ok(())
}

#[cfg(unix)]
#[test]
fn fresh_group_umask_cache_is_private_and_replays() -> TestResult {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    const CHILD: &str = "SAIL_TEMPORAL_UMASK_CONTROL";
    if std::env::var_os(CHILD).is_none() {
        // umask is process-wide. Set it only in a child that runs this single test.
        let status = std::process::Command::new("sh")
            .args(["-c", "umask 002; exec \"$@\"", "sh"])
            .arg(std::env::current_exe()?)
            .args([
                "--exact",
                "temporal::receipt_tests::fresh_group_umask_cache_is_private_and_replays",
            ])
            .env(CHILD, "1")
            .status()?;
        assert!(status.success());
        return Ok(());
    }
    let root = Scratch::new()?;
    let worker = Worker {
        executable: root.0.join("worker"),
        sha256: crate::hash(b"worker"),
    };
    std::fs::write(&worker.executable, b"worker")?;
    let cache = root.0.join("runtime");
    let engine = Counting {
        calls: AtomicU32::new(0),
        verdict: Verdict::Holds,
        policy: "test",
    };
    let job = job();
    let run = || {
        execute(
            &job,
            Some(&engine),
            Some(&worker),
            None,
            Some(&cache),
            &Cancellation::default(),
        )
    };
    assert!(!run()?.cache_hit);
    let key = run()?
        .cache_key
        .ok_or("repeat must replay under umask 002")?;
    let shard = cache
        .join("temporal-v1/v2")
        .join(key.get(..2).ok_or("short key")?);
    for dir in [
        &cache,
        &cache.join("temporal-v1"),
        &cache.join("temporal-v1/v2"),
        &shard,
    ] {
        assert_eq!(std::fs::metadata(dir)?.mode() & 0o777, 0o700);
    }
    assert_eq!(std::fs::metadata(shard.join(key))?.mode() & 0o777, 0o600);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    // Previously exposed stores require their owner's intervention.
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o770))?;
    assert!(!run()?.cache_hit);
    assert_eq!(std::fs::metadata(&cache)?.mode() & 0o777, 0o770);
    Ok(())
}

#[cfg(unix)]
#[test]
fn cancelling_during_evidence_validation_leaves_no_published_receipt() -> TestResult {
    thread_local! {
        static DURING_VALIDATION: std::cell::RefCell<Option<Cancellation>> = const { std::cell::RefCell::new(None) };
    }
    #[derive(Debug)]
    struct Cancels(Counting);
    impl TemporalBackend for Cancels {
        fn replay_policy(&self) -> Option<ReplayPolicy> {
            Some(ReplayPolicy {
                identity: "cancel-validation",
                validate: |req, response| {
                    if let Some(token) = DURING_VALIDATION.take() {
                        token.cancel();
                    }
                    sail_api::temporal::validate_response(req, response)
                },
            })
        }

        fn run(
            &self,
            worker: &Worker,
            request: &Request,
            deadline: Duration,
            cancel: &AtomicBool,
        ) -> Dispatch {
            self.0.run(worker, request, deadline, cancel)
        }
    }
    let root = Scratch::new()?;
    let worker = Worker {
        executable: root.0.join("worker"),
        sha256: crate::hash(b"worker"),
    };
    std::fs::write(&worker.executable, b"worker")?;
    let cache = root.0.join("runtime");
    let backend = Cancels(Counting {
        calls: AtomicU32::new(0),
        verdict: Verdict::Holds,
        policy: "unused",
    });
    let job = job();
    let run = |token: &Cancellation| {
        execute(
            &job,
            Some(&backend),
            Some(&worker),
            None,
            Some(&cache),
            token,
        )
    };
    let token = Cancellation::default();
    DURING_VALIDATION.set(Some(token.clone()));
    let cancelled = run(&token)?;
    assert_eq!(cancelled.outcome, Outcome::Cancelled);
    assert!(cancelled.response.is_none() && cancelled.cache_key.is_none());
    let fresh = run(&Cancellation::default())?;
    assert!(!fresh.cache_hit);
    assert!(run(&Cancellation::default())?.cache_hit);
    assert_eq!(backend.0.calls.load(Ordering::SeqCst), 2);
    Ok(())
}

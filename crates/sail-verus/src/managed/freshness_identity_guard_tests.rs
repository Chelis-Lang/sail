// Actual freshness-path controls for the unselected sidecar candidate.
use super::super::runtime_identity_guard;
use super::*;

fn slot(fixture: &Fixture, index: &FreshnessIndex) -> (Request, Arc<Mutex<RuntimeSlot>>) {
    let request = FreshnessIndex::request(
        &fixture.cache,
        &fixture.workspace,
        &fixture.context,
        &fixture.environment,
    )
    .unwrap_or_else(|e| unreachable!("fixture: {e}"));
    let runtime = index
        .entry(&request.key())
        .and_then(|entry| {
            entry
                .lock()
                .ok()
                .and_then(|projection| projection.runtime.clone())
        })
        .unwrap_or_else(|| unreachable!("complete owner required"));
    (request, runtime)
}
fn paths(fixture: &Fixture) -> [PathBuf; 2] {
    let identity = runner::runtime_identity(&fixture.cache, &fixture.context)
        .unwrap_or_else(|| unreachable!("fixture identity"));
    let directory = fixture.cache.join("runtime-identities");
    [
        directory.join(format!("{}.link", fixture.context)),
        directory.join(format!("{}.json", identity.0)),
    ]
}
fn guarded(runtime: &Mutex<RuntimeSlot>) -> bool {
    runtime
        .lock()
        .unwrap_or_else(|e| unreachable!("fixture: {e}"))
        .runtime
        .as_ref()
        .is_some_and(|validation| validation.identity_guarded)
}

#[test]
fn exact_sidecar_owner_avoids_only_hot_parse_and_preserves_complete_inspect() {
    let fixture = Fixture::new("sidecar-hot");
    let index = FreshnessIndex::default();
    assert!(fixture.establish(&index).complete);
    let (_, runtime) = slot(&fixture, &index);
    assert!(guarded(&runtime));
    runtime_identity_guard::with_enabled(false, || {
        assert!(
            fixture
                .inspect(&index)
                .is_some_and(|snapshot| snapshot.complete)
        );
        assert_eq!(runtime_identity_guard::parses(), 1);
    });
    runtime_identity_guard::with_enabled(true, || {
        for _ in 0..8 {
            assert!(
                fixture
                    .inspect(&index)
                    .is_some_and(|snapshot| snapshot.complete)
            );
        }
        assert_eq!(runtime_identity_guard::parses(), 0);
    });
}

#[test]
fn canonical_only_sidecar_aliases_retain_original_hot_parse_fallback() {
    for sidecar in 0..2 {
        let fixture = Fixture::new("sidecar-alias-fallback");
        let path = paths(&fixture)[sidecar].clone();
        let moved = path.with_extension("original");
        assert!(std::fs::rename(&path, &moved).is_ok());
        assert!(std::os::unix::fs::symlink(&moved, &path).is_ok());
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let (_, runtime) = slot(&fixture, &index);
        assert!(
            !guarded(&runtime),
            "canonical membership does not certify raw alias"
        );
        runtime_identity_guard::with_enabled(true, || {
            assert!(
                fixture
                    .inspect(&index)
                    .is_some_and(|snapshot| snapshot.complete)
            );
            assert_eq!(runtime_identity_guard::parses(), 1);
        });
    }
}

#[test]
fn absent_raw_binding_is_ineligible_without_any_inventory_changes() {
    let fixture = Fixture::new("sidecar-unbound");
    let lease = ReadLease::acquire(std::slice::from_ref(&fixture.runtime))
        .unwrap_or_else(|| unreachable!("kernel fixture"));
    let identity = runner::runtime_identity(&fixture.cache, &fixture.context)
        .unwrap_or_else(|| unreachable!("identity fixture"));
    assert!(!runtime_identity_guard::eligible(
        &fixture.cache,
        &fixture.context,
        &identity.0,
        &lease
    ));
    assert!(!lease.covers_exact_path(&paths(&fixture)[0]));
    assert!(!lease.covers_exact_path(&paths(&fixture)[1]));
}

#[test]
fn final_sidecar_validation_rejects_restored_bytes_aliases_and_identical_recreation() {
    for sidecar in 0..2 {
        for change in ["restore", "recreate", "alias", "hardlink", "mmap"] {
            let fixture = Fixture::new("sidecar-final");
            let target = paths(&fixture)[sidecar].clone();
            let bytes = std::fs::read(&target).unwrap_or_default();
            assert_ne!(bytes, [] as [u8; 0]);
            let modified = std::fs::metadata(&target)
                .and_then(|m| m.modified())
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
            let outside = fixture.root.join("outside-hardlink");
            if change == "hardlink" || change == "mmap" {
                assert!(std::fs::hard_link(&target, &outside).is_ok());
            }
            let index = FreshnessIndex::default();
            assert!(fixture.establish(&index).complete);
            let (request, runtime) = slot(&fixture, &index);
            assert!(guarded(&runtime));
            runtime_identity_guard::with_enabled(true, || {
                let result = request.refresh_outcome(&runtime, Instant::now(), || {
                    match change {
                        "recreate" => {
                            assert!(std::fs::remove_file(&target).is_ok());
                            assert!(std::fs::write(&target, &bytes).is_ok());
                        }
                        "alias" => {
                            let replacement = fixture.root.join("replacement");
                            assert!(std::fs::write(&replacement, &bytes).is_ok());
                            assert!(std::fs::remove_file(&target).is_ok());
                            assert!(std::os::unix::fs::symlink(&replacement, &target).is_ok());
                        }
                        "mmap" => {
                            let status = std::process::Command::new("python3")
                                .args(["-I", "-c", "import mmap,sys; f=open(sys.argv[1],'r+b'); m=mmap.mmap(f.fileno(),0); old=m[0:1]; m[0:1]=b'X'; m[0:1]=old; m.flush(); m.close(); f.close()"])
                                .arg(&outside).status();
                            assert!(status.is_ok_and(|status| status.success()));
                        }
                        _ => {
                            let path = if change == "hardlink" { &outside } else { &target };
                            assert!(std::fs::write(path, b"transient changed bytes").is_ok());
                            assert!(std::fs::write(path, &bytes).is_ok());
                        }
                    }
                    assert!(std::fs::File::open(&target)
                        .and_then(|file| file.set_times(std::fs::FileTimes::new().set_modified(modified)))
                        .is_ok());
                    assert_eq!(std::fs::read(&target).ok(), Some(bytes.clone()));
                }, || {});
                assert!(matches!(result, Refresh::Invalid), "{sidecar}/{change}");
                assert_eq!(runtime_identity_guard::parses(), 0);
                let runtime = runtime
                    .lock()
                    .unwrap_or_else(|e| unreachable!("fixture: {e}"));
                assert!(runtime.runtime.is_none());
                assert!(runtime.coherent.is_none());
            });
        }
    }
}

#[test]
fn namespace_rejects_sidecar_ancestor_redirect_and_same_inode_rebinding() {
    for redirect in [false, true] {
        let fixture = Fixture::new("sidecar-namespace");
        let sidecars = paths(&fixture);
        let original = fixture.cache.join("runtime-identities");
        let moved = fixture.cache.join("retired-identities");
        let index = FreshnessIndex::default();
        assert!(fixture.establish(&index).complete);
        let (_, runtime) = slot(&fixture, &index);
        assert!(guarded(&runtime));
        assert!(std::fs::rename(&original, &moved).is_ok());
        if redirect {
            assert!(std::os::unix::fs::symlink(&moved, &original).is_ok());
        } else {
            assert!(std::fs::create_dir(&original).is_ok());
            for path in sidecars {
                assert!(
                    std::fs::hard_link(moved.join(path.file_name().unwrap_or_default()), &path)
                        .is_ok()
                );
            }
        }
        runtime_identity_guard::with_enabled(true, || {
            assert!(fixture.inspect(&index).is_none());
            assert_eq!(runtime_identity_guard::parses(), 0);
        });
    }
}

#[test]
fn sidecar_owner_loss_and_evicted_live_owner_keep_original_lifetime() {
    let fixture = Fixture::new("sidecar-owner-loss");
    let index = FreshnessIndex::default();
    assert!(fixture.establish(&index).complete);
    let (request, runtime) = slot(&fixture, &index);
    assert!(guarded(&runtime));
    {
        let mut runtime = runtime
            .lock()
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(
            runtime
                .runtime
                .as_mut()
                .is_some_and(|validation| validation.lease.stop_owner_for_test().is_ok())
        );
    }
    runtime_identity_guard::with_enabled(true, || {
        assert!(matches!(
            request.refresh(&runtime, Instant::now()),
            Refresh::Invalid
        ));
        assert_eq!(runtime_identity_guard::parses(), 0);
    });
    let fixture = Fixture::new("sidecar-eviction");
    let index = FreshnessIndex::default();
    assert!(fixture.establish(&index).complete);
    let (request, runtime) = slot(&fixture, &index);
    assert!(
        index
            .entries
            .lock()
            .is_ok_and(|mut entries| entries.remove(&request.key()).is_some())
    );
    // Index eviction removes one reference, not the reader's live native owner.
    // This existing Arc remains actual authority while its full barriers pass;
    // the new boolean adds no parsed graph/Arc/reservation lifetime.
    runtime_identity_guard::with_enabled(true, || {
        assert!(
            matches!(request.refresh(&runtime, Instant::now()), Refresh::Ready(snapshot) if snapshot.complete)
        );
        assert_eq!(runtime_identity_guard::parses(), 0);
        let mut slot = runtime
            .lock()
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(
            slot.runtime
                .as_mut()
                .is_some_and(|validation| validation.lease.stop_owner_for_test().is_ok())
        );
        drop(slot);
        assert!(matches!(
            request.refresh(&runtime, Instant::now()),
            Refresh::Invalid
        ));
        assert!(
            index
                .entries
                .lock()
                .is_ok_and(|entries| !entries.contains_key(&request.key()))
        );
    });
}

#[test]
#[ignore = "explicit local complete-inspect component comparison; not a live reader or proof gate"]
fn complete_inspect_profile_retains_all_original_barriers_and_exact_output() {
    let fixture = Fixture::new("sidecar-profile");
    let identity = vec![
        (
            fixture.runtime.to_string_lossy().into_owned(),
            super::super::super::files::file_digest(&fixture.runtime).unwrap_or_default()
        );
        403
    ];
    let bytes = serde_json::to_vec(&identity).unwrap_or_else(|e| unreachable!("fixture: {e}"));
    let identity_sha = super::super::super::files::digest(&bytes);
    let directory = fixture.cache.join("runtime-identities");
    assert!(
        super::super::super::files::json(
            &directory.join(format!("{identity_sha}.json")),
            &identity
        )
        .is_ok()
    );
    assert!(
        super::super::super::files::json(
            &directory.join(format!("{}.link", fixture.context)),
            &identity_sha
        )
        .is_ok()
    );
    let index = FreshnessIndex::default();
    let before = fixture.establish(&index);
    assert!(before.complete);
    let (_, runtime) = slot(&fixture, &index);
    assert!(guarded(&runtime));
    let mut comparisons = Vec::new();
    for enabled in [false, true] {
        runtime_identity_guard::with_enabled(enabled, || {
            let mut times = Vec::new();
            for _ in 0..16 {
                let started = Instant::now();
                let snapshot = fixture
                    .inspect(&index)
                    .unwrap_or_else(|| unreachable!("complete inspection"));
                let wall_us = started.elapsed().as_micros();
                assert!(snapshot.complete);
                assert_eq!(snapshot.revision, before.revision);
                assert_eq!(snapshot.content_paths, before.content_paths);
                times.push(wall_us);
            }
            assert_eq!(
                runtime_identity_guard::parses(),
                if enabled { 0 } else { 16 }
            );
            comparisons.push(serde_json::json!({"guarded":enabled,"wall_us":times,"hot_parses":runtime_identity_guard::parses()}));
        });
    }
    assert_eq!(
        runner::runtime_identity(&fixture.cache, &fixture.context).map(|v| v.0),
        Some(identity_sha.clone())
    );
    eprintln!(
        "{}",
        serde_json::json!({"scope":"unselected complete freshness inspect comparison","generated_identity_pairs":403,"unique_runtime_files":1,"identity_json_bytes":bytes.len(),"identity_sha256":identity_sha,"comparisons":comparisons,"live_reader":false,"proof":false,"acceptance":false})
    );
}

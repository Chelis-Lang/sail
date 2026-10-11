// Frozen diagnostic-only addition. This compares complete output validation and
// real fixture input freshness, not RPC delivery, proof or reader acceptance.
fn original_locked_validate<R>(
    queue: &Queue,
    frames: &Frames,
    trace: &mut super::super::rpc_diagnostics::Trace<'_>,
    input: impl FnOnce(&mut super::super::rpc_diagnostics::Trace<'_>) -> R,
) -> Option<R> {
    // Original db4 locking and barrier algorithm, adapted only from Box owner
    // storage to the same Arc/Mutex representation used by this candidate.
    let _scratch = queue.budget.reserve(INSPECTION_BYTES)?;
    let cell = frames.output.try_lock().ok()?;
    let mut owner = cell.as_ref()?.try_lock().ok()?;
    if !trace.measure(super::super::rpc_diagnostics::Stage::OutputSeal, || {
        owner.guard.namespace_barrier()
    }) {
        return None;
    }
    let input = input(trace);
    trace
        .measure(super::super::rpc_diagnostics::Stage::OutputSeal, || {
            owner.guard.clean()
        })
        .then_some(input)
}

#[test]
#[ignore = "explicit original-lock-scope sensitivity; must fail its unchanged overlap assertion"]
fn original_lock_scope_sensitivity() {
    let mut fixture = Fixture::new();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let queue = Arc::clone(&fixture.queue);
    let frames = Arc::clone(fixture.frames());
    let (entered, overlapping, first) = std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            original_locked_validate(
                &queue,
                &frames,
                &mut super::super::rpc_diagnostics::Trace::disabled(),
                |_| {
                    let sent = entered_tx.send(()).is_ok();
                    let released = release_rx.recv_timeout(Duration::from_secs(2)).is_ok();
                    (sent, released, 17)
                },
            )
        });
        let entered = entered_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        let overlapping = original_locked_validate(
            &fixture.queue,
            fixture.frames(),
            &mut super::super::rpc_diagnostics::Trace::disabled(),
            |_| 23,
        );
        let _ = release_tx.send(());
        (entered, overlapping, worker.join().ok().flatten())
    });
    let cleanup = fixture.close();
    assert!(cleanup.0 && cleanup.1 && entered);
    assert_eq!(first, Some((true, true, 17)));
    assert_eq!(overlapping, Some(23));
}

struct InputFixture {
    root: PathBuf,
    cache: PathBuf,
    workspace: PathBuf,
    context: String,
    environment: std::collections::BTreeMap<String, String>,
}
impl InputFixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("sail-output-real-input-{}", std::process::id()));
        let workspace = root.join("project");
        let cache = root.join("cache");
        let context = "a".repeat(64);
        assert!(std::fs::create_dir_all(workspace.join("src")).is_ok());
        assert!(
            std::fs::write(
                workspace.join("Cargo.toml"),
                "[package]\nname='fixture'\nversion='0.0.0'\n"
            )
            .is_ok()
        );
        assert!(std::fs::write(workspace.join("src/lib.rs"), "original").is_ok());
        let directory = cache
            .join("projects")
            .join(
                sail_key::QueryKey(sail_key::sha256(workspace.as_os_str().as_encoded_bytes()))
                    .to_string(),
            )
            .join(&context);
        assert!(std::fs::create_dir_all(&directory).is_ok());
        let metadata = serde_json::json!({"workspace_root":workspace,"sail_invocation_directory":workspace,
            "sail_cache_directory":cache,"target_directory":workspace.join("target"),
            "packages":[{"manifest_path":workspace.join("Cargo.toml"),"source":null,"targets":[]}]});
        assert!(
            std::fs::write(
                directory.join("input-state.json"),
                serde_json::to_vec(&metadata).unwrap_or_default()
            )
            .is_ok()
        );
        assert!(
            std::fs::write(
                directory.join("source-inputs.json"),
                br#"{"paths":[],"covered":true,"by_package":{}}"#
            )
            .is_ok()
        );
        let runtime = root.join("runtime.bin");
        assert!(std::fs::write(&runtime, vec![0; 8 * 1024 * 1024]).is_ok());
        let identity = vec![(
            runtime.to_string_lossy().into_owned(),
            sail_key::QueryKey(sail_key::sha256(
                &std::fs::read(&runtime).unwrap_or_default(),
            ))
            .to_string(),
        )];
        let identity_bytes = serde_json::to_vec(&identity).unwrap_or_default();
        let identity_sha = sail_key::QueryKey(sail_key::sha256(&identity_bytes)).to_string();
        assert!(std::fs::create_dir_all(cache.join("runtime-identities")).is_ok());
        assert!(
            std::fs::write(
                cache
                    .join("runtime-identities")
                    .join(format!("{identity_sha}.json")),
                identity_bytes
            )
            .is_ok()
        );
        assert!(
            std::fs::write(
                cache
                    .join("runtime-identities")
                    .join(format!("{context}.link")),
                serde_json::to_vec(&identity_sha).unwrap_or_default()
            )
            .is_ok()
        );
        let environment = std::collections::BTreeMap::from([(
            "HOME".to_owned(),
            root.to_string_lossy().into_owned(),
        )]);
        Self {
            root,
            cache,
            workspace,
            context,
            environment,
        }
    }
    fn inspect(
        &self,
        index: &sail_verus::managed::FreshnessIndex,
    ) -> Option<sail_verus::managed::InputSnapshot> {
        index.inspect(
            &self.cache,
            &self.workspace,
            &self.context,
            &self.environment,
        )
    }
}
impl Drop for InputFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn full_seal(path: &Path, expected: &str) -> bool {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(sail_api::jobs::MAX_FRAME as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .is_ok()
        && bytes.len() <= sail_api::jobs::MAX_FRAME
        && sail_key::QueryKey(sail_key::sha256(&bytes)).to_string() == expected
}

#[test]
#[ignore = "explicit bounded full-output plus real input-freshness comparison; not acceptance"]
fn complete_output_and_real_freshness_profile() {
    use std::time::Instant;
    let input_path = std::env::var_os("SAIL_OUTPUT_PROFILE_INPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| unreachable!("profile input"));
    let result_path = std::env::var_os("SAIL_OUTPUT_PROFILE_RESULT")
        .map(PathBuf::from)
        .unwrap_or_else(|| unreachable!("profile result"));
    let bytes = std::fs::read(input_path).unwrap_or_default();
    let expected = sail_key::QueryKey(sail_key::sha256(&bytes)).to_string();
    assert_eq!(bytes.len(), 2158309);
    assert_eq!(
        expected,
        "718fc30e7df9978a21d130df8bbbb459cf9f433865462db4ca33b615ce531347"
    );
    assert!(serde_json::from_slice::<serde_json::Value>(&bytes).is_ok());
    let mut fixture = Fixture::new();
    fixture.close();
    assert!(std::fs::create_dir(&fixture.root).is_ok());
    assert!(std::fs::write(&fixture.path, &bytes).is_ok());
    fixture.bytes = bytes.clone();
    fixture.cache = ResultCache::default();
    fixture.queue = Queue::new(fixture.cache.budget());
    fixture.frames = Some(fixture.cache.insert(
        "family".to_owned(),
        Frames::new(
            Arc::new(bytes.clone()),
            Arc::new(bytes.clone()),
            Arc::new(bytes),
        ),
    ));
    fixture.worker =
        Some(Queue::spawn(&fixture.queue).unwrap_or_else(|e| unreachable!("profile: {e}")));
    let started = Instant::now();
    fixture
        .queue
        .offer(fixture.frames(), &fixture.path, &expected);
    assert!(fixture.queue.idle_for_test());
    assert!(
        fixture
            .frames()
            .output
            .lock()
            .is_ok_and(|cell| cell.is_some())
    );
    let output_construction_ms = started.elapsed().as_secs_f64() * 1000.0;
    let input = InputFixture::new();
    let index = sail_verus::managed::FreshnessIndex::default();
    let started = Instant::now();
    let established = index
        .establish(
            &input.cache,
            &input.workspace,
            &input.context,
            &input.environment,
        )
        .unwrap_or_else(|e| unreachable!("profile input: {e}"));
    assert!(established.complete);
    let revision = established.revision;
    let input_construction_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut conditions = Vec::new();
    for original in [true, false] {
        let gate = Arc::new(std::sync::Barrier::new(4));
        let rows = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for client in 0..4 {
                let gate = Arc::clone(&gate);
                let fixture = &fixture;
                let input = &input;
                let index = &index;
                let revision = &revision;
                let expected = &expected;
                handles.push(scope.spawn(move|| {
                    gate.wait();
                    let mut rows=Vec::new();
                    for sample in 0..16 {
                        let started=Instant::now();
                        let mut freshness_calls=0;
                        let mut inspect=|_: &mut super::super::rpc_diagnostics::Trace<'_>| {
                            freshness_calls+=1;
                            input.inspect(index).is_some_and(|s|s.complete && s.revision==*revision)
                        };
                        let mut trace=super::super::rpc_diagnostics::Trace::disabled();
                        let guarded=if original {
                            original_locked_validate(&fixture.queue,fixture.frames(),&mut trace,&mut inspect)
                        } else {
                            fixture.queue.validate(fixture.frames(),&mut trace,&mut inspect)
                        };
                        let hit=guarded.is_some();
                        let (output_valid,input_valid)=match guarded {
                            Some(valid)=>(true,valid),
                            None=>(full_seal(&fixture.path,expected),inspect(&mut trace)),
                        };
                        rows.push(serde_json::json!({"client":client,"sample":sample,
                            "wall_ms":started.elapsed().as_secs_f64()*1000.0,"guarded":hit,
                            "freshness_calls":freshness_calls,"output_valid":output_valid,"input_valid":input_valid}));
                    }
                    rows
                }));
            }
            let mut rows = Vec::new();
            for handle in handles {
                rows.extend(
                    handle
                        .join()
                        .unwrap_or_else(|_| unreachable!("profile thread")),
                );
            }
            rows
        });
        conditions.push(serde_json::json!({"name":if original {"original-lock-scope"}else{"candidate-unlocked-input"},"clients":4,"samples":rows}));
    }
    let source = input.workspace.join("src/lib.rs");
    let metadata = std::fs::metadata(&source).unwrap_or_else(|_| unreachable!("input metadata"));
    assert!(
        std::fs::write(&source, "changed").is_ok() && std::fs::write(&source, "original").is_ok()
    );
    assert!(
        std::fs::File::options()
            .write(true)
            .open(&source)
            .and_then(
                |file| file.set_times(std::fs::FileTimes::new().set_modified(metadata.modified()?))
            )
            .is_ok()
    );
    let input_mutation_refused = input
        .inspect(&index)
        .is_none_or(|s| !s.complete || s.revision != revision);
    drop(index);
    let cleanup = fixture.close();
    let all_complete = conditions.iter().all(|c| {
        c["samples"].as_array().is_some_and(|rows| {
            rows.iter()
                .all(|r| r["output_valid"] == true && r["input_valid"] == true)
        })
    });
    let value = serde_json::json!({"kind":"output-overlap-complete-operation-component",
        "fixture_bytes":2158309,"fixture_sha256":expected,"input_runtime_bytes":8*1024*1024,
        "input_source_files":1,"output_construction_ms":output_construction_ms,
        "input_construction_ms":input_construction_ms,"all_complete":all_complete,
        "input_mutation_refused":input_mutation_refused,"cleanup_joined":cleanup.0,
        "cleanup_removed":cleanup.1,"conditions":conditions});
    assert!(
        std::fs::write(
            result_path,
            serde_json::to_vec_pretty(&value).unwrap_or_default()
        )
        .is_ok()
    );
    assert!(cleanup.0 && cleanup.1 && input_mutation_refused && all_complete);
}

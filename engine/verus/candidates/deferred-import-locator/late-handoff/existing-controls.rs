use super::sail_owned_imports as capture;
use super::*;
use capture::{CaptureFailure, State, sail_capture_loaded_imports};
use rustc_data_structures::sync::FreezeLock;
use rustc_span::def_id::CrateNum;
use rustc_span::source_map::FilePathMapping;
use rustc_span::{
    Pos, RelativeBytePos, SourceFileHash, SourceFileHashAlgorithm, SourceFileLines,
    StableSourceFileId, Symbol,
};

static SERIAL: Mutex<()> = Mutex::new(());

fn session(f: impl FnOnce()) {
    let _guard = SERIAL.lock().unwrap();
    rustc_span::create_session_if_not_set_then(rustc_span::edition::Edition::Edition2021, |_| f());
}

fn imported(map: &SourceMap, name: &str, text: &str, cnum: usize) -> Arc<rustc_span::SourceFile> {
    let name = FileName::Custom(name.into());
    let id =
        rustc_span::def_id::StableCrateId::new(Symbol::intern("dep"), false, Vec::new(), "1.98.1");
    map.new_imported_source_file(
        name.clone(),
        SourceFileHash::new_in_memory(SourceFileHashAlgorithm::Sha256, text),
        None,
        StableSourceFileId::from_filename_for_export(&name, id),
        text.len() as u32,
        text.len() as u32,
        CrateNum::new(cnum),
        FreezeLock::new(SourceFileLines::Lines(vec![RelativeBytePos::from_u32(0)])),
        Vec::new(),
        Vec::new(),
        0,
    )
}

fn fixture(
    file: &rustc_span::SourceFile,
) -> (
    SpanContextX,
    HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
) {
    let original_start = 1000;
    let original_end = original_start + file.normalized_source_len.0;
    let external = ExternSourceFile {
        original_start_pos: BytePos(original_start),
        original_end_pos: BytePos(original_end),
        info: Arc::new(Mutex::new(ExternSourceInfo::Loaded {
            start_pos: file.start_pos,
            end_pos: BytePos(file.start_pos.0 + file.normalized_source_len.0),
        })),
    };
    let metadata = HashMap::from([(
        7,
        HashMap::from([(
            file.src_hash.hash_bytes().to_vec(),
            FileStartEndPos {
                filename: None,
                start_pos: original_start,
                end_pos: original_end,
            },
        )]),
    )]);
    (
        SpanContextX {
            local_crate: rustc_span::def_id::StableCrateId::new(
                Symbol::intern("local"),
                false,
                Vec::new(),
                "1.98.1",
            ),
            next_span_id: std::sync::atomic::AtomicU64::new(1),
            local_files: HashMap::new(),
            sail_imports: Mutex::new(None),
            imported_crates: HashMap::from([(
                7,
                CrateInfo {
                    files: vec![external],
                },
            )]),
        },
        metadata,
    )
}

fn crate_id(cnum: CrateNum) -> u64 {
    if cnum == CrateNum::new(2) { 7 } else { 8 }
}

fn packed(lo: u32, hi: u32) -> [u64; 2] {
    [7, (u64::from(lo) << 32) | u64::from(hi)]
}

#[test]
fn loaded_foreign_capture_matches_exact_stock_resolver_and_is_worker_owned() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        map.new_source_file(FileName::Custom("prefix".into()), "padding".into());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        let words = packed(1002, 1006);
        let stock = context
            .unpack_span(&words.to_vec(), Some(&map))
            .unwrap()
            .data();
        assert_eq!(
            captured.current_range(words).unwrap(),
            (stock.lo.0, stock.hi.0)
        );
        assert_eq!(stock.ctxt, rustc_span::SyntaxContext::root());
        assert_eq!(stock.parent, None);
        let State::Loaded(owner) = &captured.records()[0].state else {
            panic!("loaded owner required")
        };
        assert_eq!(owner.index, 1);
        assert_eq!(owner.hash.kind, SourceFileHashAlgorithm::Sha256);
        assert_eq!(owner.hash.hash_bytes(), file.src_hash.hash_bytes());
        assert_eq!(owner.stable_id, file.stable_id);
        assert_eq!(owner.name, format!("{:?}", file.name));
        assert_eq!(owner.normalized_length, 8);
        assert_eq!(owner.unnormalized_length, 8);
        let expected = (stock.lo.0, stock.hi.0);
        assert_eq!(
            std::thread::spawn(move || captured.current_range(words))
                .join()
                .unwrap()
                .unwrap(),
            expected
        );
    });
}

#[test]
fn delayed_missing_and_busy_entries_do_not_load_sources_or_promote_later() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let info = &context.imported_crates[&7].files[0].info;
        for (state, expected) in [
            (
                ExternSourceInfo::Delayed {
                    filename: "never-read-by-capture".into(),
                    hash: file.src_hash.hash_bytes().to_vec(),
                },
                CaptureFailure::Delayed,
            ),
            (ExternSourceInfo::None, CaptureFailure::Missing),
        ] {
            *info.lock().unwrap() = state;
            let count = map.files().len();
            let captured =
                sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                    .unwrap();
            assert_eq!(captured.current_range(packed(1002, 1006)), Err(expected));
            assert_eq!(map.files().len(), count);
            assert!(matches!(
                *info.lock().unwrap(),
                ExternSourceInfo::Delayed { .. } | ExternSourceInfo::None
            ));
            *info.lock().unwrap() = ExternSourceInfo::Loaded {
                start_pos: file.start_pos,
                end_pos: BytePos(file.start_pos.0 + 8),
            };
            assert_eq!(captured.current_range(packed(1002, 1006)), Err(expected));
        }
        let guard = info.lock().unwrap();
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        assert_eq!(
            captured.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Busy)
        );
        drop(guard);
    });
}

#[test]
fn namespace_hash_range_and_duplicate_content_refusals_are_exact() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (mut context, mut metadata) = fixture(&file);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, |_| 99),
            Err(CaptureFailure::CrateMismatch)
        ));
        let original = metadata[&7].keys().next().unwrap().clone();
        let range = metadata.get_mut(&7).unwrap().remove(&original).unwrap();
        metadata
            .get_mut(&7)
            .unwrap()
            .insert(vec![0; 32], range.clone());
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::HashMismatch)
        ));
        metadata.get_mut(&7).unwrap().insert(original, range);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::AmbiguousOriginalHash)
        ));
        metadata.get_mut(&7).unwrap().remove(&vec![0; 32]);
        let duplicate = context.imported_crates[&7].files[0].clone();
        context
            .imported_crates
            .get_mut(&7)
            .unwrap()
            .files
            .push(duplicate);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::AmbiguousOriginalRange)
        ));
        context.imported_crates.get_mut(&7).unwrap().files.pop();
        imported(&map, "other-crate.rs", "abcdefgh", 3);
        drop(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap(),
        );
        imported(&map, "same-crate-copy.rs", "abcdefgh", 2);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::AmbiguousContentOwner)
        ));
    });
}

#[test]
fn exact_bounds_missing_original_and_wrong_current_map_fail_closed() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        assert_eq!(
            captured.current_range(packed(1000, 1000)),
            Ok((file.start_pos.0, file.start_pos.0))
        );
        assert_eq!(
            captured.current_range(packed(1008, 1008)),
            Ok((file.start_pos.0 + 8, file.start_pos.0 + 8))
        );
        for words in [packed(1002, 1001), packed(1002, 1009), packed(999, 1000)] {
            assert!(captured.current_range(words).is_err());
        }
        assert_eq!(
            captured.current_range([9, packed(1002, 1006)[1]]),
            Err(CaptureFailure::UnknownCrate)
        );
        drop(captured);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &HashMap::new(), &map, crate_id),
            Err(CaptureFailure::MissingOriginalHash)
        ));
        let wrong = SourceMap::new(FilePathMapping::empty());
        imported(&wrong, "dep.rs", "different", 2);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &wrong, crate_id),
            Err(CaptureFailure::MissingCurrentFile)
        ));
        let equal_length_wrong = SourceMap::new(FilePathMapping::empty());
        imported(&equal_length_wrong, "dep.rs", "ijklmnop", 2);
        assert!(matches!(
            sail_capture_loaded_imports(
                &context.imported_crates,
                &metadata,
                &equal_length_wrong,
                crate_id
            ),
            Err(CaptureFailure::HashMismatch)
        ));
    });
}

#[test]
fn coherent_source_map_relocation_is_observed_without_asserting_equivalence() {
    session(|| {
        let mut snapshots = Vec::new();
        for prefix in [false, true] {
            let map = SourceMap::new(FilePathMapping::empty());
            if prefix {
                map.new_source_file(FileName::Custom("prefix".into()), "padding".into());
            }
            let file = imported(&map, "dep.rs", "abcdefgh", 2);
            let (context, metadata) = fixture(&file);
            let captured =
                sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                    .unwrap();
            let State::Loaded(owner) = &captured.records()[0].state else {
                panic!("loaded owner required")
            };
            snapshots.push((
                owner.index,
                owner.start,
                owner.name.clone(),
                owner.stable_id,
                owner.hash,
                captured.current_range(packed(1002, 1006)).unwrap(),
            ));
        }
        assert_ne!(snapshots[0].0, snapshots[1].0);
        assert_ne!(snapshots[0].1, snapshots[1].1);
        assert_ne!(snapshots[0].5, snapshots[1].5);
        assert_eq!(snapshots[0].2, snapshots[1].2);
        assert_eq!(snapshots[0].3, snapshots[1].3);
        assert_eq!(snapshots[0].4, snapshots[1].4);
        // No prepared span, comparison, codec or eligibility is changed here.
    });
}

#[test]
fn allocation_name_limit_and_last_owner_release_are_bounded() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::Budget)
        ));
        drop(captured);
        drop(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap(),
        );
        let huge = SourceMap::new(FilePathMapping::empty());
        let oversized = imported(&huge, &"x".repeat(4097), "abcdefgh", 2);
        let (context, metadata) = fixture(&oversized);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &huge, crate_id),
            Err(CaptureFailure::Budget)
        ));
        let small = SourceMap::new(FilePathMapping::empty());
        let normal = imported(&small, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&normal);
        drop(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &small, crate_id)
                .unwrap(),
        );
    });
}

#[test]
fn disposal_precedes_releasing_the_process_family_admission() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        let (sent, disposed) = std::sync::mpsc::channel();
        let (resume, receive) = std::sync::mpsc::channel();
        capture::drop_probe::install(sent, receive);
        let dropping = std::thread::spawn(move || drop(captured));
        let disposal = disposed.recv_timeout(std::time::Duration::from_secs(1));
        let competing =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id);
        // Release/join the owned test thread before asserting any observations.
        let _ = resume.send(());
        let joined = dropping.join();
        assert_eq!(disposal.unwrap(), (0, 0));
        assert!(matches!(competing, Err(CaptureFailure::Budget)));
        assert!(joined.is_ok());
        drop(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap(),
        );
        let mut too_many = context.imported_crates[&7].files.clone();
        too_many.resize(4097, too_many[0].clone());
        assert!(matches!(
            sail_capture_loaded_imports(
                &HashMap::from([(7, CrateInfo { files: too_many })]),
                &metadata,
                &map,
                crate_id
            ),
            Err(CaptureFailure::Budget)
        ));
    });
}

#[test]
fn original_hash_and_normalized_byte_lengths_are_kept_distinct() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let name = FileName::Custom("normalized-dep.rs".into());
        let original = rustc_span::SourceFile::new(
            name.clone(),
            "a\r\né".into(),
            SourceFileHashAlgorithm::Sha256,
            None,
        )
        .unwrap();
        assert_ne!(
            original.normalized_source_len.0,
            original.unnormalized_source_len
        );
        let id = rustc_span::def_id::StableCrateId::new(
            Symbol::intern("dep"),
            false,
            Vec::new(),
            "1.98.1",
        );
        let file = map.new_imported_source_file(
            name.clone(),
            original.src_hash,
            None,
            StableSourceFileId::from_filename_for_export(&name, id),
            original.normalized_source_len.0,
            original.unnormalized_source_len,
            CrateNum::new(2),
            original.lines.clone(),
            original.multibyte_chars.clone(),
            original.normalized_pos.clone(),
            0,
        );
        let (context, mut metadata) = fixture(&file);
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        let State::Loaded(owner) = &captured.records()[0].state else {
            panic!("loaded owner required")
        };
        assert_eq!(owner.normalized_length, original.normalized_source_len.0);
        assert_eq!(owner.unnormalized_length, original.unnormalized_source_len);
        assert_eq!(owner.hash, original.src_hash);
        let words = packed(1000, 1000 + original.normalized_source_len.0);
        let stock = context
            .unpack_span(&words.to_vec(), Some(&map))
            .unwrap()
            .data();
        assert_eq!(
            captured.current_range(words).unwrap(),
            (stock.lo.0, stock.hi.0)
        );
        drop(captured);
        metadata
            .get_mut(&7)
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .end_pos += 1;
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::MissingOriginalHash)
        ));
    });
}

#[test]
fn ordinary_stock_resolution_can_load_after_capture_but_never_promotes_the_snapshot() {
    session(|| {
        struct RemoveFile(std::path::PathBuf);
        impl Drop for RemoveFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sail-import-timing-{}-{suffix}.rs",
            std::process::id()
        ));
        let cleanup = RemoveFile(path.clone());
        let content = "abcdefgh";
        let hash = SourceFileHash::new_in_memory(SourceFileHashAlgorithm::Md5, content);
        std::fs::write(&path, content).unwrap();
        let map = SourceMap::new(FilePathMapping::empty());
        let context = SpanContextX {
            local_crate: rustc_span::def_id::StableCrateId::new(
                Symbol::intern("local"),
                false,
                Vec::new(),
                "1.98.1",
            ),
            next_span_id: std::sync::atomic::AtomicU64::new(1),
            local_files: HashMap::new(),
            sail_imports: Mutex::new(None),
            imported_crates: HashMap::from([(
                7,
                CrateInfo {
                    files: vec![ExternSourceFile {
                        original_start_pos: BytePos(1000),
                        original_end_pos: BytePos(1008),
                        info: Arc::new(Mutex::new(ExternSourceInfo::Delayed {
                            filename: path.clone(),
                            hash: hash.hash_bytes().to_vec(),
                        })),
                    }],
                },
            )]),
        };
        let metadata = HashMap::from([(
            7,
            HashMap::from([(
                hash.hash_bytes().to_vec(),
                FileStartEndPos {
                    filename: Some(path),
                    start_pos: 1000,
                    end_pos: 1008,
                },
            )]),
        )]);
        let captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        assert!(map.files().is_empty());
        assert_eq!(
            captured.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Delayed)
        );
        // The unchanged stock resolver, rather than the helper, performs I/O.
        let stock = context
            .unpack_span(&packed(1002, 1006).to_vec(), Some(&map))
            .unwrap()
            .data();
        assert_eq!(map.files().len(), 1);
        assert!(matches!(
            *context.imported_crates[&7].files[0].info.lock().unwrap(),
            ExternSourceInfo::Loaded { .. }
        ));
        assert_eq!(stock.hi.0 - stock.lo.0, 4);
        assert_eq!(stock.ctxt, rustc_span::SyntaxContext::root());
        assert_eq!(stock.parent, None);
        assert_eq!(
            captured.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Delayed)
        );
        drop(captured);
        // A later independent observation sees a locally loaded fallback;
        // the initial Foreign-only proposal still refuses it.
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::NotForeign)
        ));
        drop(cleanup);
    });
}

#[test]
fn current_unit_import_resolver_tracks_exact_twenty_five_byte_loaded_relocation() {
    std::thread::Builder::new()
        .name("rustc".into())
        .spawn(|| {
            session(|| {
                let mut observed = Vec::new();
                for prefix_length in [0, 25] {
                    let map = SourceMap::new(FilePathMapping::empty());
                    map.new_source_file(
                        FileName::Custom("prefix".into()),
                        "x".repeat(prefix_length),
                    );
                    let file = imported(&map, "dep.rs", "abcdefgh", 2);
                    let (context, metadata) = fixture(&file);
                    let mapping = sail_capture_loaded_imports(
                        &context.imported_crates,
                        &metadata,
                        &map,
                        crate_id,
                    )
                    .unwrap();
                    let current = vir::messages::Span {
                        raw_span: Arc::new(()),
                        id: 563,
                        data: packed(1002, 1006).to_vec(),
                        as_string: "dep.rs:1:3:1:7 (#0)".into(),
                    };
                    assert!(((&*current.raw_span) as &dyn std::any::Any).is::<()>());
                    let resolved = context.from_air_span(&current, Some(&map)).unwrap();
                    assert_eq!(
                        mapping.current_range(packed(1002, 1006)).unwrap(),
                        (resolved.data().lo.0, resolved.data().hi.0)
                    );
                    assert_eq!(resolved.data().ctxt, rustc_span::SyntaxContext::root());
                    assert_eq!(resolved.data().parent, None);
                    let State::Loaded(owner) = &mapping.records()[0].state else {
                        panic!("expected current Loaded authority")
                    };
                    observed.push((
                        owner.index,
                        owner.start,
                        owner.name.clone(),
                        owner.stable_id,
                        owner.hash,
                        (current.id, current.data, current.as_string),
                        map.span_to_string(
                            resolved,
                            rustc_span::RemapPathScopeComponents::DIAGNOSTICS,
                        ),
                    ));
                }
                assert_eq!(observed[0].0, observed[1].0);
                assert_eq!(observed[1].1 - observed[0].1, 25);
                assert_eq!(observed[0].2, observed[1].2);
                assert_eq!(observed[0].3, observed[1].3);
                assert_eq!(observed[0].4, observed[1].4);
                assert_eq!(observed[0].5, observed[1].5);
                assert_eq!(observed[0].6, observed[1].6);
                // No preparation eligibility or stored provenance is changed.
            });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn delayed_current_unit_import_cannot_borrow_an_existing_foreign_mapping() {
    std::thread::Builder::new()
        .name("rustc".into())
        .spawn(|| {
            session(|| {
                struct RemoveFile(std::path::PathBuf);
                impl Drop for RemoveFile {
                    fn drop(&mut self) {
                        let _ = std::fs::remove_file(&self.0);
                    }
                }
                let suffix = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let path = std::env::temp_dir().join(format!(
                    "sail-existing-foreign-{}-{suffix}.rs",
                    std::process::id()
                ));
                let cleanup = RemoveFile(path.clone());
                let map = SourceMap::new(FilePathMapping::empty());
                let content = "abcdefgh";
                std::fs::write(&path, content).unwrap();
                let hash = SourceFileHash::new_in_memory(SourceFileHashAlgorithm::Md5, content);
                let name = FileName::Custom("existing-foreign".into());
                let stable_crate = rustc_span::def_id::StableCrateId::new(
                    Symbol::intern("dep"),
                    false,
                    Vec::new(),
                    "1.98.1",
                );
                let foreign = map.new_imported_source_file(
                    name.clone(),
                    hash,
                    None,
                    StableSourceFileId::from_filename_for_export(&name, stable_crate),
                    8,
                    8,
                    CrateNum::new(2),
                    FreezeLock::new(SourceFileLines::Lines(vec![RelativeBytePos::from_u32(0)])),
                    Vec::new(),
                    Vec::new(),
                    0,
                );
                let (context, metadata) = fixture(&foreign);
                *context.imported_crates[&7].files[0].info.lock().unwrap() =
                    ExternSourceInfo::Delayed {
                        filename: path,
                        hash: hash.hash_bytes().to_vec(),
                    };
                let mapping = sail_capture_loaded_imports(
                    &context.imported_crates,
                    &metadata,
                    &map,
                    crate_id,
                )
                .unwrap();
                assert_eq!(map.files().len(), 1);
                assert_eq!(
                    mapping.current_range(packed(1002, 1006)),
                    Err(CaptureFailure::Delayed)
                );
                let current = vir::messages::Span {
                    raw_span: Arc::new(()),
                    id: 563,
                    data: packed(1002, 1006).to_vec(),
                    as_string: "same exact current diagnostic binding".into(),
                };
                let resolved = context.from_air_span(&current, Some(&map)).unwrap();
                assert_eq!(map.files().len(), 2);
                // Stock honours the Delayed descriptor, even though an equal-hash
                // Foreign source already existed. Its newly loaded Real file wins.
                assert_ne!(map.span_to_filename(resolved), foreign.name);
                assert_ne!(resolved.data().lo.0, foreign.start_pos.0 + 2);
                assert_eq!(
                    mapping.current_range(packed(1002, 1006)),
                    Err(CaptureFailure::Delayed)
                );
                drop(mapping);
                assert!(matches!(
                    sail_capture_loaded_imports(
                        &context.imported_crates,
                        &metadata,
                        &map,
                        crate_id,
                    ),
                    Err(CaptureFailure::NotForeign)
                ));
                drop(cleanup);
            });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn pre_focus_barrier_preserves_original_hash_and_refuses_late_ambiguity() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let mut captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        let words = packed(1002, 1006);
        let original = captured.records()[0].expected_hash;
        captured.revalidate(&context.imported_crates, &map);
        assert!(captured.current_range(words).is_ok());
        assert_eq!(captured.records()[0].expected_hash, original);
        imported(&map, "duplicate.rs", "abcdefgh", 2);
        captured.revalidate(&context.imported_crates, &map);
        assert_eq!(
            captured.current_range(words),
            Err(CaptureFailure::AmbiguousContentOwner)
        );
        assert_eq!(captured.records()[0].expected_hash, original);
    });
}

#[test]
fn pre_focus_barrier_never_promotes_delayed_or_accepts_changed_resolver() {
    session(|| {
        let map = SourceMap::new(FilePathMapping::empty());
        let file = imported(&map, "dep.rs", "abcdefgh", 2);
        let (context, metadata) = fixture(&file);
        let state = &context.imported_crates[&7].files[0].info;
        let saved = state.lock().unwrap().clone();
        *state.lock().unwrap() = ExternSourceInfo::Delayed {
            filename: "must-not-load".into(),
            hash: file.src_hash.hash_bytes().to_vec(),
        };
        let mut captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        *state.lock().unwrap() = saved.clone();
        captured.revalidate(&context.imported_crates, &map);
        assert_eq!(
            captured.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Delayed)
        );
        drop(captured);
        let mut captured =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id)
                .unwrap();
        *state.lock().unwrap() = ExternSourceInfo::Loaded {
            start_pos: BytePos(file.start_pos.0 + 1),
            end_pos: BytePos(file.start_pos.0 + 9),
        };
        captured.revalidate(&context.imported_crates, &map);
        assert_eq!(
            captured.current_range(packed(1002, 1006)),
            Err(CaptureFailure::ChangedResolver)
        );
    });
}


use super::sail_owned_imports as capture;
use super::*;
use capture::{CaptureFailure, State, sail_capture_loaded_imports};
use rustc_data_structures::sync::FreezeLock;
use rustc_span::def_id::CrateNum;
use rustc_span::source_map::{FileLoader, FilePathMapping, SourceMapInputs};
use rustc_span::{
    Pos, RelativeBytePos, SourceFileHash, SourceFileHashAlgorithm, SourceFileLines,
    StableSourceFileId, Symbol,
};
use std::sync::atomic::{AtomicUsize, Ordering};

static SERIAL: Mutex<()> = Mutex::new(());
fn session(f: impl FnOnce()) {
    let _guard = SERIAL.lock().unwrap();
    rustc_span::create_session_if_not_set_then(rustc_span::edition::Edition::Edition2021, |_| f());
}
struct CountLoader {
    reads: Arc<AtomicUsize>,
    content: Option<String>,
}
impl FileLoader for CountLoader {
    fn file_exists(&self, _: &std::path::Path) -> bool {
        self.content.is_some()
    }
    fn read_file(&self, _: &std::path::Path) -> std::io::Result<String> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.content
            .clone()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
    }
    fn read_binary_file(&self, _: &std::path::Path) -> std::io::Result<Arc<[u8]>> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
    fn current_directory(&self) -> std::io::Result<std::path::PathBuf> {
        Ok("/".into())
    }
}
fn imported(map: &SourceMap, name: &str, text: &str) -> Arc<rustc_span::SourceFile> {
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
        CrateNum::new(2),
        FreezeLock::new(SourceFileLines::Lines(vec![RelativeBytePos::from_u32(0)])),
        Vec::new(),
        Vec::new(),
        0,
    )
}
fn fixture(
    content: Option<&str>,
    prefix: usize,
) -> (
    SourceMap,
    SpanContextX,
    HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
    Arc<AtomicUsize>,
) {
    let reads = Arc::new(AtomicUsize::new(0));
    let map = SourceMap::with_inputs(SourceMapInputs {
        file_loader: Box::new(CountLoader {
            reads: reads.clone(),
            content: content.map(str::to_owned),
        }),
        path_mapping: FilePathMapping::empty(),
        hash_kind: SourceFileHashAlgorithm::Sha256,
        checksum_hash_kind: None,
    });
    if prefix > 0 {
        map.new_source_file(FileName::Custom("prefix".into()), "x".repeat(prefix));
    }
    let file = imported(&map, "foreign.rs", "abcdefgh");
    let path = std::path::PathBuf::from("/exact/deferred.rs");
    let metadata = HashMap::from([(
        7,
        HashMap::from([(
            file.src_hash.hash_bytes().to_vec(),
            FileStartEndPos {
                filename: Some(path.clone()),
                start_pos: 1000,
                end_pos: 1008,
            },
        )]),
    )]);
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
                        filename: path,
                        hash: file.src_hash.hash_bytes().to_vec(),
                    })),
                }],
            },
        )]),
    };
    (map, context, metadata, reads)
}
fn crate_id(cnum: CrateNum) -> u64 {
    if cnum == CrateNum::new(2) { 7 } else { 8 }
}
fn packed(lo: u32, hi: u32) -> [u64; 2] {
    [7, (u64::from(lo) << 32) | u64::from(hi)]
}
fn capture(
    context: &SpanContextX,
    metadata: &HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
    map: &SourceMap,
) -> capture::OwnedImportMappings {
    sail_capture_loaded_imports(&context.imported_crates, metadata, map, crate_id).unwrap()
}
fn rustc_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("rustc".into())
        .spawn(move || session(f))
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn deferred_capture_and_handoff_do_not_read_or_claim_loaded_coordinates() {
    session(|| {
        let (map, context, metadata, reads) = fixture(Some("abcdefgh"), 0);
        let mut owner = capture(&context, &metadata, &map);
        owner.revalidate(&context.imported_crates, &map);
        let (record, recipe) = owner.deferred(packed(1002, 1006)).unwrap();
        assert_eq!(recipe.loader_path, "/exact/deferred.rs");
        assert!(matches!(record.state, State::Deferred(_)));
        assert!(matches!(
            record.initial.state,
            capture::ResolverState::Delayed
        ));
        assert!(matches!(
            record.pre_focus.state,
            capture::ResolverState::Delayed
        ));
        assert_eq!((record.initial.start, record.pre_focus.start), (None, None));
        assert_eq!(
            owner.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Delayed)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(map.files().len(), 1);
        assert!(matches!(
            *context.imported_crates[&7].files[0].info.lock().unwrap(),
            ExternSourceInfo::Delayed { .. }
        ));
        assert_eq!(capture::retained_bytes(), 8 * 1024 * 1024);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::Budget)
        ));
        drop(owner);
        assert_eq!(capture::retained_bytes(), 0);
    });
}

#[test]
fn current_stock_success_failure_and_repeat_preserve_the_immutable_recipe() {
    rustc_thread(|| {
        for content in [Some("abcdefgh"), Some("wrong!!!"), None] {
            let (map, context, metadata, reads) = fixture(content, 25);
            let mut owner = capture(&context, &metadata, &map);
            owner.revalidate(&context.imported_crates, &map);
            let current = vir::messages::Span {
                raw_span: Arc::new(()),
                id: 17,
                data: packed(1002, 1006).to_vec(),
                as_string: "exact current spelling".into(),
            };
            assert!(from_raw_span(&current.raw_span).is_none());
            let first = context.from_air_span(&current, Some(&map));
            let second = context.from_air_span(&current, Some(&map));
            assert_eq!(first, second);
            assert_eq!(reads.load(Ordering::SeqCst), 1);
            assert_eq!(first.is_some(), content == Some("abcdefgh"));
            if let Some(span) = first {
                assert_eq!(span.data().ctxt, rustc_span::SyntaxContext::root());
                assert_eq!(span.data().parent, None);
                assert_ne!(
                    span.data().lo.0,
                    owner.deferred(packed(1002, 1006)).unwrap().1.foreign.start + 2
                );
                assert!(matches!(
                    *context.imported_crates[&7].files[0].info.lock().unwrap(),
                    ExternSourceInfo::Loaded { .. }
                ));
            } else {
                assert!(matches!(
                    *context.imported_crates[&7].files[0].info.lock().unwrap(),
                    ExternSourceInfo::None
                ));
            }
            assert!(owner.deferred(packed(1002, 1006)).is_ok());
            assert_eq!(
                owner.current_range(packed(1002, 1006)),
                Err(CaptureFailure::Delayed)
            );
            drop(owner);
        }
    });
}

#[test]
fn handoff_refuses_changed_recipe_early_resolution_busy_and_namespace_ambiguity() {
    session(|| {
        for variation in 0..6 {
            let (map, context, metadata, reads) = fixture(Some("abcdefgh"), 0);
            let mut owner = capture(&context, &metadata, &map);
            let info = &context.imported_crates[&7].files[0].info;
            match variation {
                0 => {
                    if let ExternSourceInfo::Delayed { filename, .. } = &mut *info.lock().unwrap() {
                        *filename = "/different/deferred.rs".into();
                    }
                }
                1 => {
                    if let ExternSourceInfo::Delayed { hash, .. } = &mut *info.lock().unwrap() {
                        hash[0] ^= 1;
                    }
                }
                2 => {
                    *info.lock().unwrap() = ExternSourceInfo::None;
                }
                3 => {
                    *info.lock().unwrap() = ExternSourceInfo::Loaded {
                        start_pos: BytePos(0),
                        end_pos: BytePos(8),
                    };
                }
                4 => {
                    imported(&map, "duplicate.rs", "abcdefgh");
                }
                _ => {
                    let guard = info.lock().unwrap();
                    owner.revalidate(&context.imported_crates, &map);
                    drop(guard);
                    assert!(owner.deferred(packed(1002, 1006)).is_err());
                    continue;
                }
            }
            owner.revalidate(&context.imported_crates, &map);
            assert!(owner.deferred(packed(1002, 1006)).is_err());
            assert_eq!(reads.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn path_original_hash_range_and_ambiguous_owner_controls_refuse_without_io() {
    session(|| {
        for path in [
            "relative.rs".to_owned(),
            "/nul\0.rs".to_owned(),
            format!("/{}", "x".repeat(4096)),
        ] {
            let (map, context, metadata, reads) = fixture(None, 0);
            if let ExternSourceInfo::Delayed { filename, .. } =
                &mut *context.imported_crates[&7].files[0].info.lock().unwrap()
            {
                *filename = path.into();
            }
            let owner = capture(&context, &metadata, &map);
            assert!(owner.deferred(packed(1002, 1006)).is_err());
            assert_eq!(reads.load(Ordering::SeqCst), 0);
        }
        let (map, context, mut metadata, reads) = fixture(None, 0);
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
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(capture::retained_bytes(), 0);
        let (map, context, metadata, reads) = fixture(None, 0);
        imported(&map, "second.rs", "abcdefgh");
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::AmbiguousContentOwner)
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(capture::retained_bytes(), 0);
    });
}

#[cfg(unix)]
#[test]
fn non_utf8_descriptor_is_a_safe_miss_without_lossy_identity() {
    use std::os::unix::ffi::OsStringExt;
    session(|| {
        let (map, context, metadata, reads) = fixture(None, 0);
        if let ExternSourceInfo::Delayed { filename, .. } =
            &mut *context.imported_crates[&7].files[0].info.lock().unwrap()
        {
            *filename = std::ffi::OsString::from_vec(b"/bad\xff.rs".to_vec()).into();
        }
        let owner = capture(&context, &metadata, &map);
        assert!(owner.deferred(packed(1002, 1006)).is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn deferred_physical_disposal_holds_admission_against_a_racing_capture() {
    session(|| {
        let (map, context, metadata, _) = fixture(None, 0);
        let owner = capture(&context, &metadata, &map);
        let (sent, disposed) = std::sync::mpsc::channel();
        let (resume, receive) = std::sync::mpsc::channel();
        capture::drop_probe::install(sent, receive);
        let dropping = std::thread::spawn(move || drop(owner));
        let physical = disposed.recv_timeout(std::time::Duration::from_secs(1));
        let competing =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id);
        let _ = resume.send(());
        let joined = dropping.join();
        assert_eq!(physical.unwrap(), (0, 0));
        assert!(matches!(competing, Err(CaptureFailure::Budget)));
        assert!(joined.is_ok());
        drop(capture(&context, &metadata, &map));
    });
}

#[test]
fn stock_duplicate_stable_id_reuse_never_becomes_a_recipe_content_claim() {
    rustc_thread(|| {
        let (map, context, metadata, reads) = fixture(Some("wrong!!!"), 0);
        let filename = FileName::Real(map.path_mapping().to_real_filename(
            map.working_dir(),
            std::path::Path::new("/exact/deferred.rs"),
        ));
        let existing = map.new_source_file(filename, "abcdefgh".into());
        let mut owner = capture(&context, &metadata, &map);
        owner.revalidate(&context.imported_crates, &map);
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        let current = vir::messages::Span {
            raw_span: Arc::new(()),
            id: 17,
            data: packed(1002, 1006).to_vec(),
            as_string: "same current spelling".into(),
        };
        let resolved = context.from_air_span(&current, Some(&map)).unwrap().data();
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        // The stock API reads wrong text but returns its prior same-id file.
        // Follow that current behavior; never certify that loader content.
        assert_eq!(resolved.lo.0, existing.start_pos.0 + 2);
        assert_eq!(map.files().len(), 2);
        assert!(owner.deferred(packed(1002, 1006)).is_ok());
        assert_eq!(
            owner.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Delayed)
        );
    });
}

#[test]
fn ordered_primary_label_resolution_uses_the_current_descriptor_on_both_arms() {
    rustc_thread(|| {
        for content in [Some("abcdefgh"), Some("wrong!!!"), None] {
            fn arm(
                content: Option<&str>,
                prepared: bool,
            ) -> (Vec<(Option<String>, Option<String>)>, usize) {
                let (map, context, metadata, reads) = fixture(content, 25);
                let owner = if prepared {
                    let mut owner = capture(&context, &metadata, &map);
                    owner.revalidate(&context.imported_crates, &map);
                    Some(owner)
                } else {
                    None
                };
                // Same ordered primary+label recipes; this helper exercises stock
                // resolver/rendering APIs, NOT rustc's complete Reporter emitter.
                let span = |lo, hi| vir::messages::Span {
                    raw_span: Arc::new(()),
                    id: 17,
                    data: packed(lo, hi).to_vec(),
                    as_string: "current raw unit".into(),
                };
                let mut rendered = Vec::new();
                for (primary, label) in [
                    (span(1002, 1006), span(1001, 1002)),
                    (span(1006, 1007), span(1000, 1001)),
                ] {
                    assert!(from_raw_span(&primary.raw_span).is_none());
                    let resolve = |span: &vir::messages::Span| {
                        context
                            .from_air_span(span, Some(&map))
                            .map(|s| map.span_to_diagnostic_string(s))
                    };
                    rendered.push((resolve(&primary), resolve(&label)));
                }
                if let Some(owner) = owner {
                    assert!(owner.deferred(packed(1002, 1006)).is_ok());
                }
                (rendered, reads.load(Ordering::SeqCst))
            }
            assert_eq!(arm(content, false), arm(content, true));
            assert_eq!(arm(content, true).1, 1);
        }
    });
}

#[test]
fn descriptor_range_spanning_current_files_cannot_be_a_loaded_owner() {
    session(|| {
        let (map, context, metadata, reads) = fixture(None, 0);
        let last = imported(&map, "next.rs", "different");
        *context.imported_crates[&7].files[0].info.lock().unwrap() = ExternSourceInfo::Loaded {
            start_pos: BytePos(0),
            end_pos: BytePos(last.start_pos.0 + last.normalized_source_len.0),
        };
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id),
            Err(CaptureFailure::InvalidRange)
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(capture::retained_bytes(), 0);
    });
}

// Distinct original-descriptor controls; all earlier role assertions unchanged.
fn original_fixture(
    content: Option<&str>,
) -> (
    SourceMap,
    SpanContextX,
    HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
    Arc<AtomicUsize>,
) {
    let (old_map, context, metadata, reads) = fixture(content, 0);
    drop(old_map);
    let map = SourceMap::with_inputs(SourceMapInputs {
        file_loader: Box::new(CountLoader {
            reads: reads.clone(),
            content: content.map(str::to_owned),
        }),
        path_mapping: FilePathMapping::empty(),
        hash_kind: SourceFileHashAlgorithm::Sha256,
        checksum_hash_kind: None,
    });
    (map, context, metadata, reads)
}
#[test]
fn original_descriptor_has_exact_original_authority_without_source_or_io() {
    session(|| {
        let (map, context, metadata, reads) = original_fixture(Some("abcdefgh"));
        let mut owner = capture(&context, &metadata, &map);
        // The immutable original table is NOT reread at handoff.
        drop(metadata);
        owner.revalidate(&context.imported_crates, &map);
        let (record, recipe) = owner.original_descriptor(packed(1002, 1006)).unwrap();
        assert_eq!(recipe.loader_path, "/exact/deferred.rs");
        assert!(matches!(record.state, State::OriginalDescriptor(_)));
        assert_eq!(record.initial_foreign_hash_matches, Some(0));
        assert_eq!(record.pre_focus_foreign_hash_matches, Some(0));
        assert_eq!((record.original_start, record.original_end), (1000, 1008));
        assert_eq!(
            &record.expected_hash[..usize::from(record.expected_hash_len)],
            SourceFileHash::new_in_memory(SourceFileHashAlgorithm::Sha256, "abcdefgh").hash_bytes()
        );
        assert_eq!(
            owner.current_range(packed(1002, 1006)),
            Err(CaptureFailure::Delayed)
        );
        assert!(owner.deferred(packed(1002, 1006)).is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(map.files().is_empty());
        assert_eq!(capture::retained_bytes(), 8 * 1024 * 1024);
        assert!(matches!(
            sail_capture_loaded_imports(&context.imported_crates, &HashMap::new(), &map, crate_id),
            Err(CaptureFailure::Budget)
        ));
        drop(owner);
        assert_eq!(capture::retained_bytes(), 0);
    });
}
#[test]
fn original_descriptor_rejects_changed_original_and_current_recipes() {
    session(|| {
        for original in [None, Some("/different/original.rs")] {
            let (map, context, mut metadata, reads) = original_fixture(None);
            metadata
                .get_mut(&7)
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .filename = original.map(Into::into);
            let owner = capture(&context, &metadata, &map);
            assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
            assert!(
                owner
                    .record(packed(1002, 1006))
                    .unwrap()
                    .descriptor_refusal
                    .is_some()
            );
            assert_eq!(reads.load(Ordering::SeqCst), 0);
        }
        for state in [
            ExternSourceInfo::None,
            ExternSourceInfo::Loaded {
                start_pos: BytePos(1),
                end_pos: BytePos(9),
            },
            ExternSourceInfo::Delayed {
                filename: "/changed/current.rs".into(),
                hash: SourceFileHash::new_in_memory(SourceFileHashAlgorithm::Sha256, "abcdefgh")
                    .hash_bytes()
                    .to_vec(),
            },
            ExternSourceInfo::Delayed {
                filename: "/exact/deferred.rs".into(),
                hash: vec![0; 32],
            },
        ] {
            let (map, context, metadata, reads) = original_fixture(None);
            let mut owner = capture(&context, &metadata, &map);
            *context.imported_crates[&7].files[0].info.lock().unwrap() = state;
            owner.revalidate(&context.imported_crates, &map);
            assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
            assert_eq!(reads.load(Ordering::SeqCst), 0);
        }
        let (map, context, metadata, _) = original_fixture(None);
        let mut owner = capture(&context, &metadata, &map);
        let held = context.imported_crates[&7].files[0].info.lock().unwrap();
        owner.revalidate(&context.imported_crates, &map);
        assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
        drop(held);
    });
}
#[test]
fn original_descriptor_rejects_foreign_hash_across_all_crates_at_both_barriers() {
    session(|| {
        let (map, context, metadata, reads) = original_fixture(None);
        imported(&map, "other-crate.rs", "abcdefgh");
        let owner =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, |_| 8).unwrap();
        let record = owner.record(packed(1002, 1006)).unwrap();
        assert_eq!(
            record.descriptor_refusal,
            Some(capture::DescriptorRefusal::ForeignHashElsewhere)
        );
        assert_eq!(record.initial_foreign_hash_matches, Some(1));
        assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        drop(owner);
        let (map, context, metadata, reads) = original_fixture(None);
        let mut owner = capture(&context, &metadata, &map);
        imported(&map, "new-foreign.rs", "abcdefgh");
        owner.revalidate(&context.imported_crates, &map);
        assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
        assert_eq!(
            owner
                .record(packed(1002, 1006))
                .unwrap()
                .pre_focus_foreign_hash_matches,
            Some(1)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    });
}
#[test]
fn original_descriptor_preserves_current_stock_resolution_success_failure_and_repeat() {
    rustc_thread(|| {
        for content in [Some("abcdefgh"), Some("wrong!!!"), None] {
            let (map, context, metadata, reads) = original_fixture(content);
            let mut owner = capture(&context, &metadata, &map);
            owner.revalidate(&context.imported_crates, &map);
            let current = vir::messages::Span {
                raw_span: Arc::new(()),
                id: 17,
                data: packed(1002, 1006).to_vec(),
                as_string: "current exact original recipe".into(),
            };
            let first = context.from_air_span(&current, Some(&map));
            assert_eq!(first, context.from_air_span(&current, Some(&map)));
            assert_eq!(first.is_some(), content == Some("abcdefgh"));
            assert_eq!(reads.load(Ordering::SeqCst), 1);
            assert!(owner.original_descriptor(packed(1002, 1006)).is_ok());
            assert_eq!(
                owner.current_range(packed(1002, 1006)),
                Err(CaptureFailure::Delayed)
            );
            if let Some(span) = first {
                assert_eq!(span.data().ctxt, rustc_span::SyntaxContext::root());
                assert_eq!(span.data().parent, None);
            }
        }
    });
}

#[test]
fn original_descriptor_physical_disposal_precedes_family_release() {
    session(|| {
        let (map, context, metadata, _) = original_fixture(None);
        let owner = capture(&context, &metadata, &map);
        assert!(owner.original_descriptor(packed(1002, 1006)).is_ok());
        let (sent, disposed) = std::sync::mpsc::channel();
        let (resume, receive) = std::sync::mpsc::channel();
        capture::drop_probe::install(sent, receive);
        let dropping = std::thread::spawn(move || drop(owner));
        let physical = disposed.recv_timeout(std::time::Duration::from_secs(1));
        let competing =
            sail_capture_loaded_imports(&context.imported_crates, &metadata, &map, crate_id);
        let _ = resume.send(());
        let joined = dropping.join();
        assert_eq!(physical.unwrap(), (0, 0));
        assert!(matches!(competing, Err(CaptureFailure::Budget)));
        assert!(joined.is_ok());
        drop(capture(&context, &metadata, &map));
    });
}
#[test]
fn original_descriptor_unsupported_path_remains_unavailable_with_precise_reason() {
    session(|| {
        for path in [
            "relative.rs".to_owned(),
            "/bad\0name".to_owned(),
            format!("/{}", "x".repeat(4096)),
        ] {
            let (map, context, mut metadata, reads) = original_fixture(None);
            metadata
                .get_mut(&7)
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .filename = Some(path.clone().into());
            if let ExternSourceInfo::Delayed { filename, .. } =
                &mut *context.imported_crates[&7].files[0].info.lock().unwrap()
            {
                *filename = path.into();
            }
            let owner = capture(&context, &metadata, &map);
            assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
            assert_eq!(
                owner.record(packed(1002, 1006)).unwrap().descriptor_refusal,
                Some(capture::DescriptorRefusal::UnsupportedPath)
            );
            assert_eq!(reads.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn original_descriptor_handoff_refuses_oversized_namespace_before_foreign_count() {
    session(|| {
        let (map, context, metadata, reads) = original_fixture(None);
        let mut owner = capture(&context, &metadata, &map);
        for index in 0..4097 {
            map.new_source_file(FileName::Custom(format!("extra-{index}")), "x".into());
        }
        owner.revalidate(&context.imported_crates, &map);
        let record = owner.record(packed(1002, 1006)).unwrap();
        assert!(matches!(
            record.state,
            State::Unavailable(CaptureFailure::Budget)
        ));
        assert_eq!(record.pre_focus_foreign_hash_matches, None);
        assert!(owner.original_descriptor(packed(1002, 1006)).is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    });
}

#[cfg(sail_vstd_metadata)]
include!("metadata-control.rs");

// Actual pinned phase controls. No compiler verdict/HIT claim.
fn pending(context:&SpanContextX,metadata:&HashMap<u64,HashMap<Vec<u8>,FileStartEndPos>>) -> capture::PendingImportSeeds {
    capture::sail_capture_import_seeds(&context.imported_crates,metadata).unwrap()
}
fn install_pending(context:&SpanContextX,metadata:&HashMap<u64,HashMap<Vec<u8>,FileStartEndPos>>) {
    *context.sail_imports.lock().unwrap()=Some(Ok(pending(context,metadata)));
}
#[test]
fn late_handoff_classifies_new_foreign_once_without_loading_or_second_owner() {
    session(|| {
        let (map,context,metadata,reads)=original_fixture(None);
        install_pending(&context,&metadata);
        assert_eq!(capture::retained_bytes(),8*1024*1024);
        let file=imported(&map,"late.rs","abcdefgh");
        let owner=context.sail_with_imports(&map,crate_id,|files,imports|{
            assert_eq!(files.len(),1);
            let owner=imports.unwrap();
            let (_,recipe)=owner.deferred(packed(1002,1006)).unwrap();
            assert_eq!(recipe.foreign.stable_id,file.stable_id);
            assert_eq!(recipe.loader_path,"/exact/deferred.rs");
            assert_eq!(owner.current_range(packed(1002,1006)),Err(CaptureFailure::Delayed));
            assert!(owner.validate_binding_files(packed(1002,1006),files).is_ok());
            owner
        });
        assert_eq!(reads.load(Ordering::SeqCst),0);
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports,Err(CaptureFailure::Missing))));
        assert_eq!(capture::retained_bytes(),8*1024*1024);
        drop(owner);assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn initial_actual_recipe_mutation_cannot_be_promoted_by_late_foreign() {
    session(|| {
        for change in 0..4 {
            let (map,context,metadata,reads)=original_fixture(None);install_pending(&context,&metadata);
            let file=imported(&map,"late.rs","abcdefgh");
            let replacement=match change {
                0=>ExternSourceInfo::Delayed{filename:"/different.rs".into(),hash:file.src_hash.hash_bytes().to_vec()},
                1=>ExternSourceInfo::Delayed{filename:"/exact/deferred.rs".into(),hash:vec![0;32]},
                2=>ExternSourceInfo::None,
                _=>ExternSourceInfo::Loaded{start_pos:file.start_pos,end_pos:BytePos(file.start_pos.0+8)},
            };
            *context.imported_crates[&7].files[0].info.lock().unwrap()=replacement;
            context.sail_with_imports(&map,crate_id,|_,imports|{
                let owner=imports.unwrap();assert!(matches!(owner.records()[0].state,State::Unavailable(CaptureFailure::ChangedResolver)));
                assert!(owner.deferred(packed(1002,1006)).is_err());
            });
            assert_eq!(reads.load(Ordering::SeqCst),0);assert_eq!(capture::retained_bytes(),0);
        }
    });
}
#[test]
fn current_equal_hash_in_other_crate_is_still_a_refusal() {
    session(|| {
        let (map,context,metadata,reads)=original_fixture(None);install_pending(&context,&metadata);
        imported(&map,"other.rs","abcdefgh");
        context.sail_with_imports(&map,|_|8,|_,imports|{
            let owner=imports.unwrap();let record=&owner.records()[0];
            assert!(matches!(record.state,State::Unavailable(CaptureFailure::Delayed)));
            assert_eq!(record.descriptor_refusal,Some(capture::DescriptorRefusal::ForeignHashElsewhere));
        });
        assert_eq!(reads.load(Ordering::SeqCst),0);assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn initial_loaded_coordinate_change_refuses_before_late_classification() {
    session(|| {
        let (map,context,metadata,_)=fixture(None,0);let file=map.files()[0].clone();
        *context.imported_crates[&7].files[0].info.lock().unwrap()=ExternSourceInfo::Loaded{start_pos:file.start_pos,end_pos:BytePos(file.start_pos.0+8)};
        install_pending(&context,&metadata);
        *context.imported_crates[&7].files[0].info.lock().unwrap()=ExternSourceInfo::Loaded{start_pos:BytePos(file.start_pos.0+1),end_pos:BytePos(file.start_pos.0+9)};
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports.unwrap().records()[0].state,State::Unavailable(CaptureFailure::ChangedResolver))));
        assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn discarded_and_busy_seed_cannot_be_reset_at_handoff() {
    session(|| {
        let (map,context,metadata,_)=original_fixture(None);
        let held=context.imported_crates[&7].files[0].info.lock().unwrap();
        install_pending(&context,&metadata);drop(held);
        imported(&map,"late.rs","abcdefgh");
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports.unwrap().records()[0].state,State::Unavailable(CaptureFailure::Busy))));
        *context.sail_imports.lock().unwrap()=Some(Err(CaptureFailure::Budget));
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports,Err(CaptureFailure::Budget))));
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports,Err(CaptureFailure::Missing))));
        assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn cancelling_pending_owner_disposes_names_before_quota_release() {
    session(|| {
        let (map,context,metadata,_)=original_fixture(None);
        install_pending(&context,&metadata);
        let (sent,observed)=std::sync::mpsc::channel();let (continue_drop,receive)=std::sync::mpsc::channel();
        capture::drop_probe::install(sent,receive);
        let slot=context.sail_imports.lock().unwrap().take();
        let worker=std::thread::spawn(move||drop(slot));
        observed.recv().unwrap();
        assert_eq!(capture::retained_bytes(),8*1024*1024);
        assert!(matches!(capture::sail_capture_import_seeds(&context.imported_crates,&metadata),Err(CaptureFailure::Budget)));
        continue_drop.send(()).unwrap();worker.join().unwrap();
        assert_eq!(capture::retained_bytes(),0);assert!(map.files().is_empty());
    });
}
#[test]
fn immutable_owner_does_not_reclassify_on_later_namespace_growth() {
    session(|| {
        let (map,context,metadata,reads)=original_fixture(None);install_pending(&context,&metadata);
        let owner=context.sail_with_imports(&map,crate_id,|_,imports|imports.unwrap());
        assert!(owner.original_descriptor(packed(1002,1006)).is_ok());
        imported(&map,"later.rs","abcdefgh");
        assert!(matches!(owner.validate_binding_files(packed(1002,1006),&map.files()),Err(CaptureFailure::ChangedSource)));
        assert!(owner.original_descriptor(packed(1002,1006)).is_ok());
        assert!(owner.deferred(packed(1002,1006)).is_err());
        assert_eq!(reads.load(Ordering::SeqCst),0);
    });
}
#[test]
fn late_duplicate_foreign_refuses_reuse_without_new_metadata_allocation() {
    session(|| {
        let (map,context,metadata,_)=original_fixture(None);install_pending(&context,&metadata);
        imported(&map,"late.rs","abcdefgh");
        let owner=context.sail_with_imports(&map,crate_id,|_,imports|imports.unwrap());
        assert!(owner.deferred(packed(1002,1006)).is_ok());
        imported(&map,"duplicate.rs","abcdefgh");
        assert!(matches!(owner.validate_binding_files(packed(1002,1006),&map.files()),Err(CaptureFailure::AmbiguousContentOwner)));
    });
}
#[test]
fn namespace_over_cap_fails_once_and_releases_pending_names() {
    session(|| {
        let (map,context,metadata,_)=original_fixture(None);install_pending(&context,&metadata);
        for index in 0..4097 {map.new_source_file(FileName::Custom(format!("many{index}")),String::new());}
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports,Err(CaptureFailure::Budget))));
        context.sail_with_imports(&map,crate_id,|_,imports|assert!(matches!(imports,Err(CaptureFailure::Missing))));
        assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn pending_original_metadata_is_frozen_before_late_namespace_classification() {
    session(|| {
        let (map,context,mut metadata,_)=original_fixture(None);install_pending(&context,&metadata);
        let original=metadata.get_mut(&7).unwrap().values_mut().next().unwrap();
        original.filename=Some("/mutated-original.rs".into());original.start_pos=999;
        let owner=context.sail_with_imports(&map,crate_id,|_,imports|imports.unwrap());
        let (record,recipe)=owner.original_descriptor(packed(1002,1006)).unwrap();
        assert_eq!(record.original_start,1000);assert_eq!(recipe.loader_path,"/exact/deferred.rs");
    });
}
#[test]
fn current_stock_recipe_resolution_does_not_reclassify_or_forge_reuse_owner() {
    session(|| {
        for content in [Some("abcdefgh"),None] {
            let (map,context,metadata,reads)=original_fixture(content);install_pending(&context,&metadata);
            imported(&map,"late.rs","abcdefgh");
            let owner=context.sail_with_imports(&map,crate_id,|_,imports|imports.unwrap());
            let span=vir::messages::Span{id:1,data:packed(1002,1006).to_vec(),as_string:"same current key".into(),raw_span:Arc::new(())};
            let resolved=context.from_air_span(&span,Some(&map));
            assert_eq!(resolved.is_some(),content.is_some());
            assert_eq!(reads.load(Ordering::SeqCst),1);
            assert!(owner.validate_binding_files(packed(1002,1006),&map.files()).is_ok());
            assert!(owner.deferred(packed(1002,1006)).is_ok());
            assert_eq!(owner.current_range(packed(1002,1006)),Err(CaptureFailure::Delayed));
            assert!(((&*span.raw_span) as &(dyn std::any::Any+Send+Sync)).is::<()>());
        }
    });
}

#[test]
fn original_metadata_row_cap_precedes_per_range_search_and_admission() {
    session(|| {
        let (_,context,mut metadata,_)=original_fixture(None);
        let originals=metadata.get_mut(&7).unwrap();
        for index in 0..4097u32 {
            originals.insert(index.to_le_bytes().to_vec(),FileStartEndPos{filename:None,start_pos:index,end_pos:index});
        }
        assert!(matches!(capture::sail_capture_import_seeds(&context.imported_crates,&metadata),Err(CaptureFailure::Budget)));
        assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn empty_imported_outer_namespace_cap_precedes_admission() {
    session(|| {
        let mut imported_crates=HashMap::new();
        for index in 0..4097u64 {imported_crates.insert(index,CrateInfo{files:Vec::new()});}
        assert!(matches!(capture::sail_capture_import_seeds(&imported_crates,&HashMap::new()),Err(CaptureFailure::Budget)));
        assert_eq!(capture::retained_bytes(),0);
    });
}
#[test]
fn empty_original_outer_namespace_cap_precedes_admission() {
    session(|| {
        let mut originals=HashMap::new();
        for index in 0..4097u64 {originals.insert(index,HashMap::new());}
        assert!(matches!(capture::sail_capture_import_seeds(&HashMap::new(),&originals),Err(CaptureFailure::Budget)));
        assert_eq!(capture::retained_bytes(),0);
    });
}

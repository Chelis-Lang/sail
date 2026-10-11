// Compiled only with actual pinned CrateMetadata/FileStartEndPos derives and
// bincode_next dependencies. No VIR IR is decoded or preparation proof claimed.
#[test]
fn actual_sealed_vstd_original_metadata_remaining_file_recipe_is_eligible() {
    session(|| {
        let path = std::env::var("SAIL_TEST_VSTD_METADATA_SUFFIX").unwrap();
        // Python bounded and SHA-checked the exact source/export suffix before
        // handing it to the pinned serde decoder; this is a diagnostic input.
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() <= 8 * 1024 * 1024);
        let (metadata, consumed): (CrateMetadata, usize) =
            bincode_next::serde::decode_from_slice(&bytes, bincode_next::config::legacy()).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(metadata.crate_id, 2208441662476345389);
        assert!(metadata.original_files.len() <= 4096);
        let selected = [
            0x96, 0x6a, 0x15, 0xd4, 0xa7, 0x88, 0x5d, 0x0f, 0x33, 0x91, 0xdf, 0x29, 0xd6, 0xd3,
            0x1a, 0x1d,
        ];
        let original = &metadata.original_files[selected.as_slice()];
        assert_eq!((original.start_pos, original.end_pos), (406973, 411571));
        let filename = original.filename.as_ref().unwrap();
        assert!(filename.is_absolute());
        let spelling = filename.to_str().unwrap();
        assert!(spelling.len() <= 4096 && !spelling.contains('\0'));
        let reads = Arc::new(AtomicUsize::new(0));
        let map = SourceMap::with_inputs(SourceMapInputs {
            file_loader: Box::new(CountLoader {
                reads: reads.clone(),
                content: None,
            }),
            path_mapping: FilePathMapping::empty(),
            hash_kind: SourceFileHashAlgorithm::Sha256,
            checksum_hash_kind: None,
        });
        let originals = HashMap::from([(metadata.crate_id, metadata.original_files)]);
        // The harness injects the exact pinned remaining_crate_files loop;
        // original filenames/hash/ranges supply actual Delayed descriptors.
        let imported_crates = super::remaining_original_descriptors(originals.clone());
        let mut owner =
            sail_capture_loaded_imports(&imported_crates, &originals, &map, |_| 0).unwrap();
        owner.revalidate(&imported_crates, &map);
        let packed = [
            metadata.crate_id,
            (u64::from(410885u32) << 32) | u64::from(410885u32),
        ];
        let (record, recipe) = owner.original_descriptor(packed).unwrap();
        assert_eq!(
            recipe.loader_path,
            originals[&metadata.crate_id][selected.as_slice()]
                .filename
                .as_ref()
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert!(matches!(record.state, State::OriginalDescriptor(_)));
        assert_eq!(record.initial_foreign_hash_matches, Some(0));
        assert_eq!(record.pre_focus_foreign_hash_matches, Some(0));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(map.files().is_empty());
        println!(
            "SEALED_VSTD_METADATA_SELECTED_ORIGINAL_ELIGIBLE records={} no_io=true no_foreign_namespace=true",
            originals[&metadata.crate_id].len()
        );
    });
}

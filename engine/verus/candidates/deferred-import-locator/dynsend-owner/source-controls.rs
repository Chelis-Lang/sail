// These controls are appended to the ACTUAL source test module. They are not
// executed or stubbed by the reduced SourceMap API harness.
#[test]
fn actual_deferred_provenance_is_tagged_path_bound_and_coordinate_independent() {
    session(|| {
        fn fingerprint(prefix: usize, path: &str, raw: vir::messages::RawSpan) -> Option<String> {
            let (map, context, metadata, reads) = fixture(None, prefix);
            if let ExternSourceInfo::Delayed { filename, .. } =
                &mut *context.imported_crates[&7].files[0].info.lock().unwrap()
            {
                *filename = path.into();
            }
            let mut owner = capture(&context, &metadata, &map);
            owner.revalidate(&context.imported_crates, &map);
            let span = vir::messages::Span {
                raw_span: raw,
                id: 17,
                data: packed(1002, 1006).to_vec(),
                as_string: "exact current spelling".into(),
            };
            let result = crate::verifier::sail_test_import_provenance(&span, owner);
            assert_eq!(reads.load(Ordering::SeqCst), 0);
            result
        }
        let baseline = fingerprint(0, "/exact/deferred.rs", Arc::new(())).unwrap();
        assert_eq!(
            fingerprint(25, "/exact/deferred.rs", Arc::new(())).unwrap(),
            baseline
        );
        assert_ne!(
            fingerprint(25, "/different/deferred.rs", Arc::new(())).unwrap(),
            baseline
        );
        assert!(fingerprint(25, "relative.rs", Arc::new(())).is_none());
        assert!(fingerprint(25, "/exact/deferred.rs", Arc::new(9u64)).is_none());
    });
}

#[test]
fn actual_prepared_span_codec_restores_only_the_exact_current_unit_arc() {
    session(|| {
        let current = vir::messages::Span {
            raw_span: Arc::new(()),
            id: 17,
            data: packed(1002, 1006).to_vec(),
            as_string: "current exact binding".into(),
        };
        let encoded =
            bincode_next::serde::encode_to_vec(&current, bincode_next::config::legacy()).unwrap();
        let bindings = HashMap::from([(
            (current.id, current.data.clone(), current.as_string.clone()),
            current.clone(),
        )]);
        let (restored, consumed): (vir::messages::Span, usize) =
            vir::messages::sail_resolve_spans(&bindings, || {
                bincode_next::serde::decode_from_slice(&encoded, bincode_next::config::legacy())
                    .unwrap()
            });
        assert_eq!(consumed, encoded.len());
        assert!(Arc::ptr_eq(&restored.raw_span, &current.raw_span));
        assert!(((&*restored.raw_span) as &(dyn std::any::Any + Send + Sync)).is::<()>());
        for variant in 0..3 {
            let mut changed = current.clone();
            match variant {
                0 => changed.id += 1,
                1 => changed.data[1] += 1,
                _ => changed.as_string.push('x'),
            }
            let altered =
                bincode_next::serde::encode_to_vec(&changed, bincode_next::config::legacy())
                    .unwrap();
            let result = vir::messages::sail_resolve_spans(&bindings, || {
                bincode_next::serde::decode_from_slice::<vir::messages::Span, _>(
                    &altered,
                    bincode_next::config::legacy(),
                )
            });
            assert!(result.is_err());
        }
    });
}

#[test]
fn actual_reporter_replay_preserves_order_levels_primary_labels_and_current_resolution() {
    use air::messages::Diagnostics as _;
    use vir::messages::ToAny as _;
    #[derive(Default)]
    struct Emitted(Vec<String>);
    struct CaptureEmitter {
        map: Arc<SourceMap>,
        emitted: Arc<Mutex<Emitted>>,
    }
    impl rustc_errors::emitter::Emitter for CaptureEmitter {
        fn source_map(&self) -> Option<&SourceMap> {
            Some(&self.map)
        }
        fn emit_diagnostic(&mut self, diagnostic: rustc_errors::DiagInner) {
            let primary = diagnostic
                .span
                .primary_spans()
                .iter()
                .map(|span| self.map.span_to_diagnostic_string(*span))
                .collect::<Vec<_>>();
            let labels = diagnostic
                .span
                .span_labels()
                .into_iter()
                .map(|label| {
                    (
                        self.map.span_to_diagnostic_string(label.span),
                        format!("{:?}", label.label),
                    )
                })
                .collect::<Vec<_>>();
            self.emitted.lock().unwrap().0.push(format!(
                "{:?}|{:?}|{primary:?}|{labels:?}",
                diagnostic.level(),
                diagnostic.messages
            ));
        }
    }
    rustc_thread(|| {
        fn arm(content: Option<&str>, prepared: bool) -> (Vec<String>, usize) {
            let (map, context, metadata, reads) = fixture(content, 25);
            // Pinned rustc's Emitter requires DynSend, and SourceMap implements the
            // compiler's DynSend/DynSync markers rather than std Send/Sync. Match
            // ParseSess's Arc owner; this fixture stays on its single rustc thread.
            #[allow(clippy::arc_with_non_send_sync)]
            let map = Arc::new(map);
            let context = Arc::new(context);
            let owner = if prepared {
                let mut owner = capture(&context, &metadata, &map);
                owner.revalidate(&context.imported_crates, &map);
                Some(owner)
            } else {
                None
            };
            let emitted = Arc::new(Mutex::new(Emitted::default()));
            let diagnostics = rustc_errors::DiagCtxt::new(Box::new(CaptureEmitter {
                map: map.clone(),
                emitted: emitted.clone(),
            }));
            let reporter = crate::verifier::sail_test_stock_reporter(&context, &diagnostics, &map);
            let imported_span = |lo, hi| vir::messages::Span {
                raw_span: Arc::new(()),
                id: 17,
                data: packed(lo, hi).to_vec(),
                as_string: "current raw-unit binding".into(),
            };
            let messages = (0..4)
                .map(|index| {
                    let primary = imported_span(1002, 1006);
                    let label = imported_span(1001, 1002);
                    (*vir::messages::note(&primary, format!("ordered message {index}"))
                        .secondary_label(&label, "current label"))
                    .clone()
                })
                .collect::<Vec<_>>();
            if prepared {
                crate::verifier::sail_test_replay_reporter(&messages, &reporter);
            } else {
                reporter.report(&Arc::new(messages[0].clone()).to_any());
                reporter.report_as(
                    &Arc::new(messages[1].clone()).to_any(),
                    vir::messages::MessageLevel::Warning,
                );
                reporter.report_now(&Arc::new(messages[2].clone()).to_any());
                reporter.report_as_now(
                    &Arc::new(messages[3].clone()).to_any(),
                    vir::messages::MessageLevel::Warning,
                );
            }
            let group = messages
                .iter()
                .enumerate()
                .map(|(index, message)| {
                    let mut message = message.clone();
                    message.note = format!("group {index}");
                    (
                        Arc::new(message).to_any(),
                        vir::messages::MessageLevel::Note,
                    )
                })
                .collect();
            reporter.report_as_multi(group);
            if let Some(owner) = owner {
                assert!(owner.deferred(packed(1002, 1006)).is_ok());
            }
            let output = std::mem::take(&mut emitted.lock().unwrap().0);
            assert_eq!(output.len(), 8);
            (output, reads.load(Ordering::SeqCst))
        }
        for content in [Some("abcdefgh"), Some("wrong!!!"), None] {
            let fresh = arm(content, false);
            let restored = arm(content, true);
            assert_eq!(fresh, restored);
            assert_eq!(restored.1, 1);
        }
    });
}

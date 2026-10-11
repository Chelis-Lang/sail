#![feature(rustc_private)]
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_span;
use rustc_span::source_map::{FileLoader, FilePathMapping, SourceMap, SourceMapInputs};
use rustc_span::{BytePos, FileName, SourceFileHashAlgorithm, Span, SyntaxContext};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct Capture(Vec<(String, Vec<String>)>);
struct CaptureEmitter {
    map: Arc<SourceMap>,
    emitted: Arc<Mutex<Capture>>,
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
            .collect();
        self.emitted
            .lock()
            .unwrap()
            .0
            .push((format!("{:?}", diagnostic.level()), primary));
    }
}
struct CountLoader(Arc<std::sync::atomic::AtomicUsize>);
impl FileLoader for CountLoader {
    fn file_exists(&self, _: &std::path::Path) -> bool {
        true
    }
    fn read_file(&self, _: &std::path::Path) -> std::io::Result<String> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok("abcdef\n".into())
    }
    fn read_binary_file(&self, _: &std::path::Path) -> std::io::Result<Arc<[u8]>> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    fn current_directory(&self) -> std::io::Result<std::path::PathBuf> {
        Ok("/".into())
    }
}
#[test]
fn pinned_emitter_accepts_shared_dyn_send_map_and_preserves_custom_loader() {
    std::thread::Builder::new()
        .name("rustc".into())
        .spawn(|| {
            rustc_span::create_session_if_not_set_then(
                rustc_span::edition::Edition::Edition2021,
                |_| {
                    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let map = SourceMap::with_inputs(SourceMapInputs {
                        file_loader: Box::new(CountLoader(reads.clone())),
                        path_mapping: FilePathMapping::empty(),
                        hash_kind: SourceFileHashAlgorithm::Sha256,
                        checksum_hash_kind: None,
                    });
                    // Pinned rustc Emitter requires DynSend; SourceMap has DynSync/DynSend but not std Send/Sync.
                    // Match ParseSess's Arc owner; both emitter and caller remain on this rustc thread.
                    #[allow(clippy::arc_with_non_send_sync)]
                    let map = Arc::new(map);
                    rustc_data_structures::marker::assert_dyn_send_val(&map);
                    let file = map
                        .load_file(std::path::Path::new("/exact/source.rs"))
                        .unwrap();
                    let span = Span::new(
                        file.start_pos,
                        BytePos(file.start_pos.0 + 3),
                        SyntaxContext::root(),
                        None,
                    );
                    assert_eq!(
                        file.name,
                        FileName::Real(FilePathMapping::empty().to_real_filename(
                            &FilePathMapping::empty().to_real_filename(
                                &rustc_span::RealFileName::empty(),
                                std::path::Path::new("/")
                            ),
                            std::path::Path::new("/exact/source.rs")
                        ))
                    );
                    let emitted = Arc::new(Mutex::new(Capture::default()));
                    let diagnostics = rustc_errors::DiagCtxt::new(Box::new(CaptureEmitter {
                        map: map.clone(),
                        emitted: emitted.clone(),
                    }));
                    diagnostics
                        .handle()
                        .struct_span_warn(span, "owned map warning")
                        .emit();
                    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
                    let expected = map.span_to_diagnostic_string(span);
                    assert_eq!(
                        emitted.lock().unwrap().0,
                        vec![("Warning".into(), vec![expected])]
                    );
                    assert_eq!(Arc::strong_count(&map), 2);
                    drop(diagnostics);
                    assert_eq!(Arc::strong_count(&map), 1);
                },
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

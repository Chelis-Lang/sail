use super::*;
use rustc_span::source_map::{FileLoader, FilePathMapping, SourceMapInputs};
use rustc_span::SourceFileHashAlgorithm;
use std::sync::atomic::{AtomicUsize, Ordering};
struct CountLoader(Arc<AtomicUsize>);
impl FileLoader for CountLoader {
    fn file_exists(&self,_:&std::path::Path)->bool{false}
    fn read_file(&self,_:&std::path::Path)->std::io::Result<String>{self.0.fetch_add(1,Ordering::SeqCst);Err(std::io::ErrorKind::NotFound.into())}
    fn read_binary_file(&self,_:&std::path::Path)->std::io::Result<Arc<[u8]>>{self.0.fetch_add(1,Ordering::SeqCst);Err(std::io::ErrorKind::NotFound.into())}
    fn current_directory(&self)->std::io::Result<std::path::PathBuf>{Ok("/".into())}
}
#[test]
fn exact_sealed_ranges_have_no_stock_locator_and_perform_zero_loading(){
    std::thread::Builder::new().name("rustc".to_owned()).spawn(|| {
        rustc_span::create_session_if_not_set_then(rustc_span::edition::Edition::Edition2021,|_|{
            let bytes=std::fs::read(std::env::var("SAIL_TEST_METADATA_SUFFIX").unwrap()).unwrap();
            assert!(bytes.len()<=8*1024*1024);
            let (metadata,consumed):(CrateMetadata,usize)=bincode_next::serde::decode_from_slice(&bytes,bincode_next::config::legacy()).unwrap();
            assert_eq!(consumed,bytes.len());assert_eq!(metadata.crate_id,2208441662476345389);assert_eq!(metadata.original_files.len(),145);
            let originals=HashMap::from([(metadata.crate_id,metadata.original_files)]);
            let context=SpanContextX{imported_crates:remaining_original_descriptors(originals.clone())};
            let reads=Arc::new(AtomicUsize::new(0));
            let map=SourceMap::with_inputs(SourceMapInputs{file_loader:Box::new(CountLoader(reads.clone())),path_mapping:FilePathMapping::empty(),hash_kind:SourceFileHashAlgorithm::Sha256,checksum_hash_kind:None});
            for (id,word,spelling) in [(262108,21834312362857100,"core::ops::function"),(263275,16250141912185775,"core::alloc"),(262093,16610953524872159,"alloc::boxed")] {
                let lo=(word>>32) as u32;let hi=word as u32;assert!(lo<=hi);
                assert!(!originals[&metadata.crate_id].values().any(|f|f.start_pos<=lo&&lo<=f.end_pos));
                assert!(context.pos_to_extern_source_file(metadata.crate_id,BytePos(lo)).is_none());
                let data=vec![metadata.crate_id,word];
                assert!(context.unpack_span(&data,None).is_none());assert!(context.unpack_span(&data,Some(&map)).is_none());
                let span=vir::messages::Span{raw_span:Arc::new(()),id,data,as_string:spelling.into()};
                assert!(context.from_air_span(&span,Some(&map)).is_none());
                assert_eq!(reads.load(Ordering::SeqCst),0);assert!(map.files().is_empty());
                println!("UNCOVERED id={id} lo={lo} hi={hi} stock_none=true loader_calls=0");
            }
            // An unknown crate is also None, but must remain a distinct refusal;
            // a None result alone is insufficient to establish known absence.
            assert!(context.unpack_span(&vec![u64::MAX,21834312362857100],Some(&map)).is_none());
            assert_eq!(reads.load(Ordering::SeqCst),0);
            // Supplying a covering imported row is NOT the absence case, even
            // when its stock resolver has no usable source recipe and returns None.
            let mut covering=remaining_original_descriptors(originals.clone());
            let rows=&mut covering.get_mut(&metadata.crate_id).unwrap().files;
            rows.push(ExternSourceFile{original_start_pos:BytePos(5083697),original_end_pos:BytePos(5083788),info:Arc::new(Mutex::new(ExternSourceInfo::None))});
            rows.sort_by_key(|f|f.original_start_pos);
            let covered=SpanContextX{imported_crates:covering};
            assert!(covered.pos_to_extern_source_file(metadata.crate_id,BytePos(5083697)).is_some());
            assert!(covered.unpack_span(&vec![metadata.crate_id,21834312362857100],Some(&map)).is_none());
            assert_eq!(reads.load(Ordering::SeqCst),0);
            // Positive sensitivity: an actual retained original descriptor
            // reaches the same stock loader. Missing input then yields None.
            let hash=vec![0x96,0x6a,0x15,0xd4,0xa7,0x88,0x5d,0x0f,0x33,0x91,0xdf,0x29,0xd6,0xd3,0x1a,0x1d];
            let original=&originals[&metadata.crate_id][&hash];assert_eq!((original.start_pos,original.end_pos),(406973,411571));assert!(original.filename.is_some());
            assert!(context.pos_to_extern_source_file(metadata.crate_id,BytePos(410885)).is_some());
            assert!(context.unpack_span(&vec![metadata.crate_id,(410885u64<<32)|410885],Some(&map)).is_none());
            assert_eq!(reads.load(Ordering::SeqCst),1);assert!(map.files().is_empty());
            println!("COVERED_DESCRIPTOR stock_missing_none=true loader_calls=1; covered_None_and_unknown_crate_are_distinct");
        });
    }).unwrap().join().unwrap();
}

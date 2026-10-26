use proc_macro::TokenStream;

/// Read an ordinary file without declaring it to rustc's tracked-path API.
#[proc_macro]
pub fn external_literal(_input: TokenStream) -> TokenStream {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../external-value.txt");
    let value: u64 = std::fs::read_to_string(&path)
        .expect("read the fixture input")
        .trim()
        .parse()
        .expect("parse the fixture integer");
    #[cfg(feature = "declared-input")]
    let emitted = format!("{{ let _ = include_str!({path:?}); {value}u64 }}");
    #[cfg(not(feature = "declared-input"))]
    let emitted = format!("{value}u64");
    emitted.parse().expect("emit an integer literal")
}

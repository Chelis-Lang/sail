//! A user procedural macro observes physical process inputs, not a restored Vec.
extern crate proc_macro;
use proc_macro::TokenStream;

const MARKER: &str = "--time";

/// Emits a different proof only when injected operational argv is observable.
#[proc_macro]
pub fn argv_witness(input: TokenStream) -> TokenStream {
    let found = match input.to_string().as_str() {
        "args" => std::env::args().any(|argument| argument == MARKER),
        "args_os" => std::env::args_os().any(|argument| argument == MARKER),
        "proc_cmdline" => std::fs::read("/proc/self/cmdline").is_ok_and(|bytes| {
            bytes
                .split(|byte| *byte == 0)
                .any(|argument| argument == MARKER.as_bytes())
        }),
        _ => false,
    };
    // Both variants are syntactically valid, fully observed Verus obligations.
    // The candidate must never change this macro's input relative to plain Verus.
    format!("verus! {{ pub proof fn witness() {{ assert({found}); }} }}")
        .parse()
        .unwrap()
}

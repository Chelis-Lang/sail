use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or("manifest")?);
    let external = manifest
        .parent()
        .ok_or("fixture parent")?
        .join("external-value.txt");
    if std::env::var_os("CARGO_FEATURE_DECLARED_INPUT").is_some() {
        println!("cargo:rerun-if-changed={}", external.display());
    }
    let value = std::fs::read_to_string(external)?.trim().parse::<u8>()?;
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("output directory")?);
    std::fs::write(
        out.join("generated.rs"),
        format!(
            "#[cfg(feature=\"verification\")]\nvstd::prelude::verus! {{ pub const EXTERNAL_VALUE: u8 = {value}; }}\n\
             #[cfg(not(feature=\"verification\"))]\npub const EXTERNAL_VALUE: u8 = {value};\n"
        ),
    )?;
    Ok(())
}

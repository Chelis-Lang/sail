//! Native fault fixture for the parent protocol; never a verifier backend.
#![forbid(unsafe_code)]
use std::io::{Read as _, Write as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--hold") {
        std::thread::sleep(std::time::Duration::from_secs(30));
        return Ok(());
    }
    let mut request = String::new();
    std::io::stdin().read_to_string(&mut request)?;
    if request.contains("SAIL_CONTROL_PIPES") {
        let child = std::process::Command::new(std::env::current_exe()?)
            .arg("--hold")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
        // The test parent owns and kills this entire process group.
        drop(child);
    } else if request.contains("SAIL_CONTROL_OVERSIZE") {
        std::io::stdout().write_all(&vec![b'x'; 16 * 1024 * 1024 + 1])?;
    } else if request.contains("SAIL_CONTROL_STDERR") {
        std::io::stderr().write_all(b"unexpected worker stderr")?;
        std::io::stdout().write_all(b"{}")?;
    } else if request.contains("SAIL_CONTROL_TRAILING") {
        std::io::stdout().write_all(b"{}{}")?;
    } else {
        std::io::stdout().write_all(b"not JSON")?;
    }
    Ok(())
}

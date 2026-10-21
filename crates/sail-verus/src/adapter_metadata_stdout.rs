use crate::{Error, Result};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Return the emission value for an unambiguous metadata-to-stdout command.
pub(super) fn metadata_output(args: &[OsString]) -> Option<usize> {
    let mut emission = None;
    let mut stdout = false;
    let mut index = 1;
    while let Some(argument) = args.get(index) {
        let bytes = argument.as_encoded_bytes();
        if bytes == b"--"
            || bytes.starts_with(b"@")
            || (bytes.starts_with(b"-")
                && bytes[1..]
                    .iter()
                    .all(|byte| matches!(byte, b'h' | b'v' | b'V'))
                && bytes.iter().any(|byte| matches!(byte, b'h' | b'V')))
            || bytes == b"--help"
            || bytes.starts_with(b"--help=")
            || bytes == b"--version"
            || bytes.starts_with(b"--version=")
            || bytes == b"--print"
            || bytes.starts_with(b"--print=")
            || bytes == b"--explain"
            || bytes.starts_with(b"--explain=")
            || bytes == b"--out-dir"
            || bytes.starts_with(b"--out-dir=")
        {
            return None;
        }
        if bytes == b"--emit" {
            index += 1;
            if args.get(index)?.as_encoded_bytes() != b"metadata"
                || emission.replace(index).is_some()
            {
                return None;
            }
        } else if bytes.starts_with(b"--emit=") {
            if bytes != b"--emit=metadata" || emission.replace(index).is_some() {
                return None;
            }
        } else if bytes == b"-o" {
            index += 1;
            if stdout || args.get(index)?.as_encoded_bytes() != b"-" {
                return None;
            }
            stdout = true;
        } else if bytes.starts_with(b"-o") {
            // Attached output options can hide a second output selection.
            return None;
        }
        index += 1;
    }
    if stdout { emission } else { None }
}

/// Own the temporary metadata destination until the compiler and replay finish.
pub(super) struct MetadataStdout {
    directory: PathBuf,
    metadata: PathBuf,
}

impl MetadataStdout {
    /// Replace only the stdout output path with an owned filename.
    pub(super) fn prepare(args: &mut [OsString], index: usize, run: &Path) -> Result<Self> {
        if metadata_output(args) != Some(index) {
            return Err(Error::new(
                "SAIL3111",
                "invalid metadata stdout emission selection",
            ));
        }
        let output = args
            .windows(2)
            .position(|pair| pair[0] == "-o" && pair[1] == "-")
            .ok_or_else(|| Error::new("SAIL3111", "metadata stdout output argument absent"))?;
        let directory = run.join(format!("metadata-stdout-{}", std::process::id()));
        fs::create_dir(&directory).map_err(|error| {
            Error::new(
                "SAIL3111",
                format!("cannot create metadata stdout directory: {error}"),
            )
        })?;
        let owned = Self {
            metadata: directory.join("output.rmeta"),
            directory,
        };
        // Keep filenames outside --emit because rustc splits that value at commas.
        owned.metadata.as_os_str().clone_into(&mut args[output + 1]);
        Ok(owned)
    }

    /// Copy metadata bytes to the writer, if the compiler created the file.
    pub(super) fn replay(&self, output: &mut impl Write) -> Result<()> {
        let mut metadata = match File::open(&self.metadata) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(Error::new(
                    "SAIL3111",
                    format!("cannot open metadata stdout file: {error}"),
                ));
            }
        };
        io::copy(&mut metadata, output).map_err(|error| {
            Error::new(
                "SAIL3111",
                format!("cannot replay metadata stdout: {error}"),
            )
        })?;
        Ok(())
    }
}

impl Drop for MetadataStdout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[cfg(test)]
#[path = "adapter_metadata_stdout_tests.rs"]
mod tests;

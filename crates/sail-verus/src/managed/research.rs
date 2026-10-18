//! Mutually exclusive explicit compiler-reuse research requests.

use crate::{Error, Result};

/// The one research authority allowed to configure this compiler family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// No explicit research request; ordinary retained eligibility is separate.
    Off,
    /// Cross-project vstd compiler-boundary experiment (ADR 0048).
    VstdBoundary,
    /// Same-workspace content restoration experiment (ADR 0050).
    RetainedContent,
}

/// Refuse simultaneous experiments before either can configure a child.
pub(crate) fn selected() -> Result<Mode> {
    let enabled = |name| std::env::var_os(name).is_some_and(|value| value == "1");
    match (
        enabled("SAIL_VSTD_BOUNDARY_RESEARCH"),
        enabled("SAIL_RETAINED_CONTENT_RESEARCH"),
    ) {
        (false, false) => Ok(Mode::Off),
        (true, false) => Ok(Mode::VstdBoundary),
        (false, true) => Ok(Mode::RetainedContent),
        (true, true) => Err(Error::new(
            "SAIL3135",
            "vstd-boundary and retained-content research cannot be enabled together; select one experiment",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_selection_reads_exact_values() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let Ok(expected) = std::env::var("SAIL_RESEARCH_SELECTION_EXPECTED") else {
            return Ok(());
        };
        match expected.as_str() {
            "off" => assert_eq!(selected()?, Mode::Off),
            "vstd" => assert_eq!(selected()?, Mode::VstdBoundary),
            "retained" => assert_eq!(selected()?, Mode::RetainedContent),
            "conflict" => assert_eq!(selected().err().ok_or("expected refusal")?.code, "SAIL3135"),
            _ => return Err("unknown owned test selection".into()),
        }
        Ok(())
    }

    #[test]
    fn switches_are_isolated_and_both_entrypoints_refuse_conflicts()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let executable = std::env::current_exe()?;
        for (vstd, retained, expected) in [
            ("", "", "off"),
            ("0", "0", "off"),
            ("true", "true", "off"),
            ("1", "", "vstd"),
            ("", "1", "retained"),
            ("1", "1", "conflict"),
        ] {
            let child = std::process::Command::new(&executable)
                .args([
                    "--exact",
                    "managed::research::tests::process_selection_reads_exact_values",
                ])
                .env("SAIL_VSTD_BOUNDARY_RESEARCH", vstd)
                .env("SAIL_RETAINED_CONTENT_RESEARCH", retained)
                .env("SAIL_RESEARCH_SELECTION_EXPECTED", expected)
                .output()?;
            assert!(
                child.status.success(),
                "{}",
                String::from_utf8_lossy(&child.stderr)
            );
        }
        for test in [
            "adapter::tests::conflicting_research_refuses_before_adapter_setup",
            "managed::runner::tests::conflicting_research_refuses_before_runner_setup",
        ] {
            let child = std::process::Command::new(&executable)
                .args(["--exact", test])
                .env("SAIL_VSTD_BOUNDARY_RESEARCH", "1")
                .env("SAIL_RETAINED_CONTENT_RESEARCH", "1")
                .env("SAIL_RESEARCH_SELECTION_EXPECTED", "conflict")
                .env_remove("SAIL_MANAGED_WRAPPER")
                .output()?;
            assert!(
                child.status.success(),
                "{}",
                String::from_utf8_lossy(&child.stderr)
            );
        }
        Ok(())
    }
}

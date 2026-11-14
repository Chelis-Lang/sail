//! Explanations for optional retention admission; never proof authority.

use super::{selection, skipped_note};

/// The actual admission stage that declined. Values are returned by the
/// failing check rather than inferred from later filesystem observations.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Decline {
    Profile(String),
    Metadata(&'static str),
    TargetKind(String),
    Runtime(Option<String>),
    Storage(&'static str),
    Digest(&'static str),
    CallerEnvironment,
}

impl Decline {
    pub(super) fn note(self) -> String {
        match self {
            Self::Profile(detail) => skipped_note(&format!(
                "the dependency profile is not qualified: {detail}"
            )),
            Self::Metadata(field) => skipped_note(&format!(
                "retention metadata is missing or invalid at {field}"
            )),
            Self::TargetKind(kind) => skipped_note(&format!(
                "retention requires a library target, but the selected target kind is {kind}"
            )),
            Self::Runtime(Some(manifest)) => selection::unqualified_note(&manifest),
            Self::Runtime(None) => skipped_note(
                "the launcher and solver could not establish an admitted retention runtime",
            ),
            Self::Storage(stage) => skipped_note(&format!(
                "retention storage could not be prepared at {stage}"
            )),
            Self::Digest(tool) => skipped_note(&format!(
                "the current {tool} bytes could not be hashed for retention"
            )),
            Self::CallerEnvironment => {
                skipped_note("the compiler caller environment could not be captured for this run")
            }
        }
    }
}

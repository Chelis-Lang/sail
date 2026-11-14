//! Exact optional archive-publication explanations.
//!
//! These operational notices carry no verification or proof-reuse authority.
//! The benchmark adapter recognizes this same vocabulary while retaining raw stderr.

use std::collections::BTreeMap;

/// Fixed stages, carrying no paths, journal or reuse authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Decline {
    /// The caller environment did not belong to this run.
    CallerEnvironment,
    /// The source snapshot was incomplete or no longer admitted.
    SourceSnapshot,
    /// The live graph inventory could not be read.
    LiveGraphs,
    /// The candidate receipt inventory could not be read.
    Candidates,
    /// The candidate archive entry could not be sealed.
    Entry,
    /// The archive entry or proof receipt did not pass admission.
    Identity,
    /// Recorded compiler-read bytes were unavailable or mismatched.
    CompilerReads,
    /// The compiler-read header or output-stream bounds were not admitted.
    Header,
    /// The archive encoding was unavailable or exceeded its bound.
    Encoding,
    /// Archive byte or entry accounting overflowed.
    Capacity,
    /// The managed storage budget did not admit the entry.
    Budget,
    /// The archive blob directory could not be prepared.
    BlobDirectory,
    /// An archive blob could not be copied and published.
    BlobCopy,
    /// The archive target directory could not be prepared.
    TargetDirectory,
    /// The archive entry could not be written.
    EntryWrite,
    /// Archive revision accounting or eviction could not complete.
    Eviction,
}

impl Decline {
    const ALL: [Self; 16] = [
        Self::CallerEnvironment,
        Self::SourceSnapshot,
        Self::LiveGraphs,
        Self::Candidates,
        Self::Entry,
        Self::Identity,
        Self::CompilerReads,
        Self::Header,
        Self::Encoding,
        Self::Capacity,
        Self::Budget,
        Self::BlobDirectory,
        Self::BlobCopy,
        Self::TargetDirectory,
        Self::EntryWrite,
        Self::Eviction,
    ];

    /// Recognizes one exact CLI explanation with a canonical positive u64 count.
    ///
    /// Unknown reasons, altered wording and malformed counts remain diagnostics.
    /// This does not establish any verification or receipt authority.
    #[must_use]
    pub fn from_note(text: &str) -> Option<Self> {
        if text.contains(['\r', '\n']) {
            return None;
        }
        let body = text
            .strip_prefix("note[SAIL3146]: optional retained-content publication declined at ")?
            .strip_suffix("; verification is unaffected")?;
        let (count, detail) = body.split_once(" operation(s) because ")?;
        let value: u64 = count.parse().ok()?;
        if value == 0 || value.to_string() != count {
            return None;
        }
        Self::ALL.into_iter().find(|stage| stage.detail() == detail)
    }

    fn detail(self) -> &'static str {
        match self {
            Self::CallerEnvironment => "the caller environment did not belong to this run",
            Self::SourceSnapshot => "the source snapshot was incomplete or no longer admitted",
            Self::LiveGraphs => "the live graph inventory could not be read",
            Self::Candidates => "the candidate receipt inventory could not be read",
            Self::Entry => "the candidate archive entry could not be sealed",
            Self::Identity => "the archive entry or proof receipt did not pass admission",
            Self::CompilerReads => {
                "the recorded compiler-read bytes were unavailable or mismatched"
            }
            Self::Header => "the compiler-read header or output-stream bounds were not admitted",
            Self::Encoding => "the archive entry encoding was unavailable or exceeded its bound",
            Self::Capacity => "the archive byte or entry accounting overflowed",
            Self::Budget => "the managed storage budget did not admit the archive entry",
            Self::BlobDirectory => "the archive blob directory could not be prepared",
            Self::BlobCopy => "an archive blob could not be copied and published",
            Self::TargetDirectory => "the archive target directory could not be prepared",
            Self::EntryWrite => "the archive entry could not be written",
            Self::Eviction => "the archive revision accounting or eviction could not complete",
        }
    }
}

/// At most one counter for each fixed stage, independent of candidate count.
#[derive(Default)]
pub(crate) struct Summary(BTreeMap<Decline, u64>);

impl Summary {
    pub(crate) fn record(&mut self, reason: Decline) {
        let count = self.0.entry(reason).or_default();
        *count = count.saturating_add(1);
    }

    pub(crate) fn notes(self) -> Vec<String> {
        self.0.into_iter().map(|(reason, count)| format!(
            "{}: optional retained-content publication declined at {count} operation(s) because {}; verification is unaffected",
            super::retained::SKIPPED, reason.detail()
        )).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_produced_stage_has_one_exact_recognized_notice() {
        for stage in Decline::ALL {
            let mut summary = Summary::default();
            summary.record(stage);
            let notes = summary.notes();
            assert_eq!(notes.len(), 1);
            for note in notes {
                assert_eq!(Decline::from_note(&format!("note[{note}]")), None);
                let human = note.replacen("SAIL3146:", "note[SAIL3146]:", 1);
                assert_eq!(Decline::from_note(&human), Some(stage));
                for count in ["0", "01", "+1", "-1", " 1", "1 ", "18446744073709551616"] {
                    assert_eq!(
                        Decline::from_note(
                            &human.replace("at 1 operation", &format!("at {count} operation"))
                        ),
                        None
                    );
                }
                for bad in [
                    format!("{human}\n"),
                    format!(" {human}"),
                    human.replace("unaffected", "changed"),
                    human.replace(stage.detail(), "an unknown stage"),
                    human.replace("SAIL3146", "SAIL3138"),
                ] {
                    assert_eq!(Decline::from_note(&bad), None);
                }
                assert_eq!(
                    Decline::from_note(
                        &human.replace("at 1 operation", "at 18446744073709551615 operation")
                    ),
                    Some(stage)
                );
            }
        }
    }

    #[test]
    fn repeated_failures_do_not_grow_an_operation_journal() {
        let mut summary = Summary::default();
        for _ in 0..50_000 {
            summary.record(Decline::CompilerReads);
            summary.record(Decline::BlobCopy);
        }
        let notes = summary.notes();
        assert_eq!(notes.len(), 2);
        assert!(notes.iter().all(|note| note.contains("50000 operation(s)")));
        assert_eq!(
            Summary::default().notes(),
            Vec::<String>::new(),
            "successful publication is silent"
        );
    }
}

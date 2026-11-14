//! Bounded explanations from actual optional archive-publication failures.

use std::collections::BTreeMap;

/// Fixed stages, carrying no paths, journal or reuse authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Decline {
    CallerEnvironment,
    SourceSnapshot,
    LiveGraphs,
    Candidates,
    Entry,
    Identity,
    CompilerReads,
    Header,
    Encoding,
    Capacity,
    Budget,
    BlobDirectory,
    BlobCopy,
    TargetDirectory,
    EntryWrite,
    Eviction,
}

impl Decline {
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
pub(super) struct Summary(BTreeMap<Decline, u64>);

impl Summary {
    pub(super) fn record(&mut self, reason: Decline) {
        let count = self.0.entry(reason).or_default();
        *count = count.saturating_add(1);
    }

    pub(super) fn notes(self) -> Vec<String> {
        self.0.into_iter().map(|(reason, count)| format!(
            "{}: optional retained-content publication declined at {count} operation(s) because {}; verification is unaffected",
            super::SKIPPED, reason.detail()
        )).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

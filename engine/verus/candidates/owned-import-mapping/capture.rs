//! Inert proposal: compiler-thread snapshot of the stock loaded import resolver.
//! No caller, provenance comparison, prepared codec or source loading is added.

use super::{CrateInfo, ExternSourceInfo, FileStartEndPos};
use rustc_span::def_id::CrateNum;
use rustc_span::source_map::SourceMap;
use rustc_span::{ExternalSource, SourceFileHash, StableSourceFileId};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

const RECORD_LIMIT: usize = 4096;
const NAME_LIMIT: usize = 4096;
const BUDGET: usize = 8 * 1024 * 1024;
static OWNED_BYTES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureFailure {
    Allocation,
    Budget,
    AmbiguousOriginalRange,
    AmbiguousCurrentRange,
    MissingCurrentFile,
    MissingOriginalHash,
    AmbiguousOriginalHash,
    HashMismatch,
    CrateMismatch,
    NotForeign,
    AmbiguousContentOwner,
    InvalidRange,
    Delayed,
    Missing,
    Busy,
    UnknownCrate,
}

pub(crate) struct LoadedFile {
    pub(crate) index: usize,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) normalized_length: u32,
    pub(crate) unnormalized_length: u32,
    pub(crate) name: String,
    pub(crate) stable_id: StableSourceFileId,
    pub(crate) hash: SourceFileHash,
}

pub(crate) enum State {
    Loaded(LoadedFile),
    Unavailable(CaptureFailure),
}

pub(crate) struct Record {
    pub(crate) crate_id: u64,
    pub(crate) original_start: u32,
    pub(crate) original_end: u32,
    pub(crate) state: State,
}

/// Contains owned primitives/strings only. It retains no compiler context,
/// SourceMap, source-file Arc, resolver mutex or old raw SpanData.
pub(crate) struct OwnedImportMappings {
    records: Vec<Record>,
    charged: bool,
}

impl Drop for OwnedImportMappings {
    fn drop(&mut self) {
        // Physical disposal precedes admission release, including every name.
        drop(std::mem::take(&mut self.records));
        #[cfg(test)]
        drop_probe::after_dispose(self.records.len(), self.records.capacity());
        if self.charged {
            OWNED_BYTES.fetch_sub(BUDGET, Ordering::AcqRel);
        }
    }
}

struct NameWriter {
    text: String,
}

impl std::fmt::Write for NameWriter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let size = self
            .text
            .len()
            .checked_add(value.len())
            .ok_or(std::fmt::Error)?;
        if size > NAME_LIMIT {
            return Err(std::fmt::Error);
        }
        if size > self.text.capacity() {
            self.text
                .try_reserve_exact(size - self.text.len())
                .map_err(|_| std::fmt::Error)?;
        }
        if self.text.capacity() > NAME_LIMIT {
            return Err(std::fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

/// Future call site: once in SpanContextX::new after imported_crates is built,
/// while original_crate_files is still borrowed on the compiler callback.
/// This proposal never calls resolve(), load_file(), span(), or SpanData Debug.
pub(super) fn sail_capture_loaded_imports(
    imported_crates: &HashMap<u64, CrateInfo>,
    original_crate_files: &HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
    source_map: &SourceMap,
    stable_crate_of: impl Fn(CrateNum) -> u64,
) -> Result<OwnedImportMappings, CaptureFailure> {
    let count = imported_crates
        .values()
        .try_fold(0usize, |count, info| {
            count
                .checked_add(info.files.len())
                .filter(|value| *value <= RECORD_LIMIT)
        })
        .ok_or(CaptureFailure::Budget)?;
    let files = source_map.files();
    if files.len() > RECORD_LIMIT {
        return Err(CaptureFailure::Budget);
    }
    OWNED_BYTES
        .compare_exchange(0, BUDGET, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| CaptureFailure::Budget)?;
    let mut owned = OwnedImportMappings {
        records: Vec::new(),
        charged: true,
    };
    // Fixed capacity: subsequent pushes cannot allocate another table.
    owned
        .records
        .try_reserve_exact(count)
        .map_err(|_| CaptureFailure::Allocation)?;
    let metadata = owned
        .records
        .capacity()
        .checked_mul(std::mem::size_of::<Record>())
        .and_then(|value| value.checked_add(1024))
        .ok_or(CaptureFailure::Budget)?;
    if owned.records.capacity() > RECORD_LIMIT || metadata > BUDGET {
        return Err(CaptureFailure::Budget);
    }
    let mut used = metadata;
    for (crate_id, info) in imported_crates {
        for file in &info.files {
            let original_start = file.original_start_pos.0;
            let original_end = file.original_end_pos.0;
            let original_length = original_end
                .checked_sub(original_start)
                .ok_or(CaptureFailure::InvalidRange)?;
            // Copy primitive ranges only; do not clone a Delayed path/hash.
            let selected = match file.info.try_lock() {
                Ok(state) => match &*state {
                    ExternSourceInfo::Loaded { start_pos, end_pos } => Ok((start_pos.0, end_pos.0)),
                    ExternSourceInfo::Delayed { .. } => Err(CaptureFailure::Delayed),
                    ExternSourceInfo::None => Err(CaptureFailure::Missing),
                },
                Err(_) => Err(CaptureFailure::Busy),
            };
            let state = match selected {
                Err(reason) => State::Unavailable(reason),
                Ok((start, end)) => {
                    if end.checked_sub(start) != Some(original_length) {
                        return Err(CaptureFailure::InvalidRange);
                    }
                    let mut matching = files.iter().enumerate().filter(|(_, current)| {
                        current.start_pos.0 == start
                            && start.checked_add(current.normalized_source_len.0) == Some(end)
                    });
                    let (index, current) =
                        matching.next().ok_or(CaptureFailure::MissingCurrentFile)?;
                    if matching.next().is_some() {
                        return Err(CaptureFailure::AmbiguousCurrentRange);
                    }
                    let originals = original_crate_files
                        .get(crate_id)
                        .ok_or(CaptureFailure::MissingOriginalHash)?;
                    let mut original_matches = originals.iter().filter(|(_, value)| {
                        value.start_pos == original_start && value.end_pos == original_end
                    });
                    let (expected_hash, _) = original_matches
                        .next()
                        .ok_or(CaptureFailure::MissingOriginalHash)?;
                    if original_matches.next().is_some() {
                        return Err(CaptureFailure::AmbiguousOriginalHash);
                    }
                    if expected_hash.as_slice() != current.src_hash.hash_bytes() {
                        return Err(CaptureFailure::HashMismatch);
                    }
                    if !matches!(
                        *current.external_src.borrow(),
                        ExternalSource::Foreign { .. }
                    ) {
                        return Err(CaptureFailure::NotForeign);
                    }
                    if stable_crate_of(current.cnum) != *crate_id {
                        return Err(CaptureFailure::CrateMismatch);
                    }
                    // A hash-keyed original table cannot disambiguate equal-content
                    // files in the same imported crate. Do not pick a filename.
                    if files
                        .iter()
                        .filter(|candidate| {
                            matches!(
                                *candidate.external_src.borrow(),
                                ExternalSource::Foreign { .. }
                            ) && stable_crate_of(candidate.cnum) == *crate_id
                                && candidate.src_hash.hash_bytes() == expected_hash.as_slice()
                        })
                        .count()
                        != 1
                    {
                        return Err(CaptureFailure::AmbiguousContentOwner);
                    }
                    // Full headroom includes old/new String allocation and
                    // formatter temporaries. Never clone an unbounded name.
                    if used
                        .checked_add(2 * NAME_LIMIT + 1024)
                        .filter(|size| *size <= BUDGET)
                        .is_none()
                    {
                        return Err(CaptureFailure::Budget);
                    }
                    let mut name = NameWriter {
                        text: String::new(),
                    };
                    write!(&mut name, "{:?}", current.name).map_err(|_| CaptureFailure::Budget)?;
                    used = used
                        .checked_add(name.text.capacity())
                        .ok_or(CaptureFailure::Budget)?;
                    State::Loaded(LoadedFile {
                        index,
                        start,
                        end,
                        normalized_length: current.normalized_source_len.0,
                        unnormalized_length: current.unnormalized_source_len,
                        name: name.text,
                        stable_id: current.stable_id,
                        hash: current.src_hash,
                    })
                }
            };
            owned.records.push(Record {
                crate_id: *crate_id,
                original_start,
                original_end,
                state,
            });
        }
    }
    // This is a private lookup inventory, not a reordered stock graph or
    // bucket. Inclusive range ambiguity is refused instead of selecting one.
    owned
        .records
        .sort_unstable_by_key(|record| (record.crate_id, record.original_start));
    if owned.records.windows(2).any(|pair| {
        pair[0].crate_id == pair[1].crate_id && pair[1].original_start <= pair[0].original_end
    }) {
        return Err(CaptureFailure::AmbiguousOriginalRange);
    }
    Ok(owned)
}

impl OwnedImportMappings {
    pub(crate) fn records(&self) -> &[Record] {
        &self.records
    }

    /// Owned range lookup only; not a prepared-span equivalence or raw binding.
    /// Imported stock rendering reconstructs root hygiene and parent=None. A
    /// future caller must first distinguish raw unit imports from local/unknown
    /// raw owners and preserve the exact current SailSpanKey binding.
    pub(crate) fn current_range(&self, packed: [u64; 2]) -> Result<(u32, u32), CaptureFailure> {
        let crate_id = packed[0];
        let lo = (packed[1] >> 32) as u32;
        let hi = packed[1] as u32;
        if hi < lo {
            return Err(CaptureFailure::InvalidRange);
        }
        let mut found = self.records.iter().filter(|record| {
            record.crate_id == crate_id && record.original_start <= lo && lo <= record.original_end
        });
        let record = found.next().ok_or(CaptureFailure::UnknownCrate)?;
        if found.next().is_some() {
            return Err(CaptureFailure::AmbiguousOriginalRange);
        }
        if hi > record.original_end {
            return Err(CaptureFailure::InvalidRange);
        }
        let current = match &record.state {
            State::Loaded(current) => current,
            State::Unavailable(reason) => return Err(*reason),
        };
        let new_lo = lo
            .checked_sub(record.original_start)
            .and_then(|value| value.checked_add(current.start))
            .ok_or(CaptureFailure::InvalidRange)?;
        let new_hi = hi
            .checked_sub(record.original_start)
            .and_then(|value| value.checked_add(current.start))
            .ok_or(CaptureFailure::InvalidRange)?;
        if new_lo > new_hi || new_hi > current.end {
            return Err(CaptureFailure::InvalidRange);
        }
        Ok((new_lo, new_hi))
    }
}

#[cfg(test)]
pub(super) mod drop_probe {
    use std::sync::{Mutex, mpsc};
    static HOOK: Mutex<Option<(mpsc::Sender<(usize, usize)>, mpsc::Receiver<()>)>> =
        Mutex::new(None);
    pub(crate) fn install(sent: mpsc::Sender<(usize, usize)>, resume: mpsc::Receiver<()>) {
        *HOOK.lock().unwrap() = Some((sent, resume));
    }
    pub(super) fn after_dispose(length: usize, capacity: usize) {
        let hook = HOOK.lock().unwrap().take();
        if let Some((sent, resume)) = hook {
            sent.send((length, capacity)).unwrap();
            resume
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap();
        }
    }
}

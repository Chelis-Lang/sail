//! Callback-owned LoadedForeign authority and distinct deferred diagnostic recipes.
//! Never resolves or loads a source file. Unsupported states remain unavailable.

use super::{CrateInfo, ExternSourceInfo, FileStartEndPos};
use rustc_span::def_id::CrateNum;
#[cfg(test)]
use rustc_span::source_map::SourceMap;
use rustc_span::{ExternalSource, SourceFileHash, StableSourceFileId};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

const RECORD_LIMIT: usize = 4096;
const NAME_LIMIT: usize = 4096;
const BUDGET: usize = 8 * 1024 * 1024;
static OWNED_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

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
    ChangedResolver,
    ChangedSource,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DescriptorRefusal {
    UnsupportedPath,
    MissingOriginalFilename,
    OriginalFilenameMismatch,
    ForeignHashElsewhere,
}

pub(crate) struct LoadedFile {
    pub(crate) index: usize,
    pub(crate) cnum: CrateNum,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) normalized_length: u32,
    pub(crate) unnormalized_length: u32,
    pub(crate) name: String,
    pub(crate) stable_id: StableSourceFileId,
    pub(crate) hash: SourceFileHash,
}

pub(crate) struct DeferredLocator {
    // Exact actual descriptor bytes; no canonicalization or source-file read.
    pub(crate) loader_path: String,
    pub(crate) foreign: LoadedFile,
}

pub(crate) struct OriginalDescriptorLocator {
    // Frozen original filename equals the initial actual descriptor bytes.
    // No content, hash-kind, current source owner or coordinates are retained.
    pub(crate) loader_path: String,
}

pub(crate) enum State {
    Pending(ImportSeed),
    Loaded(LoadedFile),
    Deferred(DeferredLocator),
    OriginalDescriptor(OriginalDescriptorLocator),
    Unavailable(CaptureFailure),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResolverState {
    Loaded,
    Delayed,
    Missing,
    Busy,
    Unknown,
}
#[derive(Clone, Copy)]
pub(crate) struct ResolverObservation {
    pub(crate) state: ResolverState,
    pub(crate) start: Option<u32>,
    pub(crate) end: Option<u32>,
}
impl ResolverObservation {
    fn from_selected(selected: &Result<(u32, u32), CaptureFailure>) -> Self {
        match selected {
            Ok((start, end)) => Self {
                state: ResolverState::Loaded,
                start: Some(*start),
                end: Some(*end),
            },
            Err(reason) => Self {
                state: match reason {
                    CaptureFailure::Delayed => ResolverState::Delayed,
                    CaptureFailure::Missing => ResolverState::Missing,
                    CaptureFailure::Busy => ResolverState::Busy,
                    _ => ResolverState::Unknown,
                },
                start: None,
                end: None,
            },
        }
    }
}

pub(crate) struct ImportSeed {
    original_filename: Option<String>,
    oversized_original_filename: bool,
    recipe: InitialRecipe,
}
enum InitialRecipe {
    Loaded {
        start: u32,
        end: u32,
    },
    Delayed {
        path: String,
        hash: [u8; 32],
        hash_len: u8,
    },
    Unavailable(CaptureFailure),
}
/// A bounded original authority plus exact initial resolver observation.
/// It exposes no prepared provenance or eligible role.
pub(crate) struct PendingImportSeeds {
    owned: OwnedImportMappings,
    used: usize,
}

fn copy_seed_name(value: &str, used: &mut usize) -> Result<String, CaptureFailure> {
    if value.len() > NAME_LIMIT
        || used
            .checked_add(2 * NAME_LIMIT + 1024)
            .is_none_or(|n| n > BUDGET)
    {
        return Err(CaptureFailure::Budget);
    }
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| CaptureFailure::Allocation)?;
    if text.capacity() > NAME_LIMIT {
        return Err(CaptureFailure::Budget);
    }
    text.push_str(value);
    *used = used
        .checked_add(text.capacity())
        .ok_or(CaptureFailure::Budget)?;
    Ok(text)
}

pub(crate) struct Record {
    pub(crate) crate_id: u64,
    pub(crate) original_start: u32,
    pub(crate) original_end: u32,
    // Original metadata has raw bytes only, not a hash-algorithm tag.
    // Preserve those exact bytes; current typed hash adds its own explicit tag.
    pub(crate) expected_hash: [u8; 32],
    pub(crate) expected_hash_len: u8,
    pub(crate) initial: ResolverObservation,
    pub(crate) pre_focus: ResolverObservation,
    pub(crate) state: State,
    pub(crate) descriptor_refusal: Option<DescriptorRefusal>,
    pub(crate) initial_foreign_hash_matches: Option<usize>,
    pub(crate) pre_focus_foreign_hash_matches: Option<usize>,
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
        if self.charged {
            drop_probe::after_dispose(self.records.len(), self.records.capacity());
        }
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

/// Original authority is copied once, before allocation, without an eligible
/// role or source loading. Initial resolver recipes are independently frozen.
pub(super) fn sail_capture_import_seeds(
    imported_crates: &HashMap<u64, CrateInfo>,
    original_crate_files: &HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
) -> Result<PendingImportSeeds, CaptureFailure> {
    // Both namespaces are constructor inputs. Production creates one resolver
    // row for every original row, but reduced/API callers may supply inconsistent
    // maps: cap their work before any per-range search or family allocation.
    if imported_crates.len() > RECORD_LIMIT || original_crate_files.len() > RECORD_LIMIT {
        return Err(CaptureFailure::Budget);
    }
    original_crate_files.values().try_fold(0usize, |count, files| {
        count.checked_add(files.len()).filter(|count| *count <= RECORD_LIMIT)
    }).ok_or(CaptureFailure::Budget)?;
    let count = imported_crates
        .values()
        .try_fold(0usize, |n, info| {
            n.checked_add(info.files.len())
                .filter(|n| *n <= RECORD_LIMIT)
        })
        .ok_or(CaptureFailure::Budget)?;
    let fixed = count
        .checked_mul(std::mem::size_of::<Record>())
        .and_then(|n| n.checked_add(1024))
        .ok_or(CaptureFailure::Budget)?;
    if fixed > BUDGET {
        return Err(CaptureFailure::Budget);
    }
    OWNED_BYTES
        .compare_exchange(0, BUDGET, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| CaptureFailure::Budget)?;
    PEAK_BYTES.fetch_max(BUDGET, Ordering::Relaxed);
    crate::verifier::sail_note_import_charge();
    let mut owned = OwnedImportMappings {
        records: Vec::new(),
        charged: true,
    };
    owned
        .records
        .try_reserve_exact(count)
        .map_err(|_| CaptureFailure::Allocation)?;
    let mut used = owned
        .records
        .capacity()
        .checked_mul(std::mem::size_of::<Record>())
        .and_then(|n| n.checked_add(1024))
        .ok_or(CaptureFailure::Budget)?;
    if owned.records.capacity() > RECORD_LIMIT || used > BUDGET {
        return Err(CaptureFailure::Budget);
    }
    for (crate_id, info) in imported_crates {
        for file in &info.files {
            let original_start = file.original_start_pos.0;
            let original_end = file.original_end_pos.0;
            original_end
                .checked_sub(original_start)
                .ok_or(CaptureFailure::InvalidRange)?;
            let originals = original_crate_files
                .get(crate_id)
                .ok_or(CaptureFailure::MissingOriginalHash)?;
            let mut matches = originals
                .iter()
                .filter(|(_, v)| v.start_pos == original_start && v.end_pos == original_end);
            let (expected, original) = matches.next().ok_or(CaptureFailure::MissingOriginalHash)?;
            if matches.next().is_some() {
                return Err(CaptureFailure::AmbiguousOriginalHash);
            }
            if expected.is_empty() || expected.len() > 32 {
                return Err(CaptureFailure::MissingOriginalHash);
            }
            let mut expected_hash = [0; 32];
            expected_hash[..expected.len()].copy_from_slice(expected);
            let original_name = original.filename.as_ref().and_then(|name| name.to_str());
            let oversized_original_filename =
                original_name.is_some_and(|name| name.len() > NAME_LIMIT);
            let original_filename = original_name
                .filter(|name| name.len() <= NAME_LIMIT)
                .map(|name| copy_seed_name(name, &mut used))
                .transpose()?;
            let mut descriptor_refusal = None;
            let recipe = match file.info.try_lock() {
                Ok(current) => match &*current {
                    ExternSourceInfo::Loaded { start_pos, end_pos } => InitialRecipe::Loaded {
                        start: start_pos.0,
                        end: end_pos.0,
                    },
                    ExternSourceInfo::Delayed { filename, hash } => {
                        if hash.as_slice() != expected.as_slice() {
                            return Err(CaptureFailure::HashMismatch);
                        }
                        match filename.to_str().filter(|name| {
                            filename.is_absolute()
                                && !name.is_empty()
                                && name.len() <= NAME_LIMIT
                                && !name.contains('\0')
                        }) {
                            Some(name) => InitialRecipe::Delayed {
                                path: copy_seed_name(name, &mut used)?,
                                hash: expected_hash,
                                hash_len: expected.len() as u8,
                            },
                            None => {
                                descriptor_refusal = Some(DescriptorRefusal::UnsupportedPath);
                                InitialRecipe::Unavailable(CaptureFailure::Delayed)
                            }
                        }
                    }
                    ExternSourceInfo::None => InitialRecipe::Unavailable(CaptureFailure::Missing),
                },
                Err(_) => InitialRecipe::Unavailable(CaptureFailure::Busy),
            };
            let initial = match &recipe {
                InitialRecipe::Loaded { start, end } => {
                    ResolverObservation::from_selected(&Ok((*start, *end)))
                }
                InitialRecipe::Delayed { .. } => {
                    ResolverObservation::from_selected(&Err(CaptureFailure::Delayed))
                }
                InitialRecipe::Unavailable(reason) => {
                    ResolverObservation::from_selected(&Err(*reason))
                }
            };
            owned.records.push(Record {
                crate_id: *crate_id,
                original_start,
                original_end,
                expected_hash,
                expected_hash_len: expected.len() as u8,
                initial,
                pre_focus: initial,
                state: State::Pending(ImportSeed {
                    original_filename,
                    oversized_original_filename,
                    recipe,
                }),
                descriptor_refusal,
                initial_foreign_hash_matches: None,
                pre_focus_foreign_hash_matches: None,
            });
        }
    }
    owned
        .records
        .sort_unstable_by_key(|r| (r.crate_id, r.original_start));
    if owned
        .records
        .windows(2)
        .any(|p| p[0].crate_id == p[1].crate_id && p[1].original_start <= p[0].original_end)
    {
        return Err(CaptureFailure::AmbiguousOriginalRange);
    }
    Ok(PendingImportSeeds { owned, used })
}

fn recipe_unchanged<'a>(
    seed: &ImportSeed,
    file: &'a super::ExternSourceFile,
) -> Result<std::sync::MutexGuard<'a, ExternSourceInfo>, CaptureFailure> {
    if let InitialRecipe::Unavailable(reason) = &seed.recipe {
        return Err(*reason);
    }
    let current = file.info.try_lock().map_err(|_| CaptureFailure::Busy)?;
    match (&seed.recipe, &*current) {
        (InitialRecipe::Loaded { start, end }, ExternSourceInfo::Loaded { start_pos, end_pos })
            if *start == start_pos.0 && *end == end_pos.0 =>
        {
            Ok(current)
        }
        (
            InitialRecipe::Delayed {
                path,
                hash,
                hash_len,
            },
            ExternSourceInfo::Delayed {
                filename,
                hash: current_hash,
            },
        ) if filename.to_str() == Some(path.as_str())
            && current_hash.as_slice() == &hash[..usize::from(*hash_len)] =>
        {
            Ok(current)
        }
        _ => Err(CaptureFailure::ChangedResolver),
    }
}

/// Single late role classification. The same fixed family array and admission
/// move into the complete owner; no second family or metadata resnapshot exists.
pub(super) fn sail_classify_import_seeds(
    mut pending: PendingImportSeeds,
    imported_crates: &HashMap<u64, CrateInfo>,
    files: &[std::sync::Arc<rustc_span::SourceFile>],
    stable_crate_of: impl Fn(CrateNum) -> u64,
) -> Result<OwnedImportMappings, CaptureFailure> {
    if files.len() > RECORD_LIMIT {
        return Err(CaptureFailure::Budget);
    }
    for record in &mut pending.owned.records {
        let state = std::mem::replace(
            &mut record.state,
            State::Unavailable(CaptureFailure::Unclassified),
        );
        let State::Pending(seed) = state else {
            return Err(CaptureFailure::Unclassified);
        };
        let info = imported_crates
            .get(&record.crate_id)
            .ok_or(CaptureFailure::ChangedResolver)?;
        let mut matches = info.files.iter().filter(|f| {
            f.original_start_pos.0 == record.original_start
                && f.original_end_pos.0 == record.original_end
        });
        let file = matches.next().ok_or(CaptureFailure::ChangedResolver)?;
        if matches.next().is_some() {
            return Err(CaptureFailure::AmbiguousOriginalRange);
        }
        let current = match recipe_unchanged(&seed, file) {
            Ok(current) => current,
            Err(reason) => {
                record.pre_focus = ResolverObservation::from_selected(&Err(reason));
                record.state = State::Unavailable(reason);
                continue;
            }
        };
        classify_record(
            record,
            &seed,
            &current,
            files,
            &stable_crate_of,
            &mut pending.used,
        )?;
        // seed names are physically dropped here while the full family quota
        // remains held. used deliberately overcharges these disposed buffers.
    }
    Ok(pending.owned)
}

fn classify_record(
    record: &mut Record,
    seed: &ImportSeed,
    current_state: &ExternSourceInfo,
    files: &[std::sync::Arc<rustc_span::SourceFile>],
    stable_crate_of: &impl Fn(CrateNum) -> u64,
    used: &mut usize,
) -> Result<(), CaptureFailure> {
    let crate_id = &record.crate_id;
    let original_start = record.original_start;
    let original_end = record.original_end;
    let original_length = original_end
        .checked_sub(original_start)
        .ok_or(CaptureFailure::InvalidRange)?;
    let expected = &record.expected_hash[..usize::from(record.expected_hash_len)];
    // One admitted family holds both roles. Descriptor copying is
    // bounded before allocation and never calls the current resolver.
    let mut deferred_path = None;
    let mut original_path = None;
    let mut descriptor_refusal = None;
    let mut foreign_hash_matches = None;
    let selected = match current_state {
        ExternSourceInfo::Loaded { start_pos, end_pos } => Ok((start_pos.0, end_pos.0)),
        ExternSourceInfo::Delayed { filename, hash } => {
            if hash.as_slice() != expected {
                return Err(CaptureFailure::HashMismatch);
            }
            foreign_hash_matches = Some(
                files
                    .iter()
                    .filter(|current| {
                        matches!(
                            *current.external_src.borrow(),
                            ExternalSource::Foreign { .. }
                        ) && current.src_hash.hash_bytes() == expected
                    })
                    .count(),
            );
            let path = filename.to_str().filter(|path| {
                filename.is_absolute()
                    && !path.is_empty()
                    && path.len() <= NAME_LIMIT
                    && !path.contains('\0')
            });
            if let Some(path) = path {
                let mut matching = files.iter().filter(|current| {
                    matches!(
                        *current.external_src.borrow(),
                        ExternalSource::Foreign { .. }
                    ) && stable_crate_of(current.cnum) == *crate_id
                        && current.src_hash.hash_bytes() == expected
                });
                match (matching.next(), matching.next()) {
                    (Some(current), None) => {
                        // Initial foreign coordinates select metadata
                        // only. The descriptor remains Delayed.
                        let end = current
                            .start_pos
                            .0
                            .checked_add(current.normalized_source_len.0)
                            .ok_or(CaptureFailure::InvalidRange)?;
                        if used
                            .checked_add(2 * NAME_LIMIT + 1024)
                            .filter(|size| *size <= BUDGET)
                            .is_none()
                        {
                            return Err(CaptureFailure::Budget);
                        }
                        let mut copied = String::new();
                        copied
                            .try_reserve_exact(path.len())
                            .map_err(|_| CaptureFailure::Allocation)?;
                        if copied.capacity() > NAME_LIMIT {
                            return Err(CaptureFailure::Budget);
                        }
                        copied.push_str(path);
                        *used = used
                            .checked_add(copied.capacity())
                            .ok_or(CaptureFailure::Budget)?;
                        deferred_path = Some(copied);
                        Ok((current.start_pos.0, end))
                    }
                    (None, _) => {
                        // No owner.cnum exists. Zero equal-hash Foreign
                        // files across ALL crates avoids fabricating
                        // an association from a filename.
                        let any_foreign = files.iter().any(|current| {
                            matches!(
                                *current.external_src.borrow(),
                                ExternalSource::Foreign { .. }
                            ) && current.src_hash.hash_bytes() == expected
                        });
                        let refusal = if any_foreign {
                            Some(DescriptorRefusal::ForeignHashElsewhere)
                        } else {
                            match seed.original_filename.as_deref() {
                                None if seed.oversized_original_filename => {
                                    Some(DescriptorRefusal::OriginalFilenameMismatch)
                                }
                                None => Some(DescriptorRefusal::MissingOriginalFilename),
                                Some(name) if name != path => {
                                    Some(DescriptorRefusal::OriginalFilenameMismatch)
                                }
                                Some(_) => None,
                            }
                        };
                        if let Some(refusal) = refusal {
                            descriptor_refusal = Some(refusal);
                        } else {
                            if used
                                .checked_add(2 * NAME_LIMIT + 1024)
                                .filter(|size| *size <= BUDGET)
                                .is_none()
                            {
                                return Err(CaptureFailure::Budget);
                            }
                            let mut copied = String::new();
                            copied
                                .try_reserve_exact(path.len())
                                .map_err(|_| CaptureFailure::Allocation)?;
                            if copied.capacity() > NAME_LIMIT {
                                return Err(CaptureFailure::Budget);
                            }
                            copied.push_str(path);
                            *used = used
                                .checked_add(copied.capacity())
                                .ok_or(CaptureFailure::Budget)?;
                            original_path = Some(copied);
                        }
                        Err(CaptureFailure::Delayed)
                    }
                    _ => return Err(CaptureFailure::AmbiguousContentOwner),
                }
            } else {
                // Unsupported descriptor remains the old safe miss.
                descriptor_refusal = Some(DescriptorRefusal::UnsupportedPath);
                Err(CaptureFailure::Delayed)
            }
        }
        ExternSourceInfo::None => Err(CaptureFailure::Missing),
    };
    let initial = if deferred_path.is_some() || original_path.is_some() {
        ResolverObservation {
            state: ResolverState::Delayed,
            start: None,
            end: None,
        }
    } else {
        ResolverObservation::from_selected(&selected)
    };
    let state = if let Some(loader_path) = original_path {
        State::OriginalDescriptor(OriginalDescriptorLocator { loader_path })
    } else {
        match selected {
            Err(reason) => State::Unavailable(reason),
            Ok((start, end)) => {
                if end.checked_sub(start) != Some(original_length) {
                    return Err(CaptureFailure::InvalidRange);
                }
                let mut matching = files.iter().enumerate().filter(|(_, current)| {
                    current.start_pos.0 == start
                        && start.checked_add(current.normalized_source_len.0) == Some(end)
                });
                let (index, current) = matching.next().ok_or(CaptureFailure::MissingCurrentFile)?;
                if matching.next().is_some() {
                    return Err(CaptureFailure::AmbiguousCurrentRange);
                }
                if files.iter().enumerate().any(|(other_index, file)| {
                    other_index != index
                        && file
                            .start_pos
                            .0
                            .checked_add(file.normalized_source_len.0)
                            .is_none_or(|other_end| file.start_pos.0 <= end && start <= other_end)
                }) {
                    return Err(CaptureFailure::AmbiguousCurrentRange);
                }
                let expected_hash = expected;
                if expected_hash != current.src_hash.hash_bytes() {
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
                            && candidate.src_hash.hash_bytes() == expected_hash
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
                *used = used
                    .checked_add(name.text.capacity())
                    .ok_or(CaptureFailure::Budget)?;
                let owner = LoadedFile {
                    index,
                    cnum: current.cnum,
                    start,
                    end,
                    normalized_length: current.normalized_source_len.0,
                    unnormalized_length: current.unnormalized_source_len,
                    name: name.text,
                    stable_id: current.stable_id,
                    hash: current.src_hash,
                };
                if let Some(loader_path) = deferred_path {
                    State::Deferred(DeferredLocator {
                        loader_path,
                        foreign: owner,
                    })
                } else {
                    State::Loaded(owner)
                }
            }
        }
    };

    record.initial = initial;
    record.pre_focus = initial;
    record.state = state;
    record.descriptor_refusal = descriptor_refusal;
    record.initial_foreign_hash_matches = foreign_hash_matches;
    record.pre_focus_foreign_hash_matches = foreign_hash_matches;
    Ok(())
}

/// Compatibility entry for unchanged direct role controls: classify immediately
/// at the supplied namespace without changing any role/negative assertion.
#[cfg(test)]
pub(super) fn sail_capture_loaded_imports(
    imported_crates: &HashMap<u64, CrateInfo>,
    original_crate_files: &HashMap<u64, HashMap<Vec<u8>, FileStartEndPos>>,
    source_map: &SourceMap,
    stable_crate_of: impl Fn(CrateNum) -> u64,
) -> Result<OwnedImportMappings, CaptureFailure> {
    let seeds = sail_capture_import_seeds(imported_crates, original_crate_files)?;
    let files = source_map.files();
    sail_classify_import_seeds(seeds, imported_crates, &files, stable_crate_of)
}

struct EqualName<'a> { expected:&'a [u8], position:usize }
impl std::fmt::Write for EqualName<'_> {
    fn write_str(&mut self,value:&str)->std::fmt::Result {
        let end=self.position.checked_add(value.len()).ok_or(std::fmt::Error)?;
        if self.expected.get(self.position..end)!=Some(value.as_bytes()){return Err(std::fmt::Error);}
        self.position=end;Ok(())
    }
}

impl OwnedImportMappings {
    /// Read-only reuse barrier for a USED imported binding. Namespace growth
    /// cannot reclassify an owner. Recipe resolution by the current stock
    /// Reporter remains allowed; it never supplies coordinates to this owner.
    pub(crate) fn validate_binding_files(&self,packed:[u64;2],files:&[std::sync::Arc<rustc_span::SourceFile>])->Result<(),CaptureFailure> {
        if files.len()>RECORD_LIMIT{return Err(CaptureFailure::Budget);}
        let record=self.record(packed)?;
        let expected=&record.expected_hash[..usize::from(record.expected_hash_len)];
        let owner=match &record.state {
            State::OriginalDescriptor(_)=>{
                if files.iter().any(|f|matches!(*f.external_src.borrow(),ExternalSource::Foreign{..})&&f.src_hash.hash_bytes()==expected){return Err(CaptureFailure::ChangedSource);}
                return Ok(());
            },
            State::Loaded(owner)=>owner,
            State::Deferred(locator)=>&locator.foreign,
            State::Unavailable(reason)=>return Err(*reason),
            State::Pending(_)=>return Err(CaptureFailure::Unclassified),
        };
        let current=files.get(owner.index).ok_or(CaptureFailure::MissingCurrentFile)?;
        if current.start_pos.0!=owner.start || current.start_pos.0.checked_add(current.normalized_source_len.0)!=Some(owner.end)
            || current.normalized_source_len.0!=owner.normalized_length || current.unnormalized_source_len!=owner.unnormalized_length
            || current.cnum!=owner.cnum || current.stable_id!=owner.stable_id || current.src_hash!=owner.hash
            || !matches!(*current.external_src.borrow(),ExternalSource::Foreign{..}) {return Err(CaptureFailure::ChangedSource);}
        let mut name=EqualName{expected:owner.name.as_bytes(),position:0};
        if write!(&mut name,"{:?}",current.name).is_err() || name.position!=owner.name.len(){return Err(CaptureFailure::ChangedSource);}
        if files.iter().filter(|f|matches!(*f.external_src.borrow(),ExternalSource::Foreign{..})&&f.cnum==owner.cnum&&f.src_hash.hash_bytes()==expected).count()!=1{return Err(CaptureFailure::AmbiguousContentOwner);}
        if files.iter().enumerate().any(|(index,f)|index!=owner.index&&f.start_pos.0.checked_add(f.normalized_source_len.0).is_none_or(|end|f.start_pos.0<=owner.end&&owner.start<=end)){return Err(CaptureFailure::AmbiguousCurrentRange);}
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn records(&self) -> &[Record] {
        &self.records
    }

    /// Owned range lookup only; not a prepared-span equivalence or raw binding.
    /// Imported stock rendering reconstructs root hygiene and parent=None. A
    /// future caller must first distinguish raw unit imports from local/unknown
    /// raw owners and preserve the exact current SailSpanKey binding.
    pub(crate) fn record(&self, packed: [u64; 2]) -> Result<&Record, CaptureFailure> {
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
        Ok(record)
    }
    pub(crate) fn authority(
        &self,
        packed: [u64; 2],
    ) -> Result<(&Record, &LoadedFile), CaptureFailure> {
        let record = self.record(packed)?;
        let lo = (packed[1] >> 32) as u32;
        let hi = packed[1] as u32;
        let current = match &record.state {
            State::Loaded(current) => current,
            State::Deferred(_) | State::OriginalDescriptor(_) => {
                return Err(CaptureFailure::Delayed);
            }
            State::Unavailable(reason) => return Err(*reason),
            State::Pending(_) => return Err(CaptureFailure::Unclassified),
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
        Ok((record, current))
    }

    /// A no-Foreign recipe exposes retained original descriptor authority only.
    /// It cannot supply current coordinates, loaded bytes or a hash kind.
    pub(crate) fn original_descriptor(
        &self,
        packed: [u64; 2],
    ) -> Result<(&Record, &OriginalDescriptorLocator), CaptureFailure> {
        let record = self.record(packed)?;
        match &record.state {
            State::OriginalDescriptor(locator) => Ok((record, locator)),
            State::Unavailable(reason) => Err(*reason),
            State::Pending(_) => Err(CaptureFailure::Unclassified),
            _ => Err(CaptureFailure::NotForeign),
        }
    }

    /// A recipe lookup performs no location resolution. Its role stays Deferred
    /// even after the current stock Reporter loads or refuses its own descriptor.
    pub(crate) fn deferred(
        &self,
        packed: [u64; 2],
    ) -> Result<(&Record, &DeferredLocator), CaptureFailure> {
        let record = self.record(packed)?;
        match &record.state {
            State::Deferred(locator) => Ok((record, locator)),
            State::Unavailable(reason) => Err(*reason),
            State::Pending(_) => Err(CaptureFailure::Unclassified),
            State::Loaded(_) => Err(CaptureFailure::ChangedResolver),
            State::OriginalDescriptor(_) => Err(CaptureFailure::NotForeign),
        }
    }

    pub(crate) fn current_range(&self, packed: [u64; 2]) -> Result<(u32, u32), CaptureFailure> {
        let (record, current) = self.authority(packed)?;
        Ok((
            current.start + ((packed[1] >> 32) as u32 - record.original_start),
            current.start + (packed[1] as u32 - record.original_start),
        ))
    }

    /// Compiler-thread late barrier, on the SAME admitted owner. New files may
    /// introduce ambiguity. Delayed recipes must stay exact until this handoff;
    /// unavailable states are never promoted and no resolver value is written.
    /// No SourceMap/lock/SourceFile borrow or clone is retained by this method.
    #[cfg(test)]
    pub(super) fn revalidate(&mut self, imported: &HashMap<u64, CrateInfo>, map: &SourceMap) {
        let files = map.files();
        self.revalidate_files(imported, &files);
    }

    pub(super) fn revalidate_files(
        &mut self,
        imported: &HashMap<u64, CrateInfo>,
        files: &[std::sync::Arc<rustc_span::SourceFile>],
    ) {
        for record in &mut self.records {
            // Refuse an oversized namespace BEFORE any descriptor or Foreign
            // count scan. It cannot produce unbounded per-record work or a
            // misleading partial count at handoff.
            if files.len() > RECORD_LIMIT {
                record.pre_focus = ResolverObservation::from_selected(&Err(CaptureFailure::Budget));
                record.pre_focus_foreign_hash_matches = None;
                record.state = State::Unavailable(CaptureFailure::Budget);
                continue;
            }
            // Read the ACTUAL current state even for initially unsupported
            // entries; observation never promotes their eligibility.
            let selected = (|| {
                let info = imported
                    .get(&record.crate_id)
                    .ok_or(CaptureFailure::ChangedResolver)?;
                let mut matches = info.files.iter().filter(|file| {
                    file.original_start_pos.0 == record.original_start
                        && file.original_end_pos.0 == record.original_end
                });
                let file = matches.next().ok_or(CaptureFailure::ChangedResolver)?;
                if matches.next().is_some() {
                    return Err(CaptureFailure::AmbiguousOriginalRange);
                }
                let state = file.info.try_lock().map_err(|_| CaptureFailure::Busy)?;
                match &*state {
                    ExternSourceInfo::Loaded { start_pos, end_pos } => Ok((start_pos.0, end_pos.0)),
                    ExternSourceInfo::Delayed { .. } => Err(CaptureFailure::Delayed),
                    ExternSourceInfo::None => Err(CaptureFailure::Missing),
                }
            })();
            record.pre_focus = ResolverObservation::from_selected(&selected);
            record.pre_focus_foreign_hash_matches = Some(
                files
                    .iter()
                    .filter(|current| {
                        matches!(
                            *current.external_src.borrow(),
                            ExternalSource::Foreign { .. }
                        ) && current.src_hash.hash_bytes()
                            == &record.expected_hash[..usize::from(record.expected_hash_len)]
                    })
                    .count(),
            );
            if let State::OriginalDescriptor(locator) = &record.state {
                let valid = (|| {
                    if files.len() > RECORD_LIMIT {
                        return Err(CaptureFailure::Budget);
                    }
                    if !matches!(selected, Err(CaptureFailure::Delayed)) {
                        return Err(CaptureFailure::ChangedResolver);
                    }
                    let info = imported
                        .get(&record.crate_id)
                        .ok_or(CaptureFailure::ChangedResolver)?;
                    let mut matching = info.files.iter().filter(|file| {
                        file.original_start_pos.0 == record.original_start
                            && file.original_end_pos.0 == record.original_end
                    });
                    let file = matching.next().ok_or(CaptureFailure::ChangedResolver)?;
                    if matching.next().is_some() {
                        return Err(CaptureFailure::AmbiguousOriginalRange);
                    }
                    let current = file.info.try_lock().map_err(|_| CaptureFailure::Busy)?;
                    let expected = &record.expected_hash[..usize::from(record.expected_hash_len)];
                    match &*current {
                        ExternSourceInfo::Delayed { filename, hash }
                            if filename.to_str() == Some(locator.loader_path.as_str())
                                && hash.as_slice() == expected => {}
                        _ => return Err(CaptureFailure::ChangedResolver),
                    }
                    // Original metadata was immutable constructor input. No
                    // original-table reread is claimed: compare the MUTABLE
                    // current recipe against retained original authority.
                    if files.iter().any(|file| {
                        matches!(*file.external_src.borrow(), ExternalSource::Foreign { .. })
                            && file.src_hash.hash_bytes() == expected
                    }) {
                        return Err(CaptureFailure::ChangedSource);
                    }
                    Ok(())
                })();
                if let Err(reason) = valid {
                    record.state = State::Unavailable(reason);
                }
                continue;
            }
            let (owner, deferred) = match &record.state {
                State::Loaded(owner) => (owner, None),
                State::Deferred(locator) => (&locator.foreign, Some(locator)),
                State::Unavailable(_) | State::OriginalDescriptor(_) | State::Pending(_) => {
                    continue;
                }
            };
            let valid = (|| {
                if files.len() > RECORD_LIMIT {
                    return Err(CaptureFailure::Budget);
                }
                if let Some(locator) = deferred {
                    if !matches!(selected, Err(CaptureFailure::Delayed)) {
                        return Err(CaptureFailure::ChangedResolver);
                    }
                    let info = imported
                        .get(&record.crate_id)
                        .ok_or(CaptureFailure::ChangedResolver)?;
                    let mut matching = info.files.iter().filter(|file| {
                        file.original_start_pos.0 == record.original_start
                            && file.original_end_pos.0 == record.original_end
                    });
                    let file = matching.next().ok_or(CaptureFailure::ChangedResolver)?;
                    if matching.next().is_some() {
                        return Err(CaptureFailure::AmbiguousOriginalRange);
                    }
                    let state = file.info.try_lock().map_err(|_| CaptureFailure::Busy)?;
                    match &*state {
                        ExternSourceInfo::Delayed { filename, hash }
                            if filename.to_str() == Some(locator.loader_path.as_str())
                                && hash.as_slice()
                                    == &record.expected_hash
                                        [..usize::from(record.expected_hash_len)] => {}
                        _ => return Err(CaptureFailure::ChangedResolver),
                    }
                } else {
                    match selected {
                        Ok((start, end)) if start == owner.start && end == owner.end => {}
                        Err(reason) => return Err(reason),
                        _ => return Err(CaptureFailure::ChangedResolver),
                    }
                }
                let current = files
                    .get(owner.index)
                    .ok_or(CaptureFailure::MissingCurrentFile)?;
                if current.start_pos.0 != owner.start
                    || current
                        .start_pos
                        .0
                        .checked_add(current.normalized_source_len.0)
                        != Some(owner.end)
                    || current.normalized_source_len.0 != owner.normalized_length
                    || current.unnormalized_source_len != owner.unnormalized_length
                    || current.cnum != owner.cnum
                    || current.stable_id != owner.stable_id
                    || current.src_hash != owner.hash
                {
                    return Err(CaptureFailure::ChangedSource);
                }
                if !matches!(
                    *current.external_src.borrow(),
                    ExternalSource::Foreign { .. }
                ) {
                    return Err(CaptureFailure::NotForeign);
                }
                let expected = &record.expected_hash[..usize::from(record.expected_hash_len)];
                if current.src_hash.hash_bytes() != expected {
                    return Err(CaptureFailure::HashMismatch);
                }
                // Same-callback namespace/cnum identity was frozen at construction.
                // Added equal-content candidates invalidate, including another
                // algorithm with the same raw metadata bytes.
                if files
                    .iter()
                    .filter(|file| {
                        file.cnum == owner.cnum
                            && matches!(*file.external_src.borrow(), ExternalSource::Foreign { .. })
                            && file.src_hash.hash_bytes() == expected
                    })
                    .count()
                    != 1
                {
                    return Err(CaptureFailure::AmbiguousContentOwner);
                }
                if files
                    .iter()
                    .filter(|file| {
                        file.start_pos.0 == owner.start
                            && owner.start.checked_add(file.normalized_source_len.0)
                                == Some(owner.end)
                    })
                    .count()
                    != 1
                {
                    return Err(CaptureFailure::AmbiguousCurrentRange);
                }
                if files.iter().enumerate().any(|(index, file)| {
                    index != owner.index
                        && file
                            .start_pos
                            .0
                            .checked_add(file.normalized_source_len.0)
                            .is_none_or(|end| file.start_pos.0 <= owner.end && owner.start <= end)
                }) {
                    return Err(CaptureFailure::AmbiguousCurrentRange);
                }
                // The exact audited formatter writes into a borrowed comparator,
                // rather than allocating another complete name or snapshot.
                let mut name = NameEquals {
                    remaining: &owner.name,
                    equal: true,
                };
                write!(&mut name, "{:?}", current.name)
                    .map_err(|_| CaptureFailure::ChangedSource)?;
                if !name.equal || !name.remaining.is_empty() {
                    return Err(CaptureFailure::ChangedSource);
                }
                Ok(())
            })();
            if let Err(reason) = valid {
                record.state = State::Unavailable(reason);
            }
        }
    }
}

struct NameEquals<'a> {
    remaining: &'a str,
    equal: bool,
}
impl std::fmt::Write for NameEquals<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if let Some(remaining) = self.remaining.strip_prefix(value) {
            self.remaining = remaining;
        } else {
            self.equal = false;
            return Err(std::fmt::Error);
        }
        Ok(())
    }
}

pub(crate) fn retained_peak() -> usize {
    PEAK_BYTES.load(Ordering::Relaxed)
}

pub(crate) fn retained_bytes() -> usize {
    OWNED_BYTES.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn memo_test_authority(content: &str) -> OwnedImportMappings {
    // Small synthetic typed fixture for the EXISTING memo/component tests.
    // This bypass is test-only, never an acquisition/physical-budget control.
    // Actual SourceMap/stock-resolver controls use sail_capture_loaded_imports.
    let hash = SourceFileHash::new_in_memory(rustc_span::SourceFileHashAlgorithm::Sha256, content);
    let mut expected_hash = [0; 32];
    expected_hash.copy_from_slice(hash.hash_bytes());
    OwnedImportMappings {
        records: vec![Record {
            crate_id: 1,
            original_start: 0,
            original_end: 100,
            expected_hash,
            expected_hash_len: 32,
            initial: ResolverObservation {
                state: ResolverState::Loaded,
                start: Some(100),
                end: Some(200),
            },
            pre_focus: ResolverObservation {
                state: ResolverState::Loaded,
                start: Some(100),
                end: Some(200),
            },
            descriptor_refusal: None,
            initial_foreign_hash_matches: None,
            pre_focus_foreign_hash_matches: None,
            state: State::Loaded(LoadedFile {
                index: 1,
                cnum: CrateNum::from_u32(2),
                start: 100,
                end: 200,
                normalized_length: 100,
                unnormalized_length: 100,
                name: "foreign.rs".into(),
                stable_id: StableSourceFileId::default(),
                hash,
            }),
        }],
        charged: false,
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

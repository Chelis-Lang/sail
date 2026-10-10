//! Independent durable proof observation journal; never a source of final verdicts.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sail_api::proof::{Cursor, Event, InputEvent, Page, VERSION};

const MAX_EVENT: usize = 4 * 1024 * 1024;
const INGEST: u64 = 256 * 1024;
const BUDGET: usize = 16 * 1024 * 1024;
const MAX_EVENTS: usize = 4096;
const MAX_ACTIVE_JOURNALS: usize = 8;

#[derive(Default)]
pub(crate) struct Journals(Mutex<BTreeMap<PathBuf, Arc<Mutex<Journal>>>>);

#[derive(Default)]
struct Journal {
    offset: u64,
    tail: Vec<u8>,
    events: Vec<Event>,
    identities: BTreeMap<String, usize>,
    gap: Option<String>,
    identity: Option<String>,
    halted: bool,
    retained_bytes: usize,
}

impl Journals {
    pub(crate) fn page(
        &self,
        directory: &Path,
        id: &str,
        cursor: &Cursor,
        limit: usize,
        terminal: bool,
    ) -> io::Result<Page> {
        if !cursor.stream_id.is_empty() && cursor.stream_id != id {
            return Err(io::Error::other(
                "SAIL0902: cursor belongs to another proof stream",
            ));
        }
        let mut journals = self
            .0
            .lock()
            .map_err(|_| io::Error::other("SAIL0902: proof journal lock poisoned"))?;
        journals.retain(|path, _| path.exists());
        if !journals.contains_key(directory) {
            if journals.len() >= MAX_ACTIVE_JOURNALS
                && let Some(oldest) = journals.keys().next().cloned()
            {
                journals.remove(&oldest);
            }
            journals.insert(
                directory.to_owned(),
                Arc::new(Mutex::new(Journal::load(directory, id)?)),
            );
        }
        let journal = Arc::clone(
            journals
                .get(directory)
                .ok_or_else(|| io::Error::other("SAIL0902: missing journal"))?,
        );
        drop(journals);
        let mut journal = journal
            .lock()
            .map_err(|_| io::Error::other("SAIL0902: job proof journal lock poisoned"))?;
        journal.ingest(directory, id, terminal)?;
        let latest = journal.events.last().map_or(0, |event| event.sequence);
        if cursor.after > latest {
            return Err(io::Error::other(
                "SAIL0902: cursor is ahead of the proof journal",
            ));
        }
        let mut bytes = 0;
        let events: Vec<_> = journal
            .events
            .iter()
            .filter(|event| event.sequence > cursor.after)
            .take(limit.clamp(1, 64))
            .take_while(|event| {
                bytes += serde_json::to_vec(event).map_or(MAX_EVENT, |encoded| encoded.len());
                bytes <= 8 * 1024 * 1024
            })
            .cloned()
            .collect();
        let drained = std::fs::metadata(directory.join("proof-source.jsonl"))
            .map_or(true, |metadata| journal.offset >= metadata.len());
        let exhausted = events.last().map_or(cursor.after, |event| event.sequence) == latest;
        Ok(Page {
            version: VERSION,
            next: Cursor {
                stream_id: id.into(),
                after: events.last().map_or(cursor.after, |event| event.sequence),
            },
            events,
            terminal: terminal && drained && exhausted,
            gap: journal.gap.clone(),
        })
    }
}

impl Journal {
    fn record_identity(&mut self, directory: &Path, identity: String) -> io::Result<()> {
        if self.identity.is_none() {
            std::fs::write(directory.join("proof-source-identity"), &identity)?;
            self.identity = Some(identity);
        }
        Ok(())
    }
    fn halt(&mut self, directory: &Path, message: &str) -> io::Result<()> {
        self.halted = true;
        self.tail.clear();
        self.gap = Some(message.into());
        std::fs::write(directory.join("proof-halted"), b"1")?;
        std::fs::write(directory.join("proof-gap.txt"), message)
    }
    fn load(directory: &Path, id: &str) -> io::Result<Self> {
        let mut journal = Self::default();
        match File::open(directory.join("proof-journal.jsonl")).and_then(|file| {
            let mut bytes = Vec::new();
            file.take(BUDGET as u64 + 1).read_to_end(&mut bytes)?;
            Ok(bytes)
        }) {
            Ok(bytes) => {
                if bytes.len() > BUDGET {
                    journal.halted = true;
                    journal.gap = Some("SAIL0902: retained journal exceeds budget".into());
                    return Ok(journal);
                }
                journal.retained_bytes = bytes.len();
                for line in bytes
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                {
                    let Ok(event) = serde_json::from_slice::<Event>(line) else {
                        journal.gap = Some(
                            "SAIL0902: retained proof journal contains malformed evidence".into(),
                        );
                        break;
                    };
                    if event.version != VERSION
                        || event.stream_id != id
                        || event.sequence != journal.events.len() as u64 + 1
                        || event.input.validate().is_err()
                    {
                        journal.gap = Some(
                            "SAIL0902: retained proof journal identity/sequence is invalid".into(),
                        );
                        break;
                    }
                    journal
                        .identities
                        .insert(event.input.event_id.clone(), journal.events.len());
                    journal.events.push(event);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if let Ok(gap) = std::fs::read_to_string(directory.join("proof-gap.txt")) {
            journal.gap = Some(gap);
        }
        journal.identity = std::fs::read_to_string(directory.join("proof-source-identity")).ok();
        journal.halted |= directory.join("proof-halted").is_file();
        Ok(journal)
    }

    fn ingest(&mut self, directory: &Path, id: &str, terminal: bool) -> io::Result<()> {
        let source = directory.join("proof-source.jsonl");
        let mut input = match File::open(source) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if terminal && self.events.is_empty() {
                    self.gap =
                        Some("SAIL0903: this job produced no interactive proof stream".into());
                }
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let metadata = input.metadata()?;
        let identity = format!("{}:{}", metadata.dev(), metadata.ino());
        if self
            .identity
            .as_ref()
            .is_some_and(|known| known != &identity)
            || metadata.len() < self.offset
        {
            return self.halt(
                directory,
                "SAIL0902: proof producer spool replaced or truncated",
            );
        }
        self.record_identity(directory, identity)?;
        if self.halted {
            self.offset = metadata.len();
            return Ok(());
        }
        input.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        input.take(INGEST).read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;
        self.tail.extend(bytes);
        while let Some(end) = self.tail.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = self.tail.drain(..=end).collect();
            let event = serde_json::from_slice::<InputEvent>(&line)
                .ok()
                .filter(|event| event.validate().is_ok());
            if line.len() > MAX_EVENT || event.is_none() {
                self.gap = Some(
                    "SAIL0902: malformed, oversized or incomplete proof event rejected".into(),
                );
                continue;
            }
            let Some(event) = event else { continue };
            if let Some(index) = self.identities.get(&event.event_id) {
                if self
                    .events
                    .get(*index)
                    .is_none_or(|known| known.input != event)
                {
                    self.gap =
                        Some("SAIL0902: producer event id reused with different evidence".into());
                }
                continue;
            }
            let envelope = Event {
                version: VERSION,
                stream_id: id.into(),
                sequence: self.events.len() as u64 + 1,
                input: event,
            };
            let mut encoded = serde_json::to_vec(&envelope).map_err(io::Error::other)?;
            encoded.push(b'\n');
            if self.events.len() >= MAX_EVENTS
                || self.retained_bytes.saturating_add(encoded.len()) > BUDGET
            {
                self.offset = metadata.len();
                self.tail.clear();
                return self.halt(directory,"SAIL0902: proof journal retention budget exhausted; later observations unavailable");
            }
            self.retained_bytes += encoded.len();
            OpenOptions::new()
                .append(true)
                .create(true)
                .mode(0o600)
                .open(directory.join("proof-journal.jsonl"))?
                .write_all(&encoded)?;
            self.identities
                .insert(envelope.input.event_id.clone(), self.events.len());
            self.events.push(envelope);
        }
        if self.tail.len() > MAX_EVENT {
            self.tail.clear();
            self.gap = Some("SAIL0902: oversized unterminated proof event rejected".into());
        }
        if terminal
            && !self.tail.is_empty()
            && self.offset == std::fs::metadata(directory.join("proof-source.jsonl"))?.len()
        {
            self.tail.clear();
            self.gap = Some("SAIL0902: producer ended with a truncated proof event".into());
        }
        if let Some(gap) = &self.gap {
            std::fs::write(directory.join("proof-gap.txt"), gap)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "proof_journal_tests.rs"]
mod tests;

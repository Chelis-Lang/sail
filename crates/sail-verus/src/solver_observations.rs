//! Data-only decoder for the candidate solver-launch journal protocol.
//!
//! A decoded journal grants no solver-origin authority. Admission additionally
//! binds complete original bytes and a retained original process descriptor.
//! The currently selected verifier runtime does not emit this candidate protocol.
use sail_acceptance::solver_observations::{self as core, LaunchSpec, WaitObservation};

/// Owned original journal bytes and their untrusted decoded records.
#[derive(Debug)]
pub struct Journal {
    raw: Vec<u8>,
    events: Vec<core::JournalEvent>,
}

impl Journal {
    /// Decodes complete data without granting provenance or process authority.
    ///
    /// Returns `None` for partial, unknown, oversized or malformed record fields.
    #[must_use]
    pub fn decode(raw: Vec<u8>) -> Option<Self> {
        let events = decode_journal(&raw)?;
        Some(Self { raw, events })
    }

    /// Borrows the exact complete original bytes, including outer framing.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Checks complete lifecycle and byte binding against an independent process.
    ///
    /// These arguments must come from the retained original child descriptor.
    /// This result alone grants no physical origin: the service also requires
    /// the protected journal path/producer and selected runtime correspondence.
    #[must_use]
    pub fn admit(
        &self,
        request: &[u8],
        actual_pid: u32,
        actual_status: WaitObservation,
    ) -> Option<core::AdmittedJournal<'_>> {
        core::admit_journal(&self.raw, &self.events, request, actual_pid, actual_status)
    }
}

fn record_word(frame: &[u8], offset: &mut usize) -> Option<usize> {
    usize::try_from(core::read_word(frame, offset)?).ok()
}
fn record_pid(frame: &[u8], offset: &mut usize) -> Option<u32> {
    u32::try_from(core::read_word(frame, offset)?).ok()
}
fn record_flag(frame: &[u8], offset: &mut usize) -> Option<bool> {
    let byte = *frame.get(*offset)?;
    *offset += 1;
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
fn decode_record(frame: &[u8]) -> Option<core::JournalEvent> {
    let mut offset = 1;
    let event = match *frame.first()? {
        0 => {
            if frame.get(offset) != Some(&1) {
                return None;
            }
            offset += 1;
            let request = core::copy_bytes(core::read_field(frame, &mut offset)?);
            let verifier_pid = record_pid(frame, &mut offset)?;
            core::JournalEvent::Begin {
                request,
                verifier_pid,
            }
        }
        1 => {
            let stream = record_word(frame, &mut offset)?;
            let solver = core::copy_bytes(core::read_field(frame, &mut offset)?);
            let executable = core::copy_bytes(core::read_field(frame, &mut offset)?);
            let count = record_word(frame, &mut offset)?;
            // Every argument has at least an eight-byte length field. Refuse
            // before allocating/looping over an impossible attacker count.
            if count > (frame.len() - offset) / 8 {
                return None;
            }
            let mut arguments = Vec::new();
            for _ in 0..count {
                arguments.push(core::copy_bytes(core::read_field(frame, &mut offset)?));
            }
            core::JournalEvent::Intent {
                stream,
                command: LaunchSpec {
                    executable,
                    solver,
                    arguments,
                },
            }
        }
        2 => core::JournalEvent::Started {
            stream: record_word(frame, &mut offset)?,
            pid: record_pid(frame, &mut offset)?,
        },
        3 => {
            let stream = record_word(frame, &mut offset)?;
            let pid = record_pid(frame, &mut offset)?;
            let normal = record_flag(frame, &mut offset)?;
            let code = record_pid(frame, &mut offset)?.cast_signed();
            core::JournalEvent::Waited {
                stream,
                pid,
                status: WaitObservation { normal, code },
            }
        }
        4 => {
            let attempts = record_word(frame, &mut offset)?;
            let started = record_word(frame, &mut offset)?;
            let closed = record_word(frame, &mut offset)?;
            let complete = record_flag(frame, &mut offset)?;
            let verifier_exit = record_pid(frame, &mut offset)?.cast_signed();
            core::JournalEvent::End {
                attempts,
                started,
                closed,
                complete,
                verifier_exit,
            }
        }
        _ => return None,
    };
    if offset == frame.len() {
        Some(event)
    } else {
        None
    }
}

/// Decodes data only. Callers must invoke checked `admit_journal` against the
/// original whole byte slice and independently retained request/PID/exit.
/// No decoded event, count, end flag or parser success grants authority.
fn decode_journal(bytes: &[u8]) -> Option<Vec<core::JournalEvent>> {
    let mut offset = 0;
    let mut events = Vec::new();
    while offset < bytes.len() {
        events.push(decode_record(core::read_field(bytes, &mut offset)?)?);
    }
    Some(events)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test fixture assertions"
)]
mod tests {
    use super::*;

    fn events() -> Vec<core::JournalEvent> {
        vec![
            core::JournalEvent::Begin {
                request: vec![9, 0, 255],
                verifier_pid: 17,
            },
            core::JournalEvent::Intent {
                stream: 0,
                command: LaunchSpec {
                    executable: b"/pinned/sail-z3".to_vec(),
                    arguments: vec![b"-smt2".to_vec(), Vec::new(), vec![b'-', 0, 255]],
                    solver: b"z3".to_vec(),
                },
            },
            core::JournalEvent::Started { stream: 0, pid: 41 },
            core::JournalEvent::Waited {
                stream: 0,
                pid: 41,
                status: WaitObservation {
                    normal: true,
                    code: 0,
                },
            },
            core::JournalEvent::End {
                attempts: 1,
                started: 1,
                closed: 1,
                complete: true,
                verifier_exit: 0,
            },
        ]
    }

    fn status() -> WaitObservation {
        WaitObservation {
            normal: true,
            code: 0,
        }
    }

    #[test]
    fn exact_nonempty_binary_fields_survive_whole_journal_admission() {
        let raw = core::encode_journal(&events());
        let journal = Journal::decode(raw.clone()).unwrap();
        assert_eq!(journal.raw(), raw);
        let admitted = journal.admit(&[9, 0, 255], 17, status()).unwrap();
        assert_eq!(admitted.stream_count(), 1);
        let (command, pid, waited) = admitted.stream(0).unwrap();
        assert_eq!(command.executable, b"/pinned/sail-z3");
        assert_eq!(
            command.arguments,
            [b"-smt2".to_vec(), Vec::new(), vec![b'-', 0, 255]]
        );
        assert_eq!(command.solver, b"z3");
        assert_eq!(pid, 41);
        assert_eq!(waited, status());
        assert!(admitted.stream(1).is_none());
        assert!(journal.admit(&[9, 0, 254], 17, status()).is_none());
        assert!(journal.admit(&[9, 0, 255], 18, status()).is_none());
        assert!(
            journal
                .admit(
                    &[9, 0, 255],
                    17,
                    WaitObservation {
                        normal: false,
                        code: 0
                    }
                )
                .is_none()
        );
        assert!(
            journal
                .admit(
                    &[9, 0, 255],
                    17,
                    WaitObservation {
                        normal: true,
                        code: 1
                    }
                )
                .is_none()
        );
    }

    #[test]
    fn every_truncated_prefix_and_extra_or_fabricated_lifecycle_refuses() {
        let raw = core::encode_journal(&events());
        for length in 0..raw.len() {
            if let Some(journal) = Journal::decode(raw[..length].to_vec()) {
                assert!(
                    journal.admit(&[9, 0, 255], 17, status()).is_none(),
                    "length {length}"
                );
            }
        }
        let mut doubled = raw.clone();
        doubled.extend_from_slice(&raw);
        let journal = Journal::decode(doubled).unwrap();
        assert!(journal.admit(&[9, 0, 255], 17, status()).is_none());
        for mutation in 0..5 {
            let mut records = events();
            match mutation {
                0 => {
                    records.remove(2);
                }
                1 => {
                    records.remove(3);
                }
                2 => {
                    records.insert(3, core::JournalEvent::Started { stream: 0, pid: 41 });
                }
                3 => {
                    records[3] = core::JournalEvent::Waited {
                        stream: 0,
                        pid: 42,
                        status: status(),
                    };
                }
                _ => {
                    records[1] = core::JournalEvent::Intent {
                        stream: 1,
                        command: LaunchSpec {
                            executable: b"z3".to_vec(),
                            arguments: Vec::new(),
                            solver: b"z3".to_vec(),
                        },
                    };
                }
            }
            let journal = Journal::decode(core::encode_journal(&records)).unwrap();
            assert!(
                journal.admit(&[9, 0, 255], 17, status()).is_none(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn zero_smt_requires_explicit_matching_begin_and_terminal_eof() {
        let records = vec![
            core::JournalEvent::Begin {
                request: vec![9, 0, 255],
                verifier_pid: 17,
            },
            core::JournalEvent::End {
                attempts: 0,
                started: 0,
                closed: 0,
                complete: true,
                verifier_exit: 0,
            },
        ];
        let journal = Journal::decode(core::encode_journal(&records)).unwrap();
        assert_eq!(
            journal
                .admit(&[9, 0, 255], 17, status())
                .unwrap()
                .stream_count(),
            0
        );
        let empty = Journal::decode(Vec::new()).unwrap();
        assert!(empty.admit(&[9, 0, 255], 17, status()).is_none());
        let mut raw = core::encode_journal(&records);
        raw.push(0);
        assert!(Journal::decode(raw).is_none());
    }
}

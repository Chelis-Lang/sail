//! Pure decoding reuse, never reuse of an RPC or freshness decision.

use std::io::{self, Read};
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::ownership::Owned;
use super::{Envelope, MAX_FRAME, Response, VERSION};

const WIRE_LIMIT: usize = 8 * 1024 * 1024;
const OWNED_LIMIT: usize = 128 * 1024 * 1024;

#[derive(Debug, Default)]
struct Budget(AtomicUsize);
impl Budget {
    fn reserve(self: &Arc<Self>, size: usize) -> Option<Reservation> {
        self.0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(size).filter(|total| *total <= OWNED_LIMIT)
            })
            .ok()?;
        Some(Reservation {
            budget: Arc::clone(self),
            size,
        })
    }
}
#[derive(Debug)]
struct Reservation {
    budget: Arc<Budget>,
    size: usize,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.budget.0.fetch_sub(self.size, Ordering::AcqRel);
    }
}
struct Entry {
    bytes: Box<[u8]>,
    response: Option<Response>,
    promoting: AtomicBool,
    _reservation: Reservation,
}

struct Promotion<'a>(&'a Entry);
impl Drop for Promotion<'_> {
    fn drop(&mut self) {
        // No retry inside this call. Later fresh RPCs may try again after a
        // temporary resource refusal; successful publication changes generation.
        self.0.promoting.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct Cache {
    staged: bool,
    entry: Mutex<Option<Arc<Entry>>>,
    budget: Arc<Budget>,
}
impl std::fmt::Debug for Cache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodedResultCache")
            .field(
                "retained_requested_bytes",
                &self.budget.0.load(Ordering::Acquire),
            )
            .finish_non_exhaustive()
    }
}
/// Entry captured before this RPC starts; never taken after its network read.
pub(super) struct Admission(Option<Arc<Entry>>);

impl Cache {
    pub(super) fn staged() -> Self {
        Self {
            staged: true,
            ..Self::default()
        }
    }
    pub(super) fn begin(&self) -> Admission {
        Admission(self.snapshot())
    }
    #[cfg(test)]
    pub(super) fn read(&self, reader: &mut impl Read) -> io::Result<Response> {
        self.read_at(reader, self.begin())
    }
    pub(super) fn read_at(
        &self,
        reader: &mut impl Read,
        admission: Admission,
    ) -> io::Result<Response> {
        let mut length = [0; 4];
        reader.read_exact(&mut length)?;
        let size = usize::try_from(u32::from_be_bytes(length)).map_err(io::Error::other)?;
        if size > MAX_FRAME {
            return Err(io::Error::other("SAIL0801: job frame exceeds limit"));
        }
        let mut bytes = vec![0; size];
        reader.read_exact(&mut bytes)?;
        // Only a complete fresh frame, byte-identical to a parsed version1
        // frame, certifies the syntax/version/trailing-data/content match.
        let matching = admission.0.filter(|entry| entry.bytes.as_ref() == bytes);
        if let Some(response) = matching.as_ref().and_then(|entry| entry.response.as_ref()) {
            return Ok(response.clone());
        }
        let envelope: Envelope<Response> =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if envelope.version != VERSION {
            return Err(io::Error::other("SAIL0802: incompatible job protocol"));
        }
        if self.staged {
            if let Some(entry) = matching {
                // No wait behind another promotion. Overlapping cold requests
                // all captured None before I/O, so none clones a decoded graph.
                if self.is_current(&entry)
                    && entry
                        .promoting
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                {
                    let _claim = Promotion(&entry);
                    self.retain(&bytes, &envelope.body, Some(&entry));
                }
            } else {
                self.retain_frame(bytes, &envelope.body);
            }
        } else {
            self.retain(&bytes, &envelope.body, None);
        }
        Ok(envelope.body)
    }
    fn is_current(&self, expected: &Arc<Entry>) -> bool {
        self.entry
            .try_lock()
            .is_ok_and(|slot| slot.as_ref().is_some_and(|now| Arc::ptr_eq(now, expected)))
    }
    fn snapshot(&self) -> Option<Arc<Entry>> {
        self.entry.try_lock().ok()?.as_ref().map(Arc::clone)
    }
    fn base_size(bytes: usize) -> Option<usize> {
        bytes.checked_add(size_of::<Entry>() + 2 * size_of::<usize>())
    }
    fn eligible(bytes: &[u8], response: &Response) -> bool {
        bytes.len() <= WIRE_LIMIT
            && super::ownership::audited_layout()
            && matches!(response, Response::Result(_))
    }
    fn retain_frame(&self, bytes: Vec<u8>, response: &Response) {
        if !Self::eligible(&bytes, response) {
            return;
        }
        let Some(size) = Self::base_size(bytes.len()) else {
            return;
        };
        // Box removes Vec spare capacity; its bytes are the original complete
        // parsed frame, with no extra decoded graph clone in this first stage.
        let Some(reservation) = self.budget.reserve(size) else {
            return;
        };
        let entry = Arc::new(Entry {
            bytes: bytes.into_boxed_slice(),
            response: None,
            promoting: AtomicBool::new(false),
            _reservation: reservation,
        });
        self.publish(entry, None);
    }
    fn publish(&self, entry: Arc<Entry>, expected: Option<&Arc<Entry>>) {
        if let Ok(mut slot) = self.entry.try_lock() {
            if expected.is_some_and(|expected| {
                !slot.as_ref().is_some_and(|now| Arc::ptr_eq(now, expected))
            }) {
                return;
            }
            let old = slot.replace(entry);
            drop(slot);
            drop(old);
        }
    }
    fn retain(&self, bytes: &[u8], response: &Response, expected: Option<&Arc<Entry>>) {
        if !Self::eligible(bytes, response) {
            return;
        }
        let Response::Result(result) = response else {
            return;
        };
        let Some(size) = result
            .heap()
            .and_then(|heap| Self::base_size(bytes.len())?.checked_add(heap))
        else {
            return;
        };
        if expected.is_none() {
            let old = match self.entry.try_lock() {
                Ok(mut slot) => slot.take(),
                Err(_) => return,
            };
            drop(old);
        }
        let Some(reservation) = self.budget.reserve(size) else {
            return;
        };
        let response = response.clone();
        let Response::Result(retained) = &response else {
            return;
        };
        if retained.heap().is_none_or(|heap| {
            Self::base_size(bytes.len())
                .and_then(|base| base.checked_add(heap))
                .is_none_or(|used| used > size)
        }) {
            return;
        }
        let entry = Arc::new(Entry {
            bytes: bytes.into(),
            response: Some(response),
            promoting: AtomicBool::new(false),
            _reservation: reservation,
        });
        self.publish(entry, expected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::write_frame;
    use serde_json::{Value, json};
    use std::io::Cursor;

    fn response(currentness: &str, body: Value) -> io::Result<Response> {
        let mut document = json!({"response":"result","value":{
            "job":{"version":1,"id":"owned","project_dir":"/workspace","args":["--lib"],
                "state":"completed","submitted_at_ms":1,"started_at_ms":2,"finished_at_ms":3,
                "exit_code":0,"input_revision":"revision","currentness":currentness,"error":null},
            "result":null
        }});
        document["value"]["result"] = body;
        serde_json::from_value(document).map_err(io::Error::other)
    }
    fn frame(response: &Response) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, response)?;
        Ok(bytes)
    }
    fn equal(actual: &Response, expected: &Response) -> io::Result<()> {
        assert_eq!(
            serde_json::to_value(actual).map_err(io::Error::other)?,
            serde_json::to_value(expected).map_err(io::Error::other)?
        );
        Ok(())
    }
    #[test]
    fn staged_first_frame_second_promotion_and_third_independent_graph() -> io::Result<()> {
        let cache = Cache::staged();
        let original = response("current", json!({"deep":[{"proof":"verified"}]}))?;
        let bytes = frame(&original)?;
        equal(&cache.read(&mut Cursor::new(&bytes))?, &original)?;
        let candidate = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("missing candidate"))?;
        assert!(candidate.response.is_none());
        assert_eq!(
            cache.budget.0.load(Ordering::Acquire),
            Cache::base_size(bytes.len() - 4).ok_or_else(|| io::Error::other("size"))?
        );
        equal(&cache.read(&mut Cursor::new(&bytes))?, &original)?;
        let ready = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("missing promotion"))?;
        assert!(ready.response.is_some());
        assert!(!Arc::ptr_eq(&candidate, &ready));
        let charge = cache.budget.0.load(Ordering::Acquire);
        drop(candidate);
        assert!(cache.budget.0.load(Ordering::Acquire) < charge);
        let mut returned = cache.read(&mut Cursor::new(&bytes))?;
        if let Response::Result(value) = &mut returned {
            value.result = Some(json!({"caller":"modified"}));
        }
        equal(&cache.read(&mut Cursor::new(&bytes))?, &original)?;
        assert!(Arc::ptr_eq(
            &ready,
            &cache
                .snapshot()
                .ok_or_else(|| io::Error::other("missing ready"))?
        ));
        Ok(())
    }
    #[test]
    fn overlapping_cold_requests_cannot_promote_a_newer_candidate() -> io::Result<()> {
        let cache = Cache::staged();
        let original = response("current", json!({"proof":"whole"}))?;
        let bytes = frame(&original)?;
        // All requests entered before any network completion. Taking an entry
        // after read_exact would incorrectly promote during this first wave.
        let admissions: Vec<_> = (0..4).map(|_| cache.begin()).collect();
        std::thread::scope(|scope| -> io::Result<()> {
            let handles: Vec<_> = admissions
                .into_iter()
                .map(|admission| {
                    let cache = &cache;
                    let bytes = &bytes;
                    scope.spawn(move || cache.read_at(&mut Cursor::new(bytes), admission))
                })
                .collect();
            for handle in handles {
                equal(
                    &handle
                        .join()
                        .map_err(|_| io::Error::other("reader failed"))??,
                    &original,
                )?;
            }
            Ok(())
        })?;
        assert!(
            cache
                .snapshot()
                .is_some_and(|entry| entry.response.is_none())
        );
        // Four second-wave admissions are from the same generation. Only one
        // obtains its nonblocking promotion claim; others return full parses.
        let admissions: Vec<_> = (0..4).map(|_| cache.begin()).collect();
        let prior = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("candidate missing"))?;
        for admission in admissions {
            equal(
                &cache.read_at(&mut Cursor::new(&bytes), admission)?,
                &original,
            )?;
        }
        assert!(!prior.promoting.load(Ordering::Acquire));
        assert!(
            cache
                .snapshot()
                .is_some_and(|entry| entry.response.is_some())
        );
        Ok(())
    }
    #[test]
    fn public_default_shared_family_captures_cold_admission_before_network_completion()
    -> io::Result<()> {
        use std::os::unix::net::UnixListener;
        use std::time::{Duration, Instant};
        let root = std::path::Path::new("/tmp").join(format!("sail-stage-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let socket = root.join("rpc.sock");
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        let expected = response("current", json!({"proof":"whole-result"}))?;
        let reply = frame(&expected)?;
        let client = super::super::Client::new(&socket);
        let result = std::thread::scope(|scope| -> io::Result<()> {
            let server = scope.spawn(|| -> io::Result<()> {
                for _ in 0..2 {
                    let mut streams = Vec::new();
                    for _ in 0..4 {
                        let started = Instant::now();
                        let mut stream = loop {
                            match listener.accept() {
                                Ok((stream, _)) => break stream,
                                Err(error)
                                    if error.kind() == io::ErrorKind::WouldBlock
                                        && started.elapsed() < Duration::from_secs(2) =>
                                {
                                    std::thread::sleep(Duration::from_millis(1));
                                }
                                Err(error) => return Err(error),
                            }
                        };
                        stream.set_nonblocking(false)?;
                        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                        let _: super::super::Request = super::super::read_frame(&mut stream)?;
                        streams.push(stream);
                    }
                    // No reply until all four public calls entered and sent.
                    for mut stream in streams {
                        std::io::Write::write_all(&mut stream, &reply)?;
                    }
                }
                Ok(())
            });
            for wave in 0..2 {
                std::thread::scope(|inner| -> io::Result<()> {
                    let calls: Vec<_> = (0..4)
                        .map(|_| {
                            let caller = client.clone();
                            inner.spawn(move || {
                                caller.call(&super::super::Request::Result { id: "owned".into() })
                            })
                        })
                        .collect();
                    for call in calls {
                        equal(
                            &call
                                .join()
                                .map_err(|_| io::Error::other("caller failed"))??,
                            &expected,
                        )?;
                    }
                    Ok(())
                })?;
                let cache = client
                    .decoded
                    .as_ref()
                    .ok_or_else(|| io::Error::other("cache missing"))?;
                assert_eq!(
                    cache
                        .snapshot()
                        .is_some_and(|entry| entry.response.is_some()),
                    wave == 1
                );
            }
            server
                .join()
                .map_err(|_| io::Error::other("server failed"))??;
            Ok(())
        });
        drop(listener);
        std::fs::remove_dir_all(root)?;
        result
    }
    #[test]
    fn changed_staged_frames_reset_and_late_promotion_cannot_replace_new_generation()
    -> io::Result<()> {
        let cache = Cache::staged();
        let original = response("current", json!({"proof":"old"}))?;
        let changed = response("historical", json!({"proof":"new"}))?;
        let first = frame(&original)?;
        let next = frame(&changed)?;
        cache.read(&mut Cursor::new(&first))?;
        let delayed = cache.begin();
        cache.read(&mut Cursor::new(&next))?;
        let now = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("candidate missing"))?;
        equal(
            &cache.read_at(&mut Cursor::new(&first), delayed)?,
            &original,
        )?;
        assert!(Arc::ptr_eq(
            &now,
            &cache
                .snapshot()
                .ok_or_else(|| io::Error::other("generation missing"))?
        ));
        assert!(now.response.is_none());
        equal(&cache.read(&mut Cursor::new(&next))?, &changed)?;
        assert!(
            cache
                .snapshot()
                .is_some_and(|entry| entry.response.is_some())
        );
        Ok(())
    }
    #[test]
    fn staged_promotion_refusal_and_inflight_candidates_remain_accounted() -> io::Result<()> {
        let cache = Cache::staged();
        let original = response("current", json!({"proof":"complete"}))?;
        let bytes = frame(&original)?;
        cache.read(&mut Cursor::new(&bytes))?;
        let candidate = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("candidate missing"))?;
        let charge = cache.budget.0.load(Ordering::Acquire);
        let held = cache
            .budget
            .reserve(OWNED_LIMIT - charge)
            .ok_or_else(|| io::Error::other("reserve"))?;
        equal(&cache.read(&mut Cursor::new(&bytes))?, &original)?;
        assert!(
            cache
                .snapshot()
                .is_some_and(|entry| entry.response.is_none())
        );
        assert_eq!(cache.budget.0.load(Ordering::Acquire), OWNED_LIMIT);
        drop(held);
        // This later fresh RPC can promote after resource pressure clears;
        // there was no wait/retry in the earlier refused call.
        assert!(!candidate.promoting.load(Ordering::Acquire));
        equal(&cache.read(&mut Cursor::new(&bytes))?, &original)?;
        let ready = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("promotion missing"))?;
        assert!(ready.response.is_some());
        let used = cache.budget.0.load(Ordering::Acquire);
        drop(candidate);
        assert_eq!(cache.budget.0.load(Ordering::Acquire), used - charge);
        Ok(())
    }
    #[test]
    fn identical_full_frames_reuse_decoding_but_return_independent_whole_values() -> io::Result<()>
    {
        let cache = Cache::default();
        let original = response(
            "current",
            json!({"nested":[{"proof":"verified","notes":["exact"]}]}),
        )?;
        let bytes = frame(&original)?;
        let first = cache.read(&mut Cursor::new(&bytes))?;
        equal(&first, &original)?;
        let established = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("not retained"))?;
        let mut second = cache.read(&mut Cursor::new(&bytes))?;
        assert!(Arc::ptr_eq(
            &established,
            &cache
                .snapshot()
                .ok_or_else(|| io::Error::other("not retained"))?
        ));
        if let Response::Result(value) = &mut second {
            value.result = Some(json!({"changed":"caller-owned"}));
        }
        equal(&cache.read(&mut Cursor::new(&bytes))?, &original)?;
        Ok(())
    }
    #[test]
    fn changed_metadata_currentness_error_content_and_wire_order_parse_the_fresh_reply()
    -> io::Result<()> {
        for cache in [Cache::default(), Cache::staged()] {
            let original = response("current", json!({"a":1,"b":2}))?;
            cache.read(&mut Cursor::new(frame(&original)?))?;
            cache.read(&mut Cursor::new(frame(&original)?))?;
            for currentness in ["historical", "unknown", "current"] {
                let changed = response(currentness, json!({"a":2,"b":1}))?;
                let prior = cache.snapshot();
                equal(&cache.read(&mut Cursor::new(frame(&changed)?))?, &changed)?;
                if let (Some(prior), Some(now)) = (prior, cache.snapshot()) {
                    assert!(!Arc::ptr_eq(&prior, &now));
                }
            }
            let mut changed = original.clone();
            if let Response::Result(value) = &mut changed {
                value.job.args.push("--exact-new-argument".into());
                value.job.error = Some("SAIL0000: new failure".into());
                value.job.finished_at_ms = Some(99);
                value.job.input_revision = Some("different-input-revision".into());
            }
            equal(&cache.read(&mut Cursor::new(frame(&changed)?))?, &changed)?;
            let first = frame(&original)?;
            cache.read(&mut Cursor::new(&first))?;
            let prior = cache
                .snapshot()
                .ok_or_else(|| io::Error::other("not retained"))?;
            let text = String::from_utf8(first[4..].to_vec())
                .map_err(io::Error::other)?
                .replace("\"a\":1,\"b\":2", "\"b\":2,\"a\":1");
            assert_ne!(text.as_bytes(), &first[4..]);
            let mut reordered = u32::try_from(text.len())
                .map_err(io::Error::other)?
                .to_be_bytes()
                .to_vec();
            reordered.extend_from_slice(text.as_bytes());
            equal(&cache.read(&mut Cursor::new(reordered))?, &original)?;
            assert!(!Arc::ptr_eq(
                &prior,
                &cache
                    .snapshot()
                    .ok_or_else(|| io::Error::other("not retained"))?
            ));
        }
        Ok(())
    }
    #[test]
    fn truncation_oversize_versions_duplicates_and_trailing_data_never_use_cached_success()
    -> io::Result<()> {
        for cache in [Cache::default(), Cache::staged()] {
            let bytes = frame(&response("current", json!({"proof":"verified"}))?)?;
            cache.read(&mut Cursor::new(&bytes))?;
            cache.read(&mut Cursor::new(&bytes))?;
            assert!(
                cache
                    .read(&mut Cursor::new(&bytes[..bytes.len() - 1]))
                    .is_err()
            );
            let mut missing = bytes.clone();
            missing[..4].copy_from_slice(
                &u32::try_from(bytes.len() - 3)
                    .map_err(io::Error::other)?
                    .to_be_bytes(),
            );
            assert!(cache.read(&mut Cursor::new(missing)).is_err());
            let oversized = u32::try_from(MAX_FRAME + 1)
                .map_err(io::Error::other)?
                .to_be_bytes();
            assert!(cache.read(&mut Cursor::new(oversized)).is_err());
            for payload in [
                br#"{"version":2,"body":{"response":"error","value":"wrong-version"}}"#.as_slice(),
                br#"{"version":1,"version":1,"body":{"response":"error","value":"duplicate"}}"#,
                br#"{"version":1,"body":{"response":"error","value":"trailing"}}x"#,
                br#"{"version":1,"body":{"response":"not-a-known-response","value":{}}}"#,
                br#"{"body":{"response":"error","value":"missing-version"}}"#,
            ] {
                let mut invalid = u32::try_from(payload.len())
                    .map_err(io::Error::other)?
                    .to_be_bytes()
                    .to_vec();
                invalid.extend_from_slice(payload);
                assert!(cache.read(&mut Cursor::new(invalid)).is_err());
            }
            let mut pair = bytes.clone();
            pair.extend_from_slice(&bytes);
            let mut reader = Cursor::new(pair);
            cache.read(&mut reader)?;
            assert_eq!(reader.position(), bytes.len() as u64);
            cache.read(&mut reader)?;
        }
        Ok(())
    }
    #[test]
    fn in_flight_entries_remain_charged_and_overbudget_or_large_results_stay_correct()
    -> io::Result<()> {
        let cache = Cache::default();
        let original = response("current", json!({"proof":"verified"}))?;
        cache.read(&mut Cursor::new(frame(&original)?))?;
        let held = cache
            .snapshot()
            .ok_or_else(|| io::Error::other("not retained"))?;
        let occupied = cache.budget.0.load(Ordering::Acquire);
        let reservation = cache
            .budget
            .reserve(OWNED_LIMIT - occupied)
            .ok_or_else(|| io::Error::other("reservation refused"))?;
        let changed = response("unknown", json!({"proof":"unavailable"}))?;
        equal(&cache.read(&mut Cursor::new(frame(&changed)?))?, &changed)?;
        assert!(cache.snapshot().is_none());
        assert_eq!(cache.budget.0.load(Ordering::Acquire), OWNED_LIMIT);
        drop(held);
        assert_eq!(
            cache.budget.0.load(Ordering::Acquire),
            OWNED_LIMIT - occupied
        );
        drop(reservation);
        assert_eq!(cache.budget.0.load(Ordering::Acquire), 0);
        let large = response("current", json!({"payload":"x".repeat(WIRE_LIMIT)}))?;
        equal(&cache.read(&mut Cursor::new(frame(&large)?))?, &large)?;
        assert!(cache.snapshot().is_none());
        Ok(())
    }
    #[test]
    fn unaudited_layout_decodes_correctly_without_retaining() -> io::Result<()> {
        let cache = Cache::default();
        let original = response("current", json!({"proof":"whole-graph"}))?;
        equal(&cache.read(&mut Cursor::new(frame(&original)?))?, &original)?;
        assert_eq!(
            cache.snapshot().is_some(),
            super::super::ownership::audited_layout()
        );
        Ok(())
    }
    #[test]
    fn constructors_select_staged_immediate_and_disabled_policies() {
        let client = super::super::Client::new("/private/rpc.sock");
        assert!(client.decoded.as_ref().is_some_and(|cache| cache.staged));
        assert!(
            client
                .clone()
                .decoded
                .as_ref()
                .is_some_and(|cache| cache.staged)
        );
        assert!(
            super::super::Client::without_result_decoding_cache("/private/rpc.sock")
                .decoded
                .is_none()
        );
        assert!(
            !super::super::Client::with_result_decoding_cache("/private/rpc.sock")
                .decoded
                .as_ref()
                .is_none_or(|cache| cache.staged)
        );
    }
    #[test]
    fn rebound_endpoints_share_budget_but_read_each_actual_fresh_reply() -> io::Result<()> {
        use std::os::unix::net::UnixListener;
        use std::time::{Duration, Instant};
        let root = std::path::Path::new("/tmp").join(format!("sail-rebind-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let first_socket = root.join("first.sock");
        let second_socket = root.join("second.sock");
        let first_listener = UnixListener::bind(&first_socket)?;
        let second_listener = UnixListener::bind(&second_socket)?;
        first_listener.set_nonblocking(true)?;
        second_listener.set_nonblocking(true)?;
        let first = super::super::Client::with_result_decoding_cache(&first_socket);
        let second = first.with_socket(&second_socket);
        assert!(Arc::ptr_eq(
            first
                .decoded
                .as_ref()
                .ok_or_else(|| io::Error::other("missing first cache"))?,
            second
                .decoded
                .as_ref()
                .ok_or_else(|| io::Error::other("missing second cache"))?
        ));
        let expected_first = response("current", json!({"endpoint":"first"}))?;
        let expected_second = response("historical", json!({"endpoint":"second"}))?;
        let replies = [frame(&expected_first)?, frame(&expected_second)?];
        let checked = std::thread::scope(|scope| -> io::Result<()> {
            let servers: Vec<_> = [first_listener, second_listener]
                .into_iter()
                .zip(replies)
                .map(|(listener, reply)| {
                    scope.spawn(move || -> io::Result<()> {
                        let started = Instant::now();
                        let mut stream = loop {
                            match listener.accept() {
                                Ok((stream, _)) => break stream,
                                Err(error)
                                    if error.kind() == io::ErrorKind::WouldBlock
                                        && started.elapsed() < Duration::from_secs(2) =>
                                {
                                    std::thread::sleep(Duration::from_millis(1));
                                }
                                Err(error) => return Err(error),
                            }
                        };
                        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                        let _: super::super::Request = super::super::read_frame(&mut stream)?;
                        std::io::Write::write_all(&mut stream, &reply)
                    })
                })
                .collect();
            let request = super::super::Request::Result { id: "owned".into() };
            equal(&first.call(&request)?, &expected_first)?;
            equal(&second.call(&request)?, &expected_second)?;
            for server in servers {
                server
                    .join()
                    .map_err(|_| io::Error::other("server failed"))??;
            }
            Ok(())
        });
        std::fs::remove_dir_all(root)?;
        checked
    }
    #[test]
    fn client_debug_never_contains_cached_frame_or_model_contents() -> io::Result<()> {
        let client = super::super::Client::with_result_decoding_cache("/private/rpc.sock");
        let original = response(
            "current",
            json!({"model":"sensitive-counterexample-content"}),
        )?;
        client
            .decoded
            .as_ref()
            .ok_or_else(|| io::Error::other("cache missing"))?
            .read(&mut Cursor::new(frame(&original)?))?;
        let printed = format!("{client:?}");
        assert!(!printed.contains("sensitive-counterexample-content"));
        assert!(!printed.contains("counterexample"));
        assert!(printed.len() < 256);
        Ok(())
    }
    #[test]
    fn simultaneous_distinct_fresh_frames_do_not_cross_reply_ownership() -> io::Result<()> {
        let cache = Cache::default();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|thread| {
                    let cache = &cache;
                    scope.spawn(move || -> io::Result<()> {
                        for index in 0..12 {
                            let expected = response(
                                if index % 2 == 0 { "current" } else { "unknown" },
                                json!({"thread":thread,"index":index}),
                            )?;
                            let bytes = frame(&expected)?;
                            equal(&cache.read(&mut Cursor::new(&bytes))?, &expected)?;
                            equal(&cache.read(&mut Cursor::new(&bytes))?, &expected)?;
                        }
                        Ok(())
                    })
                })
                .collect();
            for handle in handles {
                handle
                    .join()
                    .map_err(|_| io::Error::other("reader failed"))??;
            }
            Ok(())
        })
    }
}

//! Serving policy shared by every place that decides whether a cached
//! answer may be used: the shim when it serves, the daemon when it reads
//! the shared tier, and `sail cache publish` when it exports entries.

use crate::protocol::Effort;

/// Whether a cached `check-sat` answer of `unknown` with this recorded
/// effort may be served (ADR 0009, owner decision 2026-10-08): only when
/// the solver gave up for a reason other than resource exhaustion, and used
/// at most half of a non-zero limit. Anything near the limit, cancelled, or
/// without recorded effort is re-solved.
#[must_use]
pub fn unknown_servable(effort: Option<&Effort>) -> bool {
    let Some(e) = effort else { return false };
    let (Some(reason), Some(used), Some(limit)) = (e.reason.as_deref(), e.used, e.limit) else {
        return false;
    };
    let lower = reason.to_ascii_lowercase();
    let exhausted = [
        "canceled",
        "cancelled",
        "resource",
        "rlimit",
        "timeout",
        "memout",
        "memory",
    ]
    .iter()
    .any(|w| lower.contains(w));
    !exhausted && limit > 0 && used <= limit / 2
}

/// Whether `reply` is a `check-sat` answer of `unknown`.
#[must_use]
pub fn is_unknown(reply: &[u8]) -> bool {
    reply.trim_ascii() == b"unknown"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effort(reason: &str, used: u64, limit: u64) -> Effort {
        Effort {
            limit: Some(limit),
            used: Some(used),
            reason: Some(format!("(:reason-unknown \"{reason}\")")),
        }
    }

    #[test]
    fn only_far_from_limit_incomplete_unknowns_are_servable() {
        let ok = |reason: &str, used, limit| unknown_servable(Some(&effort(reason, used, limit)));
        assert!(ok("incomplete quantifiers", 10, 100));
        assert!(ok("incomplete quantifiers", 50, 100));
        assert!(!ok("incomplete quantifiers", 51, 100));
        assert!(ok("incomplete", u64::MAX / 2, u64::MAX));
        assert!(!ok("incomplete", u64::MAX / 2 + 1, u64::MAX));
        assert!(!ok("incomplete", u64::MAX, u64::MAX));
        assert!(!ok("canceled", 1, 100));
        assert!(!ok("max. resource limit exceeded", 1, 100));
        assert!(!ok("incomplete", 0, 0));
        assert!(!unknown_servable(None));
        assert!(!unknown_servable(Some(&Effort::default())));
        assert!(is_unknown(b"unknown\n"));
        assert!(!is_unknown(b"unsat\n"));
    }
}

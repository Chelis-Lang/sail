//! Actual SHA-backed independent delivery controls.
use sail_acceptance::query_delivery::InputPipe;
use sail_key::{Mode, SessionPrefix, SolverIdentity};

#[derive(Default)]
struct Pipe {
    bytes: Vec<u8>,
    closed: bool,
    fail: bool,
}
impl InputPipe for Pipe {
    fn write(&mut self, bytes: &[u8]) -> bool {
        if self.fail {
            self.fail = false;
            self.bytes
                .extend_from_slice(bytes.get(..1).unwrap_or_default());
            return false;
        }
        self.bytes.extend_from_slice(bytes);
        true
    }
    fn close(&mut self) {
        self.closed = true;
    }
}
fn identity() -> SolverIdentity {
    SolverIdentity {
        name: "z3".into(),
        version: "pinned".into(),
        binary_sha256: [3; 32],
        platform: "native".into(),
    }
}
fn session(bytes: &[u8], mode: &Mode, solver: &SolverIdentity) -> SessionPrefix {
    let mut session = SessionPrefix::new(solver, mode);
    session.append(bytes);
    assert!(session.hash_tail());
    session
}
#[test]
fn exact_delivered_bytes_accept_independent_of_physical_write_fragments() {
    let parsed = session(b"(check-sat)\n", &Mode::Default, &identity());
    let mut delivered = parsed.delivery(Pipe::default());
    assert!(delivered.write(b"(check-"));
    assert!(delivered.write(b"sat)\n"));
    assert!(delivered.matches(parsed.silence_key(), true));
    assert!(!delivered.matches(parsed.silence_key(), false));
    delivered.close();
    assert!(!delivered.matches(parsed.silence_key(), true));
    assert!(!delivered.write(b"later"));
}
#[test]
fn omitted_reordered_or_extra_delivered_bytes_withhold_publication() {
    let parsed = session(b"(echo 1)\n(echo 2)\n", &Mode::Default, &identity());
    for bytes in [
        b"(echo 2)\n(echo 1)\n".as_slice(),
        b"(echo 1)\n",
        b"(echo 1)\n(echo 2)\n ",
    ] {
        let mut delivered = parsed.delivery(Pipe::default());
        assert!(delivered.write(bytes));
        assert!(!delivered.matches(parsed.silence_key(), true));
    }
}
#[test]
fn solver_and_mode_header_are_not_reconstructed_from_the_terminal_key() {
    let identity = identity();
    let parsed = session(b"x", &Mode::Default, &identity);
    let mut changed = identity.clone();
    changed.platform = "other".into();
    for other in [
        session(b"x", &Mode::OptIn("accept-first-proof".into()), &identity),
        session(b"x", &Mode::Default, &changed),
    ] {
        let mut delivered = other.delivery(Pipe::default());
        assert!(delivered.write(b"x"));
        assert!(!delivered.matches(parsed.silence_key(), true));
    }
}
#[test]
fn failed_partial_delivery_cannot_be_healed_by_later_success() {
    let parsed = session(b"later", &Mode::Default, &identity());
    let mut delivered = parsed.delivery(Pipe {
        fail: true,
        ..Pipe::default()
    });
    assert!(!delivered.write(b"bad"));
    assert!(delivered.write(b"later"));
    assert!(!delivered.active());
    assert!(!delivered.matches(parsed.silence_key(), true));
}

#[test]
fn sealed_delivery_is_owned_once_and_cannot_be_modified_by_later_writes() {
    let parsed = session(b"(check-sat)\n", &Mode::Default, &identity());
    let mut delivery = parsed.delivery(Pipe::default());
    assert!(delivery.write(b"(check-sat)\n"));
    let token = delivery.seal(parsed.silence_key(), true);
    assert!(token.is_some());
    let Some(token) = token else {
        return;
    };
    assert_eq!(token.key(), parsed.silence_key());
    assert!(!delivery.active());
    assert!(!delivery.write(b"later"));
    assert!(delivery.seal(parsed.silence_key(), true).is_none());
    assert_eq!(token.key(), parsed.silence_key());
    delivery.close();
}

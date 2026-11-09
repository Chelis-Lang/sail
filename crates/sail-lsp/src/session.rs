//! The LSP lifecycle, as a state machine without I/O.
//!
//! [`Session::handle`] takes one message body and returns the replies to
//! write and what the driver should do. The server offers no language
//! features: it declares save notifications so the editor tells it about
//! saves, and pushes diagnostics on its own schedule.

use serde_json::{Value, json};

use crate::jsonrpc::{self, Message};
use crate::uri;

/// What the driver should do after a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Nothing.
    None,
    /// `initialize` was answered. `root` is the workspace folder the client
    /// named, when it named a local one.
    Initialized {
        /// The first workspace folder, `rootUri` or `rootPath`.
        root: Option<String>,
    },
    /// A document was saved: verify now instead of waiting for the file
    /// watcher.
    Saved {
        /// The saved document's path, when it is a local file.
        path: Option<String>,
    },
    /// `exit`: stop, successfully only if `shutdown` came first.
    Exit {
        /// Whether `shutdown` was requested before `exit`.
        clean: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Fresh,
    Running,
    ShutDown,
}

/// One client connection's lifecycle.
#[derive(Debug)]
pub struct Session {
    state: State,
    version: String,
}

impl Session {
    /// A session awaiting `initialize`. `version` is reported in
    /// `serverInfo`.
    #[must_use]
    pub fn new(version: &str) -> Self {
        Self {
            state: State::Fresh,
            version: version.to_owned(),
        }
    }

    /// Whether `initialize` has been answered and `shutdown` has not.
    #[must_use]
    pub fn running(&self) -> bool {
        self.state == State::Running
    }

    /// Handles one message body.
    pub fn handle(&mut self, body: &[u8]) -> (Vec<Value>, Effect) {
        match jsonrpc::parse(body) {
            Err(response) => (vec![response], Effect::None),
            Ok(Message::Response) => (Vec::new(), Effect::None),
            Ok(Message::Request { id, method, params }) => self.request(&id, &method, &params),
            Ok(Message::Notification { method, params }) => {
                (Vec::new(), self.notification(&method, &params))
            }
        }
    }

    fn request(&mut self, id: &Value, method: &str, params: &Value) -> (Vec<Value>, Effect) {
        match (self.state, method) {
            (State::Fresh, "initialize") => {
                self.state = State::Running;
                let result = json!({
                    "capabilities": {
                        "positionEncoding": "utf-16",
                        "textDocumentSync": {
                            "openClose": true,
                            "change": 0,
                            "save": {"includeText": false},
                        },
                    },
                    "serverInfo": {"name": "sail", "version": self.version},
                });
                (
                    vec![jsonrpc::ok(id, &result)],
                    Effect::Initialized { root: root(params) },
                )
            }
            (State::Fresh, _) => (
                vec![jsonrpc::error(
                    id,
                    jsonrpc::SERVER_NOT_INITIALIZED,
                    "initialize first",
                )],
                Effect::None,
            ),
            (_, "shutdown") => {
                self.state = State::ShutDown;
                (vec![jsonrpc::ok(id, &Value::Null)], Effect::None)
            }
            (State::ShutDown, _) => (
                vec![jsonrpc::error(
                    id,
                    jsonrpc::INVALID_REQUEST,
                    "the server is shut down",
                )],
                Effect::None,
            ),
            (State::Running, _) => (
                vec![jsonrpc::error(
                    id,
                    jsonrpc::METHOD_NOT_FOUND,
                    &format!("sail lsp publishes diagnostics only; `{method}` is not supported"),
                )],
                Effect::None,
            ),
        }
    }

    fn notification(&mut self, method: &str, params: &Value) -> Effect {
        match (self.state, method) {
            (_, "exit") => Effect::Exit {
                clean: self.state == State::ShutDown,
            },
            (State::Running, "textDocument/didSave") => Effect::Saved {
                path: params["textDocument"]["uri"]
                    .as_str()
                    .and_then(uri::to_path),
            },
            _ => Effect::None,
        }
    }
}

fn root(params: &Value) -> Option<String> {
    params["workspaceFolders"][0]["uri"]
        .as_str()
        .or_else(|| params["rootUri"].as_str())
        .and_then(uri::to_path)
        .or_else(|| params["rootPath"].as_str().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send(session: &mut Session, message: &Value) -> (Vec<Value>, Effect) {
        session.handle(message.to_string().as_bytes())
    }

    #[test]
    fn lifecycle_from_initialize_to_exit() {
        let mut session = Session::new("0.0.0");
        let (replies, effect) = send(
            &mut session,
            &json!({"jsonrpc": "2.0", "id": 1, "method": "textDocument/hover", "params": {}}),
        );
        assert_eq!(replies[0]["error"]["code"], jsonrpc::SERVER_NOT_INITIALIZED);
        assert_eq!(effect, Effect::None);
        // Notifications before initialize are dropped, as the protocol says.
        let saved = json!({"jsonrpc": "2.0", "method": "textDocument/didSave",
            "params": {"textDocument": {"uri": "file:///w/a%20b.rs"}}});
        assert_eq!(send(&mut session, &saved), (Vec::new(), Effect::None));

        let (replies, effect) = send(
            &mut session,
            &json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {
                "rootUri": "file:///old",
                "workspaceFolders": [{"uri": "file:///w", "name": "w"}],
            }}),
        );
        assert_eq!(replies[0]["id"], 2);
        assert_eq!(
            replies[0]["result"]["capabilities"]["textDocumentSync"]["save"]["includeText"],
            false
        );
        assert_eq!(
            effect,
            Effect::Initialized {
                root: Some("/w".to_owned())
            }
        );
        assert!(session.running());

        assert_eq!(
            send(&mut session, &saved).1,
            Effect::Saved {
                path: Some("/w/a b.rs".to_owned())
            }
        );
        let (replies, _) = send(
            &mut session,
            &json!({"jsonrpc": "2.0", "id": 3, "method": "textDocument/hover", "params": {}}),
        );
        assert_eq!(replies[0]["error"]["code"], jsonrpc::METHOD_NOT_FOUND);
        // Unknown notifications, `$/` ones included, are ignored.
        let unknown = json!({"jsonrpc": "2.0", "method": "$/setTrace", "params": {"value": "off"}});
        assert_eq!(send(&mut session, &unknown), (Vec::new(), Effect::None));

        let (replies, effect) = send(
            &mut session,
            &json!({"jsonrpc": "2.0", "id": 4, "method": "shutdown"}),
        );
        assert_eq!(replies[0]["result"], Value::Null);
        assert_eq!(effect, Effect::None);
        assert!(!session.running());
        assert_eq!(send(&mut session, &saved).1, Effect::None);
        let exit = json!({"jsonrpc": "2.0", "method": "exit"});
        assert_eq!(send(&mut session, &exit).1, Effect::Exit { clean: true });
    }

    #[test]
    fn exit_without_shutdown_is_not_clean_and_roots_fall_back() {
        let mut session = Session::new("0.0.0");
        let exit = json!({"jsonrpc": "2.0", "method": "exit"});
        assert_eq!(send(&mut session, &exit).1, Effect::Exit { clean: false });
        for (params, expected) in [
            (json!({"rootUri": "file:///r"}), Some("/r")),
            (json!({"rootUri": null, "rootPath": "/p"}), Some("/p")),
            (json!({"rootUri": "untitled:x"}), None),
            (json!({}), None),
        ] {
            assert_eq!(root(&params).as_deref(), expected);
        }
        let (replies, effect) = session.handle(b"{");
        assert_eq!(replies[0]["error"]["code"], jsonrpc::PARSE_ERROR);
        assert_eq!(effect, Effect::None);
    }
}

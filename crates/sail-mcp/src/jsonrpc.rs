//! JSON-RPC 2.0 framing for the MCP stdio transport.
//!
//! Each message is one line of UTF-8 JSON, terminated by `\n`, with no
//! embedded newlines. [`parse`] turns a line into a [`Message`] or into the
//! error response the peer must receive; [`ok`] and [`error`] build
//! responses. Batches (JSON arrays) are not part of MCP and are rejected.

use serde_json::{Map, Value, json};

/// Invalid JSON.
pub const PARSE_ERROR: i64 = -32700;
/// Valid JSON that is not a JSON-RPC request or notification.
pub const INVALID_REQUEST: i64 = -32600;
/// The method does not exist.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Invalid method parameters (including an unknown tool).
pub const INVALID_PARAMS: i64 = -32602;
/// An internal error.
pub const INTERNAL_ERROR: i64 = -32603;
/// MCP's `UnsupportedProtocolVersionError` (revision 2026-07-28).
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// One incoming message.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A request: it must be answered with the same `id`.
    Request {
        /// The request id (a string or a number).
        id: Value,
        /// The method name.
        method: String,
        /// The parameters (an empty object when absent).
        params: Map<String, Value>,
    },
    /// A notification: never answered.
    Notification {
        /// The method name.
        method: String,
        /// The parameters (an empty object when absent).
        params: Map<String, Value>,
    },
    /// A response to a request this side sent. The server sends none, so
    /// these are ignored.
    Response,
}

/// A successful response.
#[must_use]
pub fn ok(id: &Value, result: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// A successful owned response without another structured-result clone.
#[must_use]
pub(super) fn ok_owned(id: Value, result: Value) -> Value {
    Value::Object(Map::from_iter([
        ("jsonrpc".to_owned(), Value::String("2.0".to_owned())),
        ("id".to_owned(), id),
        ("result".to_owned(), result),
    ]))
}

/// An error response. `data` is omitted when `None`.
#[must_use]
pub fn error(id: &Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut err = json!({"code": code, "message": message});
    if let (Some(data), Some(obj)) = (data, err.as_object_mut()) {
        obj.insert("data".to_owned(), data);
    }
    json!({"jsonrpc": "2.0", "id": id, "error": err})
}

/// Serializes a message as one line, without the trailing newline.
/// `serde_json` escapes every newline inside strings, so the result never
/// contains one.
#[must_use]
pub fn line(message: &Value) -> String {
    serde_json::to_string(message).unwrap_or_default()
}

/// Parses one line. `Err` holds the error response to send back.
///
/// # Errors
///
/// A parse error for invalid JSON or UTF-8, and an invalid-request error for
/// anything that is not a well-formed JSON-RPC 2.0 message (including
/// batches, a missing or non-`"2.0"` `jsonrpc`, an id that is not a string
/// or number, and parameters that are not an object).
pub fn parse(line: &[u8]) -> Result<Message, Value> {
    let Ok(text) = std::str::from_utf8(line) else {
        return Err(error(
            &Value::Null,
            PARSE_ERROR,
            "Parse error: not UTF-8",
            None,
        ));
    };
    let value: Value = serde_json::from_str(text).map_err(|e| {
        error(
            &Value::Null,
            PARSE_ERROR,
            &format!("Parse error: {e}"),
            None,
        )
    })?;
    let Value::Object(mut obj) = value else {
        let what = if value.is_array() {
            "batches are not supported"
        } else {
            "a message must be a JSON object"
        };
        return Err(error(
            &Value::Null,
            INVALID_REQUEST,
            &format!("Invalid Request: {what}"),
            None,
        ));
    };
    let id = obj.remove("id");
    let reply_id = id
        .clone()
        .filter(|i| i.is_string() || i.is_number())
        .unwrap_or(Value::Null);
    let invalid = |why: &str| {
        error(
            &reply_id,
            INVALID_REQUEST,
            &format!("Invalid Request: {why}"),
            None,
        )
    };
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(invalid("`jsonrpc` must be \"2.0\""));
    }
    let Some(method) = obj.remove("method") else {
        if id.is_some() && (obj.contains_key("result") || obj.contains_key("error")) {
            return Ok(Message::Response);
        }
        return Err(invalid("missing `method`"));
    };
    let Value::String(method) = method else {
        return Err(invalid("`method` must be a string"));
    };
    let params = match obj.remove("params") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(p)) => p,
        Some(_) => return Err(invalid("`params` must be an object")),
    };
    match id {
        None => Ok(Message::Notification { method, params }),
        Some(id @ (Value::String(_) | Value::Number(_))) => {
            Ok(Message::Request { id, method, params })
        }
        Some(_) => Err(invalid("`id` must be a string or a number")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(v: &Value) -> Option<i64> {
        v["error"]["code"].as_i64()
    }

    #[test]
    fn requests_and_notifications() {
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#),
            Ok(Message::Request {
                id: json!(1),
                method: "tools/list".to_owned(),
                params: Map::new()
            })
        );
        let Ok(Message::Request { id, params, .. }) =
            parse(br#"{"jsonrpc":"2.0","id":"a","method":"x","params":{"k":2}}"#)
        else {
            unreachable!("not a request")
        };
        assert_eq!((id, params.get("k")), (json!("a"), Some(&json!(2))));
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Ok(Message::Notification {
                method: "notifications/initialized".to_owned(),
                params: Map::new()
            })
        );
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","id":4,"result":{}}"#),
            Ok(Message::Response)
        );
    }

    #[test]
    fn malformed_input_gets_the_right_error() {
        let e = parse(b"{not json").err().unwrap_or_default();
        assert_eq!((code(&e), &e["id"]), (Some(PARSE_ERROR), &Value::Null));
        let e = parse(b"\xff\xfe").err().unwrap_or_default();
        assert_eq!(code(&e), Some(PARSE_ERROR));
        for bad in [
            &br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#[..],
            br#""ping""#,
            br#"{"id":1,"method":"ping"}"#,
            br#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#,
            br#"{"jsonrpc":"2.0","id":1}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":7}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[1]}"#,
            br#"{"jsonrpc":"2.0","id":{"x":1},"method":"ping"}"#,
            br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
        ] {
            let e = parse(bad).err().unwrap_or_default();
            assert_eq!(
                code(&e),
                Some(INVALID_REQUEST),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        // The id is echoed when it is usable.
        let e = parse(br#"{"jsonrpc":"2.0","id":9,"method":1}"#)
            .err()
            .unwrap_or_default();
        assert_eq!(e["id"], json!(9));
    }

    #[test]
    fn responses_are_single_lines() {
        let r = ok(&json!(1), &json!({"text": "a\nb"}));
        let l = line(&r);
        assert!(!l.contains('\n'));
        assert_eq!(serde_json::from_str::<Value>(&l).ok(), Some(r));
        let e = error(&json!("x"), INVALID_PARAMS, "bad", Some(json!({"k": 1})));
        assert_eq!(e["error"]["data"]["k"], json!(1));
        assert!(
            error(&json!(1), INTERNAL_ERROR, "m", None)["error"]
                .get("data")
                .is_none()
        );
    }
}

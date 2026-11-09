//! JSON-RPC 2.0 over the LSP base protocol.
//!
//! Each message is a header block (`Content-Length: N`, optionally
//! `Content-Type`), a blank line, then `N` bytes of UTF-8 JSON. [`read`]
//! returns one message body, [`parse`] classifies it, and [`write()`] frames a
//! value. Batches are not part of LSP and are rejected.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

/// Invalid JSON.
pub const PARSE_ERROR: i64 = -32700;
/// Valid JSON that is not a JSON-RPC message.
pub const INVALID_REQUEST: i64 = -32600;
/// The method does not exist.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// A request arrived before `initialize`.
pub const SERVER_NOT_INITIALIZED: i64 = -32002;

/// The largest body accepted. This server only reads lifecycle messages and
/// document notifications without text, so anything larger is a framing error.
pub const MAX_BODY: usize = 64 << 20;

/// One incoming message.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A request: it must be answered with the same `id`.
    Request {
        /// The request id (a string or a number).
        id: Value,
        /// The method name.
        method: String,
        /// The parameters (`null` when absent).
        params: Value,
    },
    /// A notification: never answered.
    Notification {
        /// The method name.
        method: String,
        /// The parameters (`null` when absent).
        params: Value,
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

/// An error response.
#[must_use]
pub fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// A notification.
#[must_use]
pub fn notification(method: &str, params: &Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

/// Reads one framed message body. `Ok(None)` is a clean end of input before
/// any header byte.
///
/// # Errors
///
/// An I/O error, a header block without a valid `Content-Length`, a body
/// larger than [`MAX_BODY`], or input that ends inside a message.
pub fn read(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    let mut first = true;
    loop {
        let mut line = Vec::new();
        if input.read_until(b'\n', &mut line)? == 0 {
            if first {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "input ended inside a header block",
            ));
        }
        first = false;
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            break;
        }
        if let Some((name, value)) = text.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = length.filter(|length| *length <= MAX_BODY) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing, invalid or oversized Content-Length",
        ));
    };
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Writes one framed message and flushes.
///
/// # Errors
///
/// An I/O error from the writer.
pub fn write(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

/// Classifies one message body. `Err` holds the error response to send back.
///
/// # Errors
///
/// A parse error for invalid JSON, and an invalid-request error for anything
/// that is not a well-formed JSON-RPC 2.0 message.
pub fn parse(body: &[u8]) -> Result<Message, Value> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| error(&Value::Null, PARSE_ERROR, &format!("Parse error: {e}")))?;
    let Value::Object(mut object) = value else {
        return Err(error(
            &Value::Null,
            INVALID_REQUEST,
            "Invalid Request: a message must be a JSON object",
        ));
    };
    let id = object.remove("id");
    let reply = id
        .clone()
        .filter(|id| id.is_string() || id.is_number())
        .unwrap_or(Value::Null);
    let invalid = |why: &str| error(&reply, INVALID_REQUEST, &format!("Invalid Request: {why}"));
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(invalid("`jsonrpc` must be \"2.0\""));
    }
    let Some(method) = object.remove("method") else {
        if id.is_some() && (object.contains_key("result") || object.contains_key("error")) {
            return Ok(Message::Response);
        }
        return Err(invalid("missing `method`"));
    };
    let Value::String(method) = method else {
        return Err(invalid("`method` must be a string"));
    };
    let params = object.remove("params").unwrap_or(Value::Null);
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

    #[test]
    fn frames_round_trip_and_tolerate_extra_headers() -> io::Result<()> {
        let mut bytes = Vec::new();
        write(&mut bytes, &json!({"jsonrpc": "2.0", "method": "exit"}))?;
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
        bytes.extend_from_slice(
            format!(
                "content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH: {}\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        );
        bytes.extend_from_slice(body);
        let mut input = io::Cursor::new(bytes);
        let first = read(&mut input)?.unwrap_or_default();
        assert_eq!(
            parse(&first),
            Ok(Message::Notification {
                method: "exit".to_owned(),
                params: Value::Null
            })
        );
        let second = read(&mut input)?.unwrap_or_default();
        assert_eq!(
            parse(&second),
            Ok(Message::Request {
                id: json!(1),
                method: "shutdown".to_owned(),
                params: Value::Null
            })
        );
        assert_eq!(read(&mut input)?, None);
        Ok(())
    }

    #[test]
    fn broken_frames_are_errors_not_panics() {
        for bad in [
            &b"Content-Length: 10\r\n\r\n{}"[..],
            b"Content-Length: x\r\n\r\n{}",
            b"\r\n{}",
            b"Content-Length: 99999999999\r\n\r\n",
            b"Content-Length: 2",
        ] {
            assert!(read(&mut io::Cursor::new(bad)).is_err());
        }
    }

    #[test]
    fn malformed_messages_get_the_right_error() {
        let code = |value: &Value| value["error"]["code"].as_i64();
        let e = parse(b"{not json").err().unwrap_or_default();
        assert_eq!(code(&e), Some(PARSE_ERROR));
        for bad in [
            &br#"[{"jsonrpc":"2.0","id":1,"method":"x"}]"#[..],
            br#"{"id":1,"method":"x"}"#,
            br#"{"jsonrpc":"2.0","id":1}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":7}"#,
            br#"{"jsonrpc":"2.0","id":{},"method":"x"}"#,
        ] {
            let e = parse(bad).err().unwrap_or_default();
            assert_eq!(code(&e), Some(INVALID_REQUEST));
        }
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","id":4,"result":null}"#),
            Ok(Message::Response)
        );
        // Positional parameters are legal JSON-RPC and are passed through.
        assert!(matches!(
            parse(br#"{"jsonrpc":"2.0","method":"x","params":[1]}"#),
            Ok(Message::Notification { .. })
        ));
    }
}

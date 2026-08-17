// Copyright 2026 Salesforce, Inc. All rights reserved.

//! The slice of MCP this policy needs: the JSON-RPC envelope and the two
//! methods it acts on. Derived inline rather than taken from a shared crate so
//! the policy builds in any PDK workspace.

use serde_json::Value;

pub const TOOLS_CALL_METHOD_NAME: &str = "tools/call";
pub const TOOLS_LIST_METHOD_NAME: &str = "tools/list";

pub const CONTENT_TYPE_HEADER: &str = "content-type";
pub const CONTENT_LENGTH_HEADER: &str = "content-length";
pub const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";
pub const POST_METHOD: &str = "POST";

/// A JSON-RPC `id` canonicalized to a string so numeric and string ids share
/// one lookup key. Notifications have no id and are never correlated.
pub fn id_key(id: &Value) -> Option<String> {
    match id {
        Value::Null => None,
        Value::String(s) => Some(format!("s:{}", s)),
        Value::Number(n) => Some(format!("n:{}", n)),
        _ => None,
    }
}

/// Extracts `(id, params.name)` pairs from a `tools/call` request body,
/// handling both a single JSON-RPC object and a batch array.
pub fn correlate_request(body: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    match body {
        Value::Array(batch) => {
            for msg in batch {
                if let Some(pair) = correlate_one(msg) {
                    out.push(pair);
                }
            }
        }
        single => {
            if let Some(pair) = correlate_one(single) {
                out.push(pair);
            }
        }
    }
    out
}

/// Whether the request body asks for tool descriptors, single or batched.
pub fn requests_tools_list(body: &Value) -> bool {
    let asks =
        |msg: &Value| msg.get("method").and_then(Value::as_str) == Some(TOOLS_LIST_METHOD_NAME);
    match body {
        Value::Array(batch) => batch.iter().any(asks),
        single => asks(single),
    }
}

fn correlate_one(msg: &Value) -> Option<(String, String)> {
    if msg.get("method").and_then(Value::as_str)? != TOOLS_CALL_METHOD_NAME {
        return None;
    }
    let key = id_key(msg.get("id")?)?;
    let name = msg.pointer("/params/name").and_then(Value::as_str)?;
    Some((key, name.to_string()))
}

/// True when the result should be left alone regardless of contract matches:
/// an error result carries a failure message, not data to interpret.
pub fn is_error_result(result: &Value) -> bool {
    result.get("isError").and_then(Value::as_bool) == Some(true)
}

/// True when the result is a `tools/list` payload. Descriptors are never
/// annotated, so a shape check guards against a mis-correlated id.
pub fn is_tools_list_result(result: &Value) -> bool {
    result.get("tools").map(Value::is_array) == Some(true)
}

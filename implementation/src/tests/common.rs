// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Shared fixtures and assertions for the policy-level tests.

use pdk_unit::{UnitHttpMessage, UnitHttpRequest, UnitHttpResponse, UnitTest, UnitTestBuilder};
use serde_json::{json, Value};

pub const DELIMITER: &str = "--- GATEWAY SEMANTIC CONTRACT (trusted) ---";
pub const TOOL: &str = "get_sales_order";
/// The JSON-RPC id used by every single-call helper in this suite.
pub const REQUEST_ID: i64 = 1;

/// The spec §11 conformance vector: seven rules, frozen at the version the
/// build spec enumerates. The contract actually deployed has since dropped the
/// two rules that outputSchema covers; see [`ERP_CONDITIONAL_CONTRACT`].
pub const ERP_CONTRACT: &str = include_str!("../../tests/fixtures/erp-sales-order.contract.json");
/// The deployed contract: only rules whose truth depends on the payload.
pub const ERP_CONDITIONAL_CONTRACT: &str =
    include_str!("../../tests/fixtures/erp-sales-order-conditional.contract.json");
pub const ERP_RESPONSE: &str = include_str!("../../tests/fixtures/erp-sales-order.response.json");

/// Every rule in the spec §11 contract, in declaration order.
pub const ALL_ERP_RULES: [&str; 7] = [
    "uom-external",
    "shipped-means-goods-issued",
    "delivery-block-active",
    "confirmed-date-stale",
    "credit-blocked-over-limit",
    "holds-role-filtered",
    "replica-stale",
];

/// The ERP sales order document, as returned by the upstream.
pub fn erp_order() -> Value {
    let response: Value = serde_json::from_str(ERP_RESPONSE).unwrap();
    response["result"]["structuredContent"].clone()
}

/// A `tools/call` response carrying `doc` as both text and structured content,
/// answering the id that [`call_tool`] sends.
pub fn erp_response_with(doc: Value) -> Value {
    let mut response: Value = serde_json::from_str(ERP_RESPONSE).unwrap();
    response["id"] = json!(REQUEST_ID);
    response["result"]["content"][0]["text"] = Value::String(doc.to_string());
    response["result"]["structuredContent"] = doc;
    response
}

pub fn config_with(contracts: Value) -> Value {
    json!({
        "envelope": { "delimiter": DELIMITER, "sanitizeUpstreamDelimiter": true },
        "contracts": contracts,
        "merge": {
            "order": ["json", "markdown", "text"],
            "globalMaxTokens": 4000,
            "duplicateRuleIds": "firstWins",
            "onBudgetExceeded": "dropBySeverity"
        },
        "dedupe": { "injectOncePer": "call", "sessionTtlSeconds": 900 },
        "warnOnUncoveredTools": true
    })
}

/// The default configuration: the shipped ERP contract, generous budget, no dedupe.
pub fn erp_config() -> Value {
    config_with(json!([{
        "contractId": "erp-sales-order",
        "format": "json",
        "inline": ERP_CONTRACT,
        "toolMapping": [TOOL],
    }]))
}

/// The deployed configuration: conditional rules only, static meaning delegated
/// to the tool's `outputSchema`.
pub fn conditional_config() -> Value {
    config_with(json!([{
        "contractId": "erp-sales-order",
        "format": "json",
        "inline": ERP_CONDITIONAL_CONTRACT,
        "toolMapping": [TOOL],
    }]))
}

/// The default configuration with SSE annotation switched on.
pub fn sse_config() -> Value {
    let mut config = erp_config();
    config["sse"] = json!({ "mode": "annotate", "streamTimeoutMillis": 60000 });
    config
}

/// Builds a tester whose upstream replies with `response_body` as JSON.
pub fn tester_with(config: &Value, response_body: String) -> UnitTest {
    tester_serving(config, "application/json", response_body)
}

/// Builds a tester whose upstream replies with `response_body` under an
/// arbitrary content type, for the transports JSON helpers cannot express.
pub fn tester_serving(config: &Value, content_type: &str, response_body: String) -> UnitTest {
    let content_type = content_type.to_string();
    UnitTestBuilder::default()
        .with_config(&config.to_string())
        .with_backend(move |_req| {
            UnitHttpResponse::new(200)
                .with_header("content-type", &content_type)
                .with_body(response_body.clone())
        })
        .with_entrypoint(crate::configure)
}

/// Drives one `tools/call` and returns the client-visible body as raw text,
/// leaving framing intact for the caller to inspect.
pub fn call_tool_raw(tester: &mut UnitTest, tool: &str) -> String {
    let response = tester.request(
        UnitHttpRequest::post()
            .with_path("/mcp")
            .with_header("content-type", "application/json")
            .with_header("accept", "application/json, text/event-stream")
            .with_body(
                json!({
                    "jsonrpc": "2.0",
                    "id": REQUEST_ID,
                    "method": "tools/call",
                    "params": { "name": tool, "arguments": { "salesOrderId": "0000004711" } }
                })
                .to_string(),
            ),
    );
    assert_eq!(response.status_code(), 200, "policy must not alter status");
    String::from_utf8(response.body().to_vec()).expect("response body must remain UTF-8")
}

/// Wraps `payload` in the single-frame SSE envelope A2D and other streamable
/// HTTP MCP servers use for a `tools/call` answer.
pub fn sse_frame(payload: &Value) -> String {
    format!("event: message\ndata: {}\n\n", payload)
}

/// The JSON-RPC payloads carried by the `data:` lines of an SSE body.
pub fn sse_payloads(body: &str) -> Vec<Value> {
    body.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

/// Drives one `tools/call` through the policy and returns the client-visible body.
pub fn call_tool(tester: &mut UnitTest, tool: &str) -> Value {
    call_with_body(
        tester,
        json!({
            "jsonrpc": "2.0",
            "id": REQUEST_ID,
            "method": "tools/call",
            "params": { "name": tool, "arguments": { "salesOrderId": "0000004711" } }
        })
        .to_string(),
    )
}

pub fn call_with_body(tester: &mut UnitTest, request_body: String) -> Value {
    let response = tester.request(
        UnitHttpRequest::post()
            .with_path("/mcp")
            .with_header("content-type", "application/json")
            .with_header("accept", "application/json")
            .with_body(request_body),
    );
    assert_eq!(response.status_code(), 200, "policy must not alter status");
    serde_json::from_slice(response.body()).expect("response body must remain valid JSON")
}

/// The upstream document as it survived the policy, with the gateway's own
/// field removed so it can be compared byte for byte against what the backend
/// sent. The policy owns `_semanticContract`; everything else must be untouched.
pub fn upstream_document(result: &Value) -> Value {
    let mut doc = result["structuredContent"].clone();
    if let Some(obj) = doc.as_object_mut() {
        obj.remove(crate::inject::CONTRACT_FIELD);
    }
    doc
}

/// The guidance the gateway wrote into `structuredContent`, in output order.
pub fn structured_guidance(result: &Value) -> Vec<String> {
    result["structuredContent"][crate::inject::CONTRACT_FIELD]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The text of the injected guidance block, or `None` if the policy stayed silent.
pub fn injected_block(result: &Value) -> Option<String> {
    let content = result.get("content")?.as_array()?;
    content
        .iter()
        .filter_map(|el| el.get("text").and_then(Value::as_str))
        .find(|t| t.starts_with(DELIMITER))
        .map(str::to_string)
}

/// Rule IDs present in the injected block, in output order.
pub fn fired_rules(body: &Value) -> Vec<String> {
    let Some(block) = injected_block(&body["result"]) else {
        return Vec::new();
    };
    block
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(": ").map(|(id, _)| id.to_string()))
        .collect()
}

/// Asserts the fired set matches `expected` exactly, order-insensitively.
pub fn assert_fired(body: &Value, expected: &[&str]) {
    let mut actual = fired_rules(body);
    actual.sort();
    let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(actual, want, "fired rule set mismatch");
}

/// Asserts that none of `silent` appear, and that the rest of the contract still fires.
pub fn assert_silent(body: &Value, silent: &[&str]) {
    let expected: Vec<&str> = ALL_ERP_RULES
        .iter()
        .filter(|r| !silent.contains(r))
        .copied()
        .collect();
    assert_fired(body, &expected);
}

// Copyright 2026 Salesforce, Inc. All rights reserved.

//! End-to-end demo against captured responses from the A2D mock ERP MCP server
//! (`erp-sales-order-mcp`, tool `get_sales_order`).
//!
//! These are real recorded bodies, not hand-written ones: the mock returns the
//! order document only as JSON text inside `content[0].text`, with no
//! `structuredContent`, so they exercise the text-binding path that a large
//! share of real MCP servers use.
//!
//! Run with output:
//!   cargo test --lib tests::a2d_demo -- --nocapture

use super::common::*;
use serde_json::Value;

const LIVE_ORDER: &str = include_str!("../../tests/fixtures/a2d-live-order.response.json");
const LIVE_POISONED: &str = include_str!("../../tests/fixtures/a2d-live-poisoned.response.json");

fn run(recorded: &str) -> (Value, Value) {
    let before: Value = serde_json::from_str(recorded).unwrap();
    // The recorded id must match the id the demo request sends.
    let mut upstream = before.clone();
    upstream["id"] = serde_json::json!(REQUEST_ID);

    let mut tester = tester_with(&erp_config(), upstream.to_string());
    let after = call_tool(&mut tester, TOOL);
    (before, after)
}

fn print_transformation(title: &str, before: &Value, after: &Value) {
    println!("\n{}\n{}", title, "=".repeat(title.len()));

    let upstream_text = before["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    println!("\n-- what the model saw before the policy --");
    println!("{}", &upstream_text[..upstream_text.len().min(600)]);

    println!("\n-- what the model sees through the gateway --");
    match injected_block(&after["result"]) {
        Some(block) => println!("{}", block),
        None => println!("(no guidance injected)"),
    }
}

#[test]
fn live_order_is_annotated_with_the_full_contract() {
    let (before, after) = run(LIVE_ORDER);
    print_transformation("A2D mock: sales order 0000004711", &before, &after);

    assert_fired(&after, &ALL_ERP_RULES);

    // The gateway adds a sibling element; it never edits the upstream one.
    let content = after["result"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(
        content[0], before["result"]["content"][0],
        "the recorded upstream element must survive verbatim"
    );
}

#[test]
fn a_forged_delimiter_from_the_mock_is_defanged() {
    let (before, after) = run(LIVE_POISONED);
    print_transformation(
        "A2D mock: prompt-injected order 0000009999",
        &before,
        &after,
    );

    let forged = before["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        forged.contains(DELIMITER),
        "the recorded upstream body must actually contain the forged delimiter"
    );

    let upstream_after = after["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        !upstream_after.contains(DELIMITER),
        "the forged delimiter must not survive:\n{}",
        upstream_after
    );
    assert!(
        upstream_after.contains("credit-override"),
        "text is defanged, not censored"
    );

    let joined = after["result"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|el| el["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        joined.matches(DELIMITER).count(),
        1,
        "exactly one trusted block, written by the gateway"
    );

    // The injected guidance still states the real credit position, contradicting
    // the injected claim that credit was cleared.
    let block = injected_block(&after["result"]).unwrap();
    assert!(block.contains("credit-blocked-over-limit"));
}

#[test]
fn the_live_stream_framing_is_annotated_as_the_mock_actually_sends_it() {
    // A2D answers every tools/call with a single SSE frame and refuses an
    // Accept header without text/event-stream, so this is the transport the
    // policy meets in the deployed demo — not the JSON path above.
    let mut upstream: Value = serde_json::from_str(LIVE_ORDER).unwrap();
    upstream["id"] = serde_json::json!(REQUEST_ID);

    let mut tester = tester_serving(&sse_config(), "text/event-stream", sse_frame(&upstream));
    let body = call_tool_raw(&mut tester, TOOL);

    assert!(
        body.starts_with("event: message\ndata: "),
        "body was: {}",
        body
    );
    assert!(body.ends_with("\n\n"), "the frame must stay dispatched");

    let payloads = sse_payloads(&body);
    assert_eq!(payloads.len(), 1);
    assert_fired(&payloads[0], &ALL_ERP_RULES);

    print_transformation(
        "A2D mock over text/event-stream: sales order 0000004711",
        &serde_json::from_str::<Value>(LIVE_ORDER).unwrap(),
        &payloads[0],
    );
}

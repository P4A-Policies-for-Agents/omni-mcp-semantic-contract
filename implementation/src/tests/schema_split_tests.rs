// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Tests for the division of labour between the tool's `outputSchema` and the
//! contract. Static field meaning belongs in the schema, which every MCP client
//! reads without a gateway; the contract carries only claims that depend on the
//! values a particular payload holds. These tests pin that boundary, so a rule
//! that is really a field description cannot creep back in unnoticed.

use super::common::*;
use serde_json::{json, Value};

/// Every rule left in the deployed contract, in declaration order.
const CONDITIONAL_RULES: [&str; 5] = [
    "shipped-means-goods-issued",
    "confirmed-date-stale",
    "credit-blocked-over-limit",
    "holds-role-filtered",
    "replica-stale",
];

/// Rules dropped in 4.0.0 because `outputSchema` states them unconditionally.
const SCHEMA_COVERED_RULES: [&str; 2] = ["uom-external", "delivery-block-active"];

/// Runs the deployed contract against a mutated order document.
fn run_with_order(mutate: impl FnOnce(&mut Value)) -> Value {
    let mut doc = erp_order();
    mutate(&mut doc);
    let mut tester = tester_with(&conditional_config(), erp_response_with(doc).to_string());
    call_tool(&mut tester, TOOL)
}

// ---------------------------------------------------------------------------
// What the contract still carries
// ---------------------------------------------------------------------------

mod the_contract {
    use super::*;

    #[test]
    fn fires_every_conditional_rule_on_the_reference_order() {
        let body = run_with_order(|_| {});
        assert_fired(&body, &CONDITIONAL_RULES);
    }

    #[test]
    fn no_longer_carries_the_rules_the_schema_states() {
        let fired = fired_rules(&run_with_order(|_| {}));
        for rule in SCHEMA_COVERED_RULES {
            assert!(
                !fired.contains(&rule.to_string()),
                "{} is unconditional field meaning and belongs in outputSchema",
                rule
            );
        }
    }

    /// The point of moving static meaning out is that the agent stops paying for
    /// it on every single call.
    #[test]
    fn injects_a_smaller_block_than_the_seven_rule_version() {
        let slim = injected_block(&run_with_order(|_| {})["result"]).unwrap();

        let mut tester = tester_with(&erp_config(), erp_response_with(erp_order()).to_string());
        let full = injected_block(&call_tool(&mut tester, TOOL)["result"]).unwrap();

        assert!(
            slim.len() < full.len(),
            "conditional-only block ({} bytes) must be smaller than the full one ({} bytes)",
            slim.len(),
            full.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Every surviving rule must genuinely depend on the payload
// ---------------------------------------------------------------------------

/// A rule that fires on every possible payload is a field description wearing a
/// rule's clothes: it could have been said once in the schema. For each rule we
/// exhibit a document where it stays silent, which is the property no static
/// description can express.
mod each_rule_is_genuinely_conditional {
    use super::*;

    #[test]
    fn shipped_means_goods_issued_is_silent_once_goods_issue_posts() {
        let body = run_with_order(|doc| doc["items"][0]["goodsIssuedQuantity"] = json!(288));
        assert!(!fired_rules(&body).contains(&"shipped-means-goods-issued".to_string()));
    }

    #[test]
    fn confirmed_date_stale_is_silent_without_a_delivery_block() {
        let body = run_with_order(|doc| doc["header"]["deliveryBlock"] = Value::Null);
        assert!(!fired_rules(&body).contains(&"confirmed-date-stale".to_string()));
    }

    #[test]
    fn credit_blocked_over_limit_is_silent_when_credit_is_approved() {
        let body = run_with_order(|doc| doc["credit"]["creditStatus"] = json!("A"));
        assert!(!fired_rules(&body).contains(&"credit-blocked-over-limit".to_string()));
    }

    #[test]
    fn credit_blocked_over_limit_is_silent_when_exposure_is_within_limit() {
        let body = run_with_order(|doc| doc["credit"]["exposure"] = json!(1000.0));
        assert!(!fired_rules(&body).contains(&"credit-blocked-over-limit".to_string()));
    }

    #[test]
    fn holds_role_filtered_is_silent_when_a_hold_is_actually_visible() {
        let body =
            run_with_order(|doc| doc["holds"] = json!([{ "type": "credit", "blocking": true }]));
        assert!(!fired_rules(&body).contains(&"holds-role-filtered".to_string()));
    }

    #[test]
    fn replica_stale_is_silent_within_the_lag_threshold() {
        let body = run_with_order(|doc| doc["meta"]["replicationLagSeconds"] = json!(120));
        assert!(!fired_rules(&body).contains(&"replica-stale".to_string()));
    }

    /// A clean order trips nothing at all: with the static meaning delegated to
    /// the schema, the policy is silent unless the payload warrants a warning.
    #[test]
    fn a_clean_order_produces_no_injection_at_all() {
        let body = run_with_order(|doc| {
            doc["header"]["deliveryBlock"] = Value::Null;
            doc["items"][0]["goodsIssuedQuantity"] = json!(288);
            doc["credit"]["creditStatus"] = json!("A");
            doc["credit"]["exposure"] = json!(1000.0);
            doc["meta"]["replicationLagSeconds"] = json!(0);
        });
        assert_fired(&body, &[]);
        assert!(
            injected_block(&body["result"]).is_none(),
            "a clean order must not carry a trusted block at all"
        );
    }
}

// ---------------------------------------------------------------------------
// The rules must not restate what the schema already says
// ---------------------------------------------------------------------------

mod guidance_does_not_duplicate_the_schema {
    use super::*;

    /// The schema defines what these fields mean. If a rule spends its tokens
    /// re-teaching that vocabulary, the split has been undone.
    #[test]
    fn no_rule_redefines_a_field_the_schema_documents() {
        let block = injected_block(&run_with_order(|_| {})["result"]).unwrap();
        for phrase in ["base unit", "conversionFactor", "A = open", "ISO 4217"] {
            assert!(
                !block.contains(phrase),
                "guidance restates schema vocabulary: {}",
                phrase
            );
        }
    }

    /// Each rule should read as a statement about this order, not about the API.
    #[test]
    fn every_rule_stays_within_a_sentence_or_two() {
        let block = injected_block(&run_with_order(|_| {})["result"]).unwrap();
        for line in block.lines().skip(1) {
            assert!(
                line.len() < 260,
                "rule guidance has grown into documentation: {}",
                line
            );
        }
    }
}

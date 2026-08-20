// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Tests for the `erp-delivery` contract, which carries operational overlays
//! rather than field meaning.
//!
//! [`schema_split_tests`](super::schema_split_tests) pins the weaker property
//! that a rule must depend on the payload. These tests pin the stronger one the
//! delivery contract was written to demonstrate: every rule asserts a fact that
//! is **not in the payload at all** and lives in a system the ERP delivery API
//! does not read. A recall, a carrier incident, a licence hold, an engineering
//! change. That is the class of claim no `outputSchema` can carry, because the
//! schema is authored once with the tool and these facts change daily.

use super::common::{
    assert_fired, config_with, fired_rules, injected_block, structured_guidance, tester_with,
    REQUEST_ID,
};
use pdk_unit::{UnitHttpMessage, UnitHttpRequest, UnitTest};
use serde_json::{json, Value};

const DELIVERY_TOOL: &str = "get_delivery_document";
const DELIVERY_CONTRACT: &str = include_str!("../../tests/fixtures/erp-delivery.contract.json");
const DELIVERY_MOCKS: &str = include_str!("../../tests/fixtures/erp-delivery.mocks.json");

/// The delivery that looks entirely healthy on paper.
const SUSPECT: &str = "0080067890";
/// The delivery nothing is wrong with.
const CLEAN: &str = "0080012345";
/// The delivery that must not be discussed with the customer at all.
const RESTRICTED: &str = "0080055512";

/// Every rule in the delivery contract, in declaration order.
const ALL_RULES: [&str; 6] = [
    "batch-under-recall",
    "carrier-service-suspended",
    "export-licence-missing",
    "customer-communication-hold",
    "material-superseded",
    "legacy-plant-pricing",
];

fn contract() -> Value {
    serde_json::from_str(DELIVERY_CONTRACT).unwrap()
}

/// One of the three mock delivery documents, by id.
fn delivery(id: &str) -> Value {
    serde_json::from_str::<Value>(DELIVERY_MOCKS).unwrap()[id].clone()
}

fn delivery_config() -> Value {
    config_with(json!([{
        "contractId": "erp-delivery",
        "format": "json",
        "inline": DELIVERY_CONTRACT,
        "toolMapping": [DELIVERY_TOOL],
    }]))
}

/// A `tools/call` response carrying `doc` as both text and structured content,
/// the way A2D answers once the tool declares an `outputSchema`.
fn response_with(doc: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": REQUEST_ID,
        "result": {
            "content": [{ "type": "text", "text": doc.to_string() }],
            "structuredContent": doc,
            "isError": false
        }
    })
}

fn call_delivery(tester: &mut UnitTest, id: &str) -> Value {
    let response = tester.request(
        UnitHttpRequest::post()
            .with_path("/mcp")
            .with_header("content-type", "application/json")
            .with_header("accept", "application/json")
            .with_body(
                json!({
                    "jsonrpc": "2.0",
                    "id": REQUEST_ID,
                    "method": "tools/call",
                    "params": {
                        "name": DELIVERY_TOOL,
                        "arguments": { "deliveryId": id }
                    }
                })
                .to_string(),
            ),
    );
    assert_eq!(response.status_code(), 200, "policy must not alter status");
    serde_json::from_slice(response.body()).expect("response body must remain valid JSON")
}

/// Runs the delivery contract against mock document `id`, optionally mutated.
fn run(id: &str, mutate: impl FnOnce(&mut Value)) -> Value {
    let mut doc = delivery(id);
    mutate(&mut doc);
    let mut tester = tester_with(&delivery_config(), response_with(doc).to_string());
    call_delivery(&mut tester, id)
}

// ---------------------------------------------------------------------------
// The three scenarios
// ---------------------------------------------------------------------------

mod scenarios {
    use super::*;

    /// Everything a schema-reading client can see says this shipment is fine:
    /// fully picked, goods issued, tracking number, an arrival date three days
    /// out. Four independent facts held elsewhere say otherwise.
    #[test]
    fn the_healthy_looking_delivery_trips_four_rules() {
        assert_fired(
            &run(SUSPECT, |_| {}),
            &[
                "batch-under-recall",
                "carrier-service-suspended",
                "material-superseded",
                "legacy-plant-pricing",
            ],
        );
    }

    /// The export and legal holds are the two rules that tell the agent to
    /// withhold rather than to reinterpret.
    #[test]
    fn the_restricted_delivery_trips_both_withholding_rules() {
        assert_fired(
            &run(RESTRICTED, |_| {}),
            &["export-licence-missing", "customer-communication-hold"],
        );
    }

    /// Credibility depends on silence being the normal case.
    #[test]
    fn the_clean_delivery_trips_nothing_at_all() {
        let body = run(CLEAN, |_| {});
        assert_fired(&body, &[]);
        assert!(
            injected_block(&body["result"]).is_none(),
            "a clean delivery must carry no trusted block"
        );
        assert!(
            structured_guidance(&body["result"]).is_empty(),
            "a clean delivery must carry no _semanticContract field"
        );
    }
}

// ---------------------------------------------------------------------------
// Every rule carries knowledge the payload does not contain
// ---------------------------------------------------------------------------

mod knowledge_is_external {
    use super::*;

    /// The property that separates this contract from a well-written schema.
    /// Each rule cites the artifact it came from — a recall number, an incident
    /// id, a compliance standard, an engineering change. If that identifier
    /// appeared in the payload, the tool would already be publishing the fact
    /// and the rule would be redundant.
    #[test]
    fn no_rule_reference_appears_anywhere_in_the_payload() {
        let documents: Value = serde_json::from_str(DELIVERY_MOCKS).unwrap();
        let corpus = documents.to_string();

        for rule in contract()["rules"].as_array().unwrap() {
            let reference = rule["reference"].as_str().unwrap();
            assert!(
                !corpus.contains(reference),
                "{} cites {}, which the payload already carries; \
                 a fact the document states belongs in outputSchema",
                rule["id"],
                reference
            );
        }
    }

    /// Each rule names the date or identifier that makes it true today. A rule
    /// whose guidance holds forever is a field description, and the schema is
    /// where it should live.
    #[test]
    fn every_rule_names_the_artifact_that_makes_it_current() {
        for rule in contract()["rules"].as_array().unwrap() {
            let guidance = rule["guidance"].as_str().unwrap();
            let reference = rule["reference"].as_str().unwrap();
            let dated = guidance.contains("2026-");
            assert!(
                dated || guidance.contains(reference),
                "{} states a timeless fact; move it to outputSchema",
                rule["id"]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Every rule is genuinely conditional
// ---------------------------------------------------------------------------

/// For each rule, a document on which it stays silent. Without this a rule is
/// an unconditional statement that could have been said once, in the schema.
mod each_rule_is_conditional {
    use super::*;

    fn assert_silent(body: &Value, rule: &str) {
        assert!(
            !fired_rules(body).contains(&rule.to_string()),
            "{} fired on a document it has no business firing on",
            rule
        );
    }

    #[test]
    fn recall_is_silent_on_stock_from_another_batch() {
        let body = run(SUSPECT, |doc| {
            doc["items"][1]["batchNumber"] = json!("B-7742-2026")
        });
        assert_silent(&body, "batch-under-recall");
    }

    /// The recalled batch sits on the second line, so a rule that only looked
    /// at the first would miss it. Losing the first line must not silence it.
    #[test]
    fn recall_still_fires_when_the_batch_is_not_on_the_first_line() {
        assert!(fired_rules(&run(SUSPECT, |_| {}))
            .contains(&"batch-under-recall".to_string()));
    }

    /// The rule names a batch series, not one batch, so a different batch from
    /// the same recall fires without the contract being re-authored.
    #[test]
    fn recall_covers_every_batch_in_the_series() {
        let body = run(SUSPECT, |doc| {
            doc["items"][1]["batchNumber"] = json!("B-7741-2099")
        });
        assert!(fired_rules(&body).contains(&"batch-under-recall".to_string()));
    }

    /// And it quantifies over every line, not the first two.
    #[test]
    fn recall_reaches_a_line_beyond_the_first_two() {
        let body = run(SUSPECT, |doc| {
            doc["items"][1]["batchNumber"] = json!("B-6604-2026");
            let mut third = doc["items"][1].clone();
            third["itemNumber"] = json!(30);
            third["batchNumber"] = json!("B-7741-2031");
            doc["items"].as_array_mut().unwrap().push(third);
        });
        assert!(fired_rules(&body).contains(&"batch-under-recall".to_string()));
    }

    #[test]
    fn carrier_suspension_is_silent_on_the_carriers_standard_service() {
        let body = run(SUSPECT, |doc| {
            doc["shipping"]["serviceLevel"] = json!("STANDARD")
        });
        assert_silent(&body, "carrier-service-suspended");
    }

    #[test]
    fn carrier_suspension_is_silent_on_a_different_carrier() {
        let body = run(SUSPECT, |doc| doc["shipping"]["carrier"] = json!("STD-EU"));
        assert_silent(&body, "carrier-service-suspended");
    }

    #[test]
    fn export_hold_is_silent_once_a_licence_is_recorded() {
        let body = run(RESTRICTED, |doc| {
            doc["exportControl"]["licenseNumber"] = json!("DE-BAFA-2026-11884")
        });
        assert_silent(&body, "export-licence-missing");
    }

    #[test]
    fn export_hold_is_silent_on_uncontrolled_goods() {
        let body = run(RESTRICTED, |doc| {
            doc["exportControl"]["eccn"] = Value::Null
        });
        assert_silent(&body, "export-licence-missing");
    }

    #[test]
    fn communication_hold_is_silent_for_a_customer_not_in_dispute() {
        let body = run(RESTRICTED, |doc| {
            doc["header"]["shipToParty"] = json!("C-10014")
        });
        assert_silent(&body, "customer-communication-hold");
    }

    #[test]
    fn supersession_is_silent_on_the_replacement_material() {
        let body = run(SUSPECT, |doc| {
            doc["items"][0]["material"] = json!("MAT-88121")
        });
        assert_silent(&body, "material-superseded");
    }

    #[test]
    fn legacy_pricing_is_silent_on_a_document_created_after_the_cutover() {
        let body = run(SUSPECT, |doc| doc["header"]["createdOn"] = json!("2026-08-04"));
        assert_silent(&body, "legacy-plant-pricing");
    }

    #[test]
    fn legacy_pricing_is_silent_at_another_plant() {
        let body = run(SUSPECT, |doc| doc["header"]["plant"] = json!("2000"));
        assert_silent(&body, "legacy-plant-pricing");
    }
}

// ---------------------------------------------------------------------------
// Delivery of the guidance itself
// ---------------------------------------------------------------------------

mod delivery_to_the_client {
    use super::*;

    /// A2D declares an `outputSchema` for this tool, so the client reads
    /// `structuredContent` and may discard extra content elements. Both
    /// channels must carry the same rules.
    #[test]
    fn both_channels_carry_the_same_four_rules() {
        let body = run(SUSPECT, |_| {});
        let structured = structured_guidance(&body["result"]);
        let block = injected_block(&body["result"]).expect("content channel must carry the block");

        assert_eq!(structured.len(), 4, "structured channel must carry every rule");
        for line in &structured {
            let id = line.split_once(": ").unwrap().0;
            assert!(block.contains(id), "{} missing from the content channel", id);
        }
    }

    /// Critical rules outrank warnings when the budget bites, and the two
    /// withholding rules are the ones that must never be dropped.
    #[test]
    fn withholding_rules_survive_a_budget_too_small_for_everything() {
        let mut config = delivery_config();
        config["merge"]["globalMaxTokens"] = json!(60);

        let mut tester = tester_with(&config, response_with(delivery(SUSPECT)).to_string());
        let body = call_delivery(&mut tester, SUSPECT);
        let fired = fired_rules(&body);

        assert!(fired.contains(&"batch-under-recall".to_string()));
        assert!(fired.contains(&"carrier-service-suspended".to_string()));
        assert!(
            !fired.contains(&"legacy-plant-pricing".to_string()),
            "a warn rule must yield before a critical one"
        );
    }

    #[test]
    fn the_contract_governs_only_the_delivery_tool() {
        let contract = contract();
        let mapping = contract["toolMapping"].as_array().unwrap();
        assert_eq!(mapping, &[json!(DELIVERY_TOOL)]);
    }

    #[test]
    fn every_declared_rule_fires_on_at_least_one_scenario() {
        let mut seen: Vec<String> = Vec::new();
        for id in [SUSPECT, CLEAN, RESTRICTED] {
            seen.extend(fired_rules(&run(id, |_| {})));
        }
        for rule in ALL_RULES {
            assert!(
                seen.contains(&rule.to_string()),
                "{} has no scenario that demonstrates it",
                rule
            );
        }
    }
}

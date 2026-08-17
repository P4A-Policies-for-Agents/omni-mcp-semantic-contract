// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Configuration validation.
//!
//! Config errors fail loudly at load time. Runtime errors fail open, but a
//! misconfigured policy that silently governs nothing is worse than one that
//! refuses to start.

use super::common::*;
use crate::config::PolicyConfig;
use serde_json::{json, Value};

fn load(config: &Value) -> Result<PolicyConfig, String> {
    PolicyConfig::load(config.to_string().as_bytes()).map_err(|e| e.to_string())
}

fn expect_rejected(config: &Value, expected_fragment: &str) {
    let err = load(config).err().unwrap_or_else(|| {
        panic!(
            "config was accepted but should have been rejected: {}",
            config
        )
    });
    assert!(
        err.contains(expected_fragment),
        "error `{}` should mention `{}`",
        err,
        expected_fragment
    );
}

#[test]
fn the_reference_config_loads() {
    let config = load(&erp_config()).unwrap();
    assert_eq!(config.contracts.len(), 1);
    assert_eq!(config.delimiter, DELIMITER);
    assert!(config.sanitize_upstream);
    assert!(config.remote.is_none());
}

#[test]
fn an_empty_configuration_is_rejected() {
    assert!(PolicyConfig::load(b"").is_err());
}

#[test]
fn malformed_json_is_rejected() {
    assert!(PolicyConfig::load(b"{ not json").is_err());
}

// ---------------------------------------------------------------------------
// Envelope and budget
// ---------------------------------------------------------------------------

#[test]
fn an_empty_delimiter_is_rejected() {
    let mut config = erp_config();
    config["envelope"]["delimiter"] = json!("   ");
    expect_rejected(&config, "delimiter");
}

#[test]
fn a_non_positive_token_budget_is_rejected() {
    for value in [0, -1] {
        let mut config = erp_config();
        config["merge"]["globalMaxTokens"] = json!(value);
        expect_rejected(&config, "globalMaxTokens");
    }
}

#[test]
fn an_unknown_budget_strategy_is_rejected() {
    let mut config = erp_config();
    config["merge"]["onBudgetExceeded"] = json!("dropLowestSeverity");
    expect_rejected(&config, "onBudgetExceeded");
}

#[test]
fn an_unknown_merge_format_is_rejected() {
    let mut config = erp_config();
    config["merge"]["order"] = json!(["json", "yaml"]);
    expect_rejected(&config, "yaml");
}

// ---------------------------------------------------------------------------
// Contracts
// ---------------------------------------------------------------------------

#[test]
fn a_contract_without_inline_content_is_rejected() {
    let config = config_with(json!([{
        "contractId": "empty", "format": "json", "toolMapping": [TOOL],
    }]));
    expect_rejected(&config, "inline");
}

#[test]
fn an_unparseable_contract_is_rejected_rather_than_skipped() {
    let config = config_with(json!([{
        "contractId": "broken", "format": "json",
        "inline": "{ not a contract", "toolMapping": [TOOL],
    }]));
    expect_rejected(&config, "broken");
}

#[test]
fn duplicate_contract_ids_are_rejected() {
    let entry = json!({
        "contractId": "erp-sales-order", "format": "json",
        "inline": ERP_CONTRACT, "toolMapping": [TOOL],
    });
    let config = config_with(json!([entry, entry]));
    expect_rejected(&config, "erp-sales-order");
}

#[test]
fn format_url_on_an_inline_entry_points_at_the_right_setting() {
    let config = config_with(json!([{
        "contractId": "remote", "format": "url",
        "inline": "https://example.com/c.json", "toolMapping": [TOOL],
    }]));
    expect_rejected(&config, "remoteContractUrl");
}

// ---------------------------------------------------------------------------
// Merge order
// ---------------------------------------------------------------------------

#[test]
fn contracts_are_ordered_by_the_configured_format_precedence() {
    let text = json!({ "contractId": "t", "format": "text",
                       "inline": "Text guidance.", "toolMapping": [TOOL] });
    let js = json!({ "contractId": "j", "format": "json",
                     "inline": ERP_CONTRACT, "toolMapping": [TOOL] });

    let mut config = config_with(json!([text, js]));
    config["merge"]["order"] = json!(["json", "markdown", "text"]);
    let loaded = load(&config).unwrap();
    let ids: Vec<&str> = loaded
        .contracts_for(None, TOOL)
        .iter()
        .map(|c| c.contract_id.as_str())
        .collect();
    assert_eq!(ids, vec!["j", "t"], "json outranks text");

    config["merge"]["order"] = json!(["text", "json", "markdown"]);
    let loaded = load(&config).unwrap();
    let ids: Vec<&str> = loaded
        .contracts_for(None, TOOL)
        .iter()
        .map(|c| c.contract_id.as_str())
        .collect();
    assert_eq!(ids, vec!["t", "j"], "precedence is configurable");
}

// ---------------------------------------------------------------------------
// SSE
// ---------------------------------------------------------------------------

mod sse {
    use super::*;

    #[test]
    fn streams_are_forwarded_and_calls_left_untimed_by_default() {
        let config = load(&erp_config()).unwrap();
        assert!(!config.annotate_sse);
        assert_eq!(
            config.upstream_timeout_ms, "0",
            "pass-through is what lets a long MCP call stream indefinitely"
        );
    }

    #[test]
    fn opting_in_bounds_the_upstream_timeout() {
        let config = load(&sse_config()).unwrap();
        assert!(config.annotate_sse);
        assert_eq!(config.upstream_timeout_ms, "60000");
    }

    #[test]
    fn the_bound_is_configurable() {
        let mut raw = erp_config();
        raw["sse"] = json!({ "mode": "annotate", "streamTimeoutMillis": 5000 });
        assert_eq!(load(&raw).unwrap().upstream_timeout_ms, "5000");
    }

    #[test]
    fn annotating_without_a_bound_is_rejected() {
        // Zero here would restore the unbounded wait the bound exists to prevent.
        let mut raw = erp_config();
        raw["sse"] = json!({ "mode": "annotate", "streamTimeoutMillis": 0 });
        expect_rejected(&raw, "streamTimeoutMillis");
    }

    #[test]
    fn an_unknown_mode_is_rejected() {
        let mut raw = erp_config();
        raw["sse"] = json!({ "mode": "buffer" });
        expect_rejected(&raw, "sse.mode");
    }

    #[test]
    fn a_timeout_without_annotate_stays_inert() {
        let mut raw = erp_config();
        raw["sse"] = json!({ "mode": "passThrough", "streamTimeoutMillis": 5000 });
        let config = load(&raw).unwrap();
        assert!(!config.annotate_sse);
        assert_eq!(
            config.upstream_timeout_ms, "0",
            "a bound only applies to the mode that needs one"
        );
    }
}

// ---------------------------------------------------------------------------
// Remote contract
// ---------------------------------------------------------------------------

mod remote {
    use super::*;

    fn with_remote(integrity: Option<&str>) -> Value {
        let mut config = erp_config();
        config["remoteContractUrl"] = json!("https://contracts.example.com/c.json");
        config["remoteContract"] = json!({
            "contractId": "remote",
            "cacheTtlSeconds": 900,
            "onFetchFailure": "useStale",
            "toolMapping": [TOOL],
            "severity": "warn"
        });
        if let Some(pin) = integrity {
            config["remoteContract"]["integrity"] = json!(pin);
        }
        config
    }

    #[test]
    fn a_url_without_an_integrity_pin_is_rejected() {
        // A policy whose purpose is to pin trusted guidance must not itself
        // load unverified guidance.
        expect_rejected(&with_remote(None), "integrity");
    }

    #[test]
    fn a_malformed_integrity_pin_is_rejected() {
        expect_rejected(&with_remote(Some("sha256:nothex")), "integrity");
        expect_rejected(&with_remote(Some("deadbeef")), "integrity");
    }

    #[test]
    fn a_well_formed_pin_is_accepted() {
        let pin = format!("sha256:{}", "a".repeat(64));
        let config = load(&with_remote(Some(&pin))).unwrap();
        let remote = config.remote.expect("remote spec");
        assert_eq!(remote.contract_id, "remote");
        assert_eq!(remote.ttl_secs, 900);
        assert!(remote.use_stale);
    }

    #[test]
    fn pass_through_disables_stale_reuse() {
        let pin = format!("sha256:{}", "a".repeat(64));
        let mut config = with_remote(Some(&pin));
        config["remoteContract"]["onFetchFailure"] = json!("passThrough");
        assert!(!load(&config).unwrap().remote.unwrap().use_stale);
    }

    #[test]
    fn settings_without_a_url_are_inert() {
        let mut config = erp_config();
        config["remoteContract"] = json!({ "contractId": "remote" });
        assert!(load(&config).unwrap().remote.is_none());
    }
}

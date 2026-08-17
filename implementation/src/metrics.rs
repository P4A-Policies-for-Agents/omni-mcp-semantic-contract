// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Counter emission.
//!
//! PDK exposes no counter primitive, so counters are emitted as structured log
//! lines with a fixed `semantic_contract_metric` prefix and `key=value` tags,
//! which is what a gateway log pipeline scrapes. Rule ids are logged; guidance
//! text and payload content never are.

use pdk::logger;

pub const RULE_FIRED: &str = "semantic_contract.rule_fired";
pub const RULES_DROPPED_BUDGET: &str = "semantic_contract.rules_dropped_budget";
pub const PAYLOAD_UNBINDABLE: &str = "semantic_contract.payload_unbindable";
pub const DELIMITER_SANITIZED: &str = "semantic_contract.delimiter_sanitized";
pub const CONTRACT_LOAD_FAILED: &str = "semantic_contract.contract_load_failed";
pub const SSE_SKIPPED: &str = "semantic_contract.sse_skipped";
pub const PASSTHROUGH_ON_ERROR: &str = "semantic_contract.passthrough_on_error";
pub const CRITICAL_OVER_BUDGET: &str = "semantic_contract.critical_over_budget";
pub const RULE_DEDUPED: &str = "semantic_contract.rule_deduped";
/// Which channels carried the guidance. `structured=false` on a tool that
/// declares an `outputSchema` means a schema-aware client saw nothing.
pub const GUIDANCE_DELIVERED: &str = "semantic_contract.guidance_delivered";

/// Emits a counter increment tagged with the asset and tool it belongs to.
/// `extra` carries counter-specific tags already formatted as `key=value`.
pub fn count(name: &str, asset: &str, tool: &str, extra: &str) {
    if extra.is_empty() {
        logger::info!(
            "semantic_contract_metric name={} assetId={} toolName={} value=1",
            name,
            asset,
            tool
        );
    } else {
        logger::info!(
            "semantic_contract_metric name={} assetId={} toolName={} {} value=1",
            name,
            asset,
            tool,
            extra
        );
    }
}

/// Emits a counter that is not scoped to a specific call, such as a contract
/// that failed to load at init.
pub fn count_global(name: &str, extra: &str) {
    logger::info!("semantic_contract_metric name={} {} value=1", name, extra);
}

// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Validated runtime configuration.
//!
//! Configuration errors fail loudly: the policy refuses to load rather than
//! silently governing nothing. This is the inverse of the runtime posture,
//! where every failure passes traffic through untouched.

use crate::contract::{self, Contract, Format, Severity};
use crate::generated::config::Config as RawConfig;
use crate::metrics;
use crate::state::DedupeScope;
use pdk::logger;
use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    Empty,
    Parse(serde_json::Error),
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Empty => f.write_str("configuration was empty"),
            ConfigError::Parse(e) => write!(f, "configuration could not be parsed: {}", e),
            ConfigError::Invalid(m) => write!(f, "invalid configuration: {}", m),
        }
    }
}

/// A hash-pinned contract artifact fetched over HTTP.
pub struct RemoteSpec {
    pub contract_id: String,
    pub service: pdk::hl::Service,
    /// `sha256:<64 hex>`. Mandatory: unverified content is never used.
    pub integrity: String,
    pub ttl_secs: u64,
    /// `useStale` keeps the last verified copy when a refetch fails.
    pub use_stale: bool,
    pub tools: Vec<String>,
    pub severity: Severity,
}

pub struct PolicyConfig {
    pub delimiter: String,
    pub sanitize_upstream: bool,
    /// Whether `text/event-stream` results are rewritten or forwarded as-is.
    pub annotate_sse: bool,
    /// Upstream timeout header value. `0` removes the timeout entirely, which
    /// is only safe while streams are forwarded rather than buffered.
    pub upstream_timeout_ms: String,
    /// Inline contracts in merge order. Rule selection walks this in sequence.
    pub contracts: Vec<Contract>,
    /// Format precedence, used to place a fetched contract among the inline ones.
    pub merge_order: Vec<String>,
    pub remote: Option<RemoteSpec>,
    pub global_max_tokens: usize,
    pub duplicate_first_wins: bool,
    pub dedupe_scope: DedupeScope,
    pub session_ttl_secs: u64,
    pub warn_on_uncovered_tools: bool,
}

/// The only budget strategy this version implements.
const BUDGET_STRATEGY: &str = "dropBySeverity";

const KNOWN_FORMATS: [&str; 4] = ["json", "markdown", "text", "url"];

impl PolicyConfig {
    /// Parses and validates the configuration bytes handed to the entrypoint.
    pub fn load(bytes: &[u8]) -> Result<PolicyConfig, ConfigError> {
        if bytes.is_empty() {
            return Err(ConfigError::Empty);
        }
        let raw: RawConfig = serde_json::from_slice(bytes).map_err(ConfigError::Parse)?;
        PolicyConfig::from_raw(raw)
    }

    fn from_raw(raw: RawConfig) -> Result<PolicyConfig, ConfigError> {
        if raw.merge.global_max_tokens <= 0 {
            return Err(ConfigError::Invalid(format!(
                "merge.globalMaxTokens must be greater than 0, got {}",
                raw.merge.global_max_tokens
            )));
        }

        let order = raw
            .merge
            .order
            .clone()
            .unwrap_or_else(|| KNOWN_FORMATS.iter().map(|s| s.to_string()).collect());
        for name in &order {
            if !KNOWN_FORMATS.contains(&name.as_str()) {
                return Err(ConfigError::Invalid(format!(
                    "merge.order contains unknown format `{}`; known formats are {}",
                    name,
                    KNOWN_FORMATS.join(", ")
                )));
            }
        }

        // Only one strategy exists. Rejecting anything else keeps a typo from
        // silently selecting the default behaviour under a different name.
        if raw.merge.on_budget_exceeded != BUDGET_STRATEGY {
            return Err(ConfigError::Invalid(format!(
                "merge.onBudgetExceeded must be `{}`, got `{}`",
                BUDGET_STRATEGY, raw.merge.on_budget_exceeded
            )));
        }

        if raw.envelope.delimiter.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "envelope.delimiter must not be empty; it is the trust anchor of the injected block"
                    .to_string(),
            ));
        }

        let mut seen_ids: Vec<String> = Vec::new();
        let mut contracts: Vec<Contract> = Vec::new();

        for entry in &raw.contracts {
            if seen_ids.contains(&entry.contract_id) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate contractId `{}`",
                    entry.contract_id
                )));
            }
            seen_ids.push(entry.contract_id.clone());

            let format = Format::parse(&entry.format).ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "contract `{}` has unknown format `{}`",
                    entry.contract_id, entry.format
                ))
            })?;

            let tools = entry.tool_mapping.clone().unwrap_or_default();
            let severity = entry
                .severity
                .as_deref()
                .and_then(Severity::parse)
                .unwrap_or(Severity::Warn);

            let inline = entry.inline.as_deref().unwrap_or("");
            if inline.trim().is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "contract `{}` has format `{}` but no `inline` content",
                    entry.contract_id,
                    format.as_str()
                )));
            }

            let loaded = match format {
                Format::Json => contract::load_json(inline, &tools, &entry.contract_id),
                Format::Markdown => {
                    contract::load_markdown(inline, &tools, severity, &entry.contract_id)
                }
                Format::Text => contract::load_text(inline, &tools, severity, &entry.contract_id),
                Format::Url => {
                    // `url` is not selectable on an inline entry; remote
                    // artifacts are configured under `remoteContract`.
                    return Err(ConfigError::Invalid(format!(
                        "contract `{}`: use `remoteContractUrl` for fetched artifacts",
                        entry.contract_id
                    )));
                }
            }
            .map_err(|e| {
                ConfigError::Invalid(format!("contract `{}`: {}", entry.contract_id, e))
            })?;

            for w in &loaded.warnings {
                match w {
                    contract::LoadWarning::BadExpression { rule_id, reason } => {
                        logger::warn!(
                            "semantic_contract: rule `{}` in contract `{}` disabled, \
                             malformed `when` expression: {}",
                            rule_id,
                            entry.contract_id,
                            reason
                        );
                    }
                    contract::LoadWarning::BadRule { rule_id, reason } => {
                        logger::warn!(
                            "semantic_contract: rule `{}` in contract `{}` skipped: {}",
                            rule_id,
                            entry.contract_id,
                            reason
                        );
                    }
                }
                metrics::count_global(
                    metrics::CONTRACT_LOAD_FAILED,
                    &format!("contractId={} scope=rule", entry.contract_id),
                );
            }

            // A contract that governs no tool would load and never fire.
            if loaded.contract.tools.is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "contract `{}` resolves to an empty tool mapping",
                    entry.contract_id
                )));
            }

            contracts.push(loaded.contract);
        }

        // Deterministic merge order: contracts sort by the configured format
        // precedence, keeping declaration order within a format.
        contracts.sort_by_key(|c| {
            order
                .iter()
                .position(|f| f == c.format.as_str())
                .unwrap_or(usize::MAX)
        });

        let dedupe_scope = DedupeScope::parse(&raw.dedupe.inject_once_per);
        let session_ttl_secs = raw
            .dedupe
            .session_ttl_seconds
            .filter(|v| *v > 0)
            .unwrap_or(900) as u64;

        let remote = build_remote_spec(&raw)?;
        let (annotate_sse, upstream_timeout_ms) = build_sse_settings(&raw)?;

        Ok(PolicyConfig {
            delimiter: raw.envelope.delimiter,
            sanitize_upstream: raw.envelope.sanitize_upstream_delimiter,
            annotate_sse,
            upstream_timeout_ms,
            contracts,
            merge_order: order,
            remote,
            global_max_tokens: raw.merge.global_max_tokens as usize,
            duplicate_first_wins: raw.merge.duplicate_rule_ids != "lastWins",
            dedupe_scope,
            session_ttl_secs,
            warn_on_uncovered_tools: raw.warn_on_uncovered_tools.unwrap_or(true),
        })
    }

    /// Contracts governing a tool, in merge order, with the fetched contract
    /// placed by its format precedence rather than appended.
    pub fn contracts_for<'a>(
        &'a self,
        remote: Option<&'a Contract>,
        tool: &str,
    ) -> Vec<&'a Contract> {
        let mut out: Vec<&Contract> = self.contracts.iter().filter(|c| c.covers(tool)).collect();

        if let Some(r) = remote.filter(|r| r.covers(tool)) {
            let rank = |c: &Contract| {
                self.merge_order
                    .iter()
                    .position(|f| f == c.format.as_str())
                    .unwrap_or(usize::MAX)
            };
            let pos = out
                .iter()
                .position(|c| rank(c) > rank(r))
                .unwrap_or(out.len());
            out.insert(pos, r);
        }
        out
    }
}

/// Resolves SSE handling and the upstream timeout it implies.
///
/// Under `passThrough` the timeout is removed outright, which is what lets a
/// long MCP call stream for as long as it needs. Under `annotate` the filter
/// waits for end of stream, so an unbounded timeout would let an upstream that
/// never closes stall the response forever; the configured bound is the escape
/// hatch, and zero would reintroduce exactly the hang it exists to prevent.
fn build_sse_settings(raw: &RawConfig) -> Result<(bool, String), ConfigError> {
    let settings = raw.sse.clone();
    let mode = settings
        .as_ref()
        .and_then(|s| s.mode.clone())
        .unwrap_or_else(|| "passThrough".to_string());

    match mode.as_str() {
        "passThrough" => Ok((false, "0".to_string())),
        "annotate" => {
            let timeout = settings
                .as_ref()
                .and_then(|s| s.stream_timeout_millis)
                .unwrap_or(60_000);
            if timeout <= 0 {
                return Err(ConfigError::Invalid(format!(
                    "sse.streamTimeoutMillis must be greater than 0 when sse.mode is \
                     `annotate`, got {}; an unbounded wait on a stream that never closes \
                     would stall the response indefinitely",
                    timeout
                )));
            }
            Ok((true, timeout.to_string()))
        }
        other => Err(ConfigError::Invalid(format!(
            "sse.mode must be `passThrough` or `annotate`, got `{}`",
            other
        ))),
    }
}

fn build_remote_spec(raw: &RawConfig) -> Result<Option<RemoteSpec>, ConfigError> {
    let Some(service) = raw.remote_contract_url.clone() else {
        return Ok(None);
    };

    let settings = raw.remote_contract.clone();
    let integrity = settings
        .as_ref()
        .and_then(|s| s.integrity.clone())
        .unwrap_or_default();

    // Fail closed on an unpinned artifact: a gateway that pins descriptors
    // against injection must not itself be an unpinned injection vector.
    if integrity.trim().is_empty() {
        return Err(ConfigError::Invalid(
            "remoteContractUrl is set but remoteContract.integrity is missing; \
             a fetched contract must be pinned with sha256:<64 hex chars>"
                .to_string(),
        ));
    }
    if contract::parse_integrity_pin(&integrity).is_none() {
        return Err(ConfigError::Invalid(format!(
            "remoteContract.integrity `{}` is not of the form sha256:<64 hex chars>",
            integrity
        )));
    }

    Ok(Some(RemoteSpec {
        contract_id: settings
            .as_ref()
            .and_then(|s| s.contract_id.clone())
            .unwrap_or_else(|| "remote".to_string()),
        service,
        integrity,
        ttl_secs: settings
            .as_ref()
            .and_then(|s| s.cache_ttl_seconds)
            .filter(|v| *v > 0)
            .unwrap_or(900) as u64,
        use_stale: settings
            .as_ref()
            .and_then(|s| s.on_fetch_failure.clone())
            .map(|v| v != "passThrough")
            .unwrap_or(true),
        tools: settings
            .as_ref()
            .and_then(|s| s.tool_mapping.clone())
            .unwrap_or_default(),
        severity: settings
            .as_ref()
            .and_then(|s| s.severity.as_deref().and_then(Severity::parse))
            .unwrap_or(Severity::Warn),
    }))
}

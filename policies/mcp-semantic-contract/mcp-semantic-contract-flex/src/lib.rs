// Copyright 2026 Salesforce, Inc. All rights reserved.

//! MCP Semantic Contract — deterministic response interpretation at the gateway.
//!
//! Request phase: correlate JSON-RPC `id` to `params.name` for every
//! `tools/call` in the body. Response phase: for each result, bind `payload`,
//! evaluate the bound contracts' rules, and append matched guidance as one
//! extra text element in `result.content[]`.
//!
//! Runtime posture is fail-open throughout. Any failure — unparseable body,
//! missing correlation, unbindable payload — passes the response through
//! unmodified and emits a metric. Configuration errors are the exception and
//! fail loudly at load time.

mod config;
mod contract;
mod expr;
// Emitted by `cargo anypoint config-gen` and overwritten on every build, so it
// is not ours to tidy.
#[allow(clippy::all)]
mod generated;
mod inject;
mod mcp;
mod metrics;
mod sha256;
mod sse;
mod state;

#[cfg(test)]
mod tests;

use crate::config::{PolicyConfig, RemoteSpec};
use crate::contract::{Binding, Cond, Contract};
use crate::inject::Fired;
use crate::state::{CallContext, DedupeScope, TimeSource};

use anyhow::{anyhow, Result};
use futures::join;
use pdk::data_storage::{DataStorage, DataStorageBuilder};
use pdk::hl::timer::Clock;
use pdk::hl::*;
use pdk::logger;
use pdk::metadata::Metadata;
use pdk::script::PayloadBinding;
use serde_json::Value;
use std::cell::RefCell;
use std::time::Duration;

const TIMEOUT_HEADER: &str = "x-envoy-upstream-rq-timeout-ms";
const CLIENT_ID_HEADER: &str = "x-client-id";
const STORAGE_NAMESPACE: &str = "mcp-semantic-contract";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// The fetched contract, swapped atomically on each successful refresh.
/// Single-threaded runtime, so a `RefCell` is the whole synchronisation story;
/// the borrow is never held across an `await`.
#[derive(Default)]
struct RemoteStore {
    contract: RefCell<Option<Contract>>,
}

impl RemoteStore {
    fn snapshot(&self) -> Option<Contract> {
        self.contract.borrow().clone()
    }

    fn replace(&self, contract: Option<Contract>) {
        *self.contract.borrow_mut() = contract;
    }
}

// ---------------------------------------------------------------------------
// Request phase
// ---------------------------------------------------------------------------

async fn request_filter(request_state: RequestState, config: &PolicyConfig) -> Flow<CallContext> {
    let headers_state = request_state.into_headers_state().await;
    let handler = headers_state.handler();

    // MCP calls can be long-running and may answer over SSE, so the timeout is
    // normally removed outright. Annotating a stream means waiting for it to
    // close, which needs a bound instead.
    handler.set_header(TIMEOUT_HEADER, &config.upstream_timeout_ms);

    if headers_state.method().as_str() != mcp::POST_METHOD {
        return Flow::Continue(CallContext::default());
    }

    let is_json = handler
        .header(mcp::CONTENT_TYPE_HEADER)
        .map(|ct| ct.to_ascii_lowercase().contains("json"))
        .unwrap_or(false);
    if !is_json {
        return Flow::Continue(CallContext::default());
    }

    // Everything needed from the headers must be read before crossing into
    // the body state; the transition consumes the headers state.
    let session_id = handler
        .header(mcp::MCP_SESSION_ID_HEADER)
        .filter(|s| !s.is_empty());
    let subject = handler.header(CLIENT_ID_HEADER).filter(|s| !s.is_empty());

    let body_state = headers_state.into_body_state().await;
    let bytes = body_state.as_bytes();

    let (correlations, lists_tools) = match serde_json::from_slice::<Value>(bytes.as_slice()) {
        Ok(body) => (
            mcp::correlate_request(&body),
            mcp::requests_tools_list(&body),
        ),
        // Not JSON-RPC we recognise. Nothing to correlate, nothing to annotate.
        Err(_) => (Vec::new(), false),
    };

    Flow::Continue(CallContext {
        correlations,
        lists_tools,
        session_id,
        subject,
    })
}

// ---------------------------------------------------------------------------
// Response phase
// ---------------------------------------------------------------------------

async fn response_filter(
    response_state: ResponseState,
    request_data: RequestData<CallContext>,
    config: &PolicyConfig,
    storage: &impl DataStorage,
    remote_store: &RemoteStore,
    time: &TimeSource,
    asset_id: &str,
) {
    let RequestData::Continue(ctx) = request_data else {
        // The request was rejected or cancelled upstream; there is no tool
        // result to interpret.
        return;
    };
    if ctx.is_empty() {
        return;
    }

    let headers_state = response_state.into_headers_state().await;
    let content_type = headers_state
        .handler()
        .header(mcp::CONTENT_TYPE_HEADER)
        .unwrap_or_default()
        .to_ascii_lowercase();

    let is_sse = content_type.starts_with("text/event-stream");

    if is_sse && !config.annotate_sse {
        for (_, tool) in &ctx.correlations {
            metrics::count(metrics::SSE_SKIPPED, asset_id, tool, "");
        }
        return;
    }
    if !is_sse && !content_type.contains("json") {
        return;
    }

    // The body is about to change length; the gateway must recompute it.
    headers_state
        .handler()
        .remove_header(mcp::CONTENT_LENGTH_HEADER);

    // Reaching the body state means waiting for end of stream. Under
    // `annotate` that wait is bounded by the timeout set on the request.
    let body_state = headers_state.into_body_state().await;
    let bytes = body_state.as_bytes();
    if bytes.as_slice().is_empty() {
        return;
    }

    // Snapshot the fetched contract once per response; the refresh task must
    // stay free to swap it while this filter awaits.
    let remote = remote_store.snapshot();

    let rewritten = if is_sse {
        annotate_stream(
            bytes.as_slice(),
            &ctx,
            config,
            remote.as_ref(),
            storage,
            time,
            asset_id,
        )
        .await
    } else {
        annotate_json(
            bytes.as_slice(),
            &ctx,
            config,
            remote.as_ref(),
            storage,
            time,
            asset_id,
        )
        .await
    };

    let Some(out) = rewritten else {
        return;
    };

    if let Err(e) = body_state.handler().set_body(&out) {
        logger::warn!(
            "semantic_contract: set_body failed ({:?}); response unchanged",
            e
        );
        metrics::count(
            metrics::PASSTHROUGH_ON_ERROR,
            asset_id,
            "-",
            "reason=set_body",
        );
    }
}

/// Annotates a JSON body, single or batched. `None` leaves it untouched.
async fn annotate_json(
    bytes: &[u8],
    ctx: &CallContext,
    config: &PolicyConfig,
    remote: Option<&Contract>,
    storage: &impl DataStorage,
    time: &TimeSource,
    asset_id: &str,
) -> Option<Vec<u8>> {
    let mut body: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            logger::debug!(
                "semantic_contract: response body is not JSON ({}); passing through",
                e
            );
            metrics::count(
                metrics::PASSTHROUGH_ON_ERROR,
                asset_id,
                "-",
                "reason=body_parse",
            );
            return None;
        }
    };

    if !annotate_payload(&mut body, ctx, config, remote, storage, time, asset_id).await {
        return None;
    }

    match serde_json::to_vec(&body) {
        Ok(out) => Some(out),
        Err(e) => {
            logger::warn!(
                "semantic_contract: re-serialisation failed ({}); response unchanged",
                e
            );
            metrics::count(
                metrics::PASSTHROUGH_ON_ERROR,
                asset_id,
                "-",
                "reason=serialize",
            );
            None
        }
    }
}

/// Annotates the JSON-RPC payload of every SSE frame that carries one, leaving
/// the framing and any non-JSON frame exactly as received.
async fn annotate_stream(
    bytes: &[u8],
    ctx: &CallContext,
    config: &PolicyConfig,
    remote: Option<&Contract>,
    storage: &impl DataStorage,
    time: &TimeSource,
    asset_id: &str,
) -> Option<Vec<u8>> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        logger::debug!("semantic_contract: event stream is not UTF-8; passing through");
        metrics::count(
            metrics::PASSTHROUGH_ON_ERROR,
            asset_id,
            "-",
            "reason=sse_encoding",
        );
        return None;
    };

    let mut stream = sse::Stream::parse(text);
    let mut modified = false;

    for (payload, rewritten) in stream.payloads_mut() {
        if annotate_payload(payload, ctx, config, remote, storage, time, asset_id).await {
            *rewritten = true;
            modified = true;
        }
    }

    modified.then(|| stream.render().into_bytes())
}

/// Annotates one JSON-RPC payload, which may be a single message or a batch.
async fn annotate_payload(
    body: &mut Value,
    ctx: &CallContext,
    config: &PolicyConfig,
    remote: Option<&Contract>,
    storage: &impl DataStorage,
    time: &TimeSource,
    asset_id: &str,
) -> bool {
    match body {
        Value::Array(batch) => {
            let mut modified = false;
            for msg in batch.iter_mut() {
                modified |=
                    annotate_message(msg, ctx, config, remote, storage, time, asset_id).await;
            }
            modified
        }
        single => annotate_message(single, ctx, config, remote, storage, time, asset_id).await,
    }
}

/// Annotates one JSON-RPC response object in place. Returns whether anything
/// changed.
async fn annotate_message(
    msg: &mut Value,
    ctx: &CallContext,
    config: &PolicyConfig,
    remote: Option<&Contract>,
    storage: &impl DataStorage,
    time: &TimeSource,
    asset_id: &str,
) -> bool {
    let Some(id_key) = msg.get("id").and_then(mcp::id_key) else {
        return false;
    };

    if msg.get("error").map(|e| !e.is_null()).unwrap_or(false) {
        return false;
    }

    // Descriptors are handled before correlation: a `tools/list` id was never
    // paired with a tool name, so the lookup below would discard it.
    if msg
        .get("result")
        .map(mcp::is_tools_list_result)
        .unwrap_or(false)
    {
        let Some(result) = msg.get_mut("result") else {
            return false;
        };
        return inject::declare_in_tools_list(result, |name| {
            !config.contracts_for(remote, name).is_empty()
        }) > 0;
    }

    let Some(tool) = ctx.tool_for(&id_key).map(str::to_string) else {
        return false;
    };

    let Some(result) = msg.get_mut("result").filter(|r| r.is_object()) else {
        return false;
    };

    // The gateway owns its structured field on every path. Stripping it here
    // rather than only when guidance is written means an upstream cannot leave
    // a forged block behind by arranging for no rule to fire, or by returning
    // an error, or by naming a tool no contract covers.
    let stripped = inject::clear_structured(result);
    if stripped {
        metrics::count(
            metrics::DELIMITER_SANITIZED,
            asset_id,
            &tool,
            "forgedContractField=1",
        );
    }

    if mcp::is_error_result(result) {
        return stripped;
    }

    let contracts = config.contracts_for(remote, &tool);
    if contracts.is_empty() {
        if config.warn_on_uncovered_tools {
            logger::warn!(
                "semantic_contract: tool `{}` on asset `{}` is not covered by any contract",
                tool,
                asset_id
            );
        }
        return stripped;
    }

    let (payload, binding) = contract::bind_payload(result);
    if binding == Binding::Unbindable {
        metrics::count(metrics::PAYLOAD_UNBINDABLE, asset_id, &tool, "");
    }

    // Select rules in merge order, first or last occurrence of a duplicated
    // rule id winning per configuration.
    let mut fired: Vec<Fired> = Vec::new();
    for c in &contracts {
        for rule in &c.rules {
            let matched = match &rule.cond {
                Cond::Always => true,
                // An unbindable payload makes every conditional rule false.
                Cond::When(_) if binding == Binding::Unbindable => false,
                Cond::When(ast) => expr::eval(ast, &payload),
            };
            if !matched {
                continue;
            }

            if let Some(pos) = fired.iter().position(|f| f.rule.id == rule.id) {
                if config.duplicate_first_wins {
                    continue;
                }
                fired.remove(pos);
            }

            if config.dedupe_scope == DedupeScope::Session
                && state::already_delivered(
                    storage,
                    ctx,
                    asset_id,
                    &c.contract_id,
                    &rule.id,
                    config.session_ttl_secs,
                    time,
                )
                .await
            {
                metrics::count(
                    metrics::RULE_DEDUPED,
                    asset_id,
                    &tool,
                    &format!("ruleId={}", rule.id),
                );
                continue;
            }

            fired.push(Fired {
                contract_id: c.contract_id.clone(),
                rule: rule.clone(),
            });
        }
    }

    if fired.is_empty() {
        return stripped;
    }

    let outcome = inject::build(fired, &config.delimiter, config.global_max_tokens);

    for (_, rule_id, severity) in &outcome.dropped_for_budget {
        metrics::count(
            metrics::RULES_DROPPED_BUDGET,
            asset_id,
            &tool,
            &format!("ruleId={} severity={}", rule_id, severity.as_str()),
        );
    }
    if outcome.critical_over_budget {
        metrics::count(metrics::CRITICAL_OVER_BUDGET, asset_id, &tool, "");
    }

    let Some(block) = outcome.block else {
        return stripped;
    };

    // A compromised upstream must not be able to forge a trusted block, so the
    // delimiter is escaped everywhere it already appears before ours is added.
    if config.sanitize_upstream {
        let mut elements = 0;
        if let Some(Value::Array(content)) = result.get_mut("content") {
            elements = inject::sanitize_upstream(content, &config.delimiter);
        }
        // structuredContent is a second copy of the same document, and the one
        // a schema-aware client reads first. It has to be defanged too.
        let mut structured = 0;
        if let Some(sc) = result.get_mut("structuredContent") {
            structured = inject::sanitize_structured(sc, &config.delimiter);
        }
        if elements > 0 || structured > 0 {
            metrics::count(
                metrics::DELIMITER_SANITIZED,
                asset_id,
                &tool,
                &format!("elements={} structuredStrings={}", elements, structured),
            );
        }
    }

    // Both channels, because they serve different clients: schema-aware ones
    // read structuredContent and may discard extra content elements, while
    // clients predating structured output only ever look at content[].
    let structured = inject::write_structured(result, &outcome.lines);
    inject::append_block(result, block);
    metrics::count(
        metrics::GUIDANCE_DELIVERED,
        asset_id,
        &tool,
        &format!("structured={} content=1", structured),
    );

    for (contract_id, rule_id, severity) in &outcome.kept {
        metrics::count(
            metrics::RULE_FIRED,
            asset_id,
            &tool,
            &format!(
                "contractId={} ruleId={} severity={}",
                contract_id,
                rule_id,
                severity.as_str()
            ),
        );
    }

    true
}

// ---------------------------------------------------------------------------
// Remote contract fetching
// ---------------------------------------------------------------------------

/// Fetches, verifies and parses the pinned artifact. Never returns unverified
/// content: an integrity mismatch drops the contract entirely.
async fn fetch_remote(client: &HttpClient, spec: &RemoteSpec) -> Result<Contract, String> {
    let response = client
        .request(&spec.service)
        .timeout(FETCH_TIMEOUT)
        .headers(vec![("accept", "*/*")])
        .get()
        .await
        .map_err(|e| format!("fetch failed: {:?}", e))?;

    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(format!("fetch returned HTTP {}", response.status_code()));
    }

    let body = response.body();
    contract::verify_integrity(&spec.integrity, body).map_err(|e| e.to_string())?;

    let text = std::str::from_utf8(body).map_err(|_| "artifact is not valid UTF-8".to_string())?;
    let content_type = response
        .headers()
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.clone());

    let loaded = contract::load_fetched(
        text,
        content_type.as_deref(),
        &spec.tools,
        spec.severity,
        &spec.contract_id,
    )
    .map_err(|e| e.to_string())?;

    for w in &loaded.warnings {
        logger::warn!("semantic_contract: remote contract load warning: {:?}", w);
    }

    Ok(loaded.contract)
}

async fn refresh_remote(client: &HttpClient, spec: &RemoteSpec, store: &RemoteStore) {
    match fetch_remote(client, spec).await {
        Ok(contract) => {
            logger::info!(
                "semantic_contract: fetched contract `{}` v{} governing {} tool(s)",
                contract.contract_id,
                contract.version,
                contract.tools.len()
            );
            store.replace(Some(contract));
        }
        Err(reason) => {
            metrics::count_global(
                metrics::CONTRACT_LOAD_FAILED,
                &format!("contractId={} scope=remote", spec.contract_id),
            );
            if spec.use_stale && store.snapshot().is_some() {
                logger::warn!(
                    "semantic_contract: refresh of contract `{}` failed ({}); \
                     keeping the last verified copy",
                    spec.contract_id,
                    reason
                );
            } else {
                logger::warn!(
                    "semantic_contract: contract `{}` unavailable ({}); it governs nothing \
                     until the next successful fetch",
                    spec.contract_id,
                    reason
                );
                store.replace(None);
            }
        }
    }
}

/// Refetches on TTL expiry. Contracts are never loaded on the request path.
async fn refresh_loop(
    timer: &pdk::hl::timer::Timer,
    client: &HttpClient,
    spec: &RemoteSpec,
    store: &RemoteStore,
) {
    while timer.next_tick().await {
        refresh_remote(client, spec, store).await;
    }
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[entrypoint]
async fn configure(
    launcher: Launcher,
    Configuration(bytes): Configuration,
    store_builder: DataStorageBuilder,
    metadata: Metadata,
    client: HttpClient,
    clock: Clock,
) -> Result<()> {
    let config = PolicyConfig::load(&bytes).map_err(|err| {
        anyhow!(
            "Failed to parse configuration '{}'. Cause: {}",
            String::from_utf8_lossy(&bytes),
            err
        )
    })?;

    // Absent when the gateway runs without a control-plane connection; the
    // policy still governs traffic, the metric tag is just less useful.
    let asset_id = metadata
        .api_metadata
        .id
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    logger::info!(
        "semantic_contract: loaded {} contract(s) governing {} tool binding(s) on asset `{}`",
        config.contracts.len(),
        config
            .contracts
            .iter()
            .map(|c| c.tools.len())
            .sum::<usize>(),
        asset_id
    );

    let storage = store_builder.local(STORAGE_NAMESPACE.to_string());
    let remote_store = RemoteStore::default();

    // A tick period is only worth setting when something actually needs to
    // wake up; without a remote contract the clock stays idle.
    let time = match &config.remote {
        Some(spec) => TimeSource::Ticking(clock.period(Duration::from_secs(spec.ttl_secs))),
        None => TimeSource::Idle(clock),
    };

    // Load the pinned artifact once before serving, so the first request is
    // governed by the same rule set as the thousandth.
    if let Some(spec) = &config.remote {
        refresh_remote(&client, spec, &remote_store).await;
    }

    let filter = on_request(|rs| request_filter(rs, &config)).on_response(|rs, rd| {
        response_filter(rs, rd, &config, &storage, &remote_store, &time, &asset_id)
    });

    match (&config.remote, time.timer()) {
        (Some(spec), Some(timer)) => {
            // Both futures must progress; proxy-wasm interleaves them only at
            // await points, so no locking is involved.
            let joined = join!(
                launcher.launch(filter),
                refresh_loop(timer, &client, spec, &remote_store)
            );
            joined.0?;
        }
        _ => {
            launcher.launch(filter).await?;
        }
    }
    Ok(())
}

// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Per-transaction correlation and cross-call dedupe.
//!
//! Correlation is carried on `RequestData<CallContext>`, which PDK scopes to a
//! single HTTP transaction — exactly the lifetime the JSON-RPC `id` is unique
//! for. No shared store is involved, so batch requests are handled by carrying
//! N entries instead of one.

use pdk::data_storage::{DataStorage, StoreMode};
use pdk::hl::timer::{Clock, Timer};
use pdk::logger;
use serde::{Deserialize, Serialize};
use std::time::UNIX_EPOCH;

/// What the request filter hands to the response filter.
#[derive(Debug, Clone, Default)]
pub struct CallContext {
    /// `id` → tool name, one entry per `tools/call` in the request body.
    pub correlations: Vec<(String, String)>,
    /// Whether the request asked for tool descriptors. Those responses carry no
    /// correlatable id but still need the gateway's field declared on them.
    pub lists_tools: bool,
    /// `Mcp-Session-Id`, when the client sent one.
    pub session_id: Option<String>,
    /// Authenticated subject, used as the dedupe key when no session id exists.
    pub subject: Option<String>,
}

impl CallContext {
    pub fn tool_for(&self, id_key: &str) -> Option<&str> {
        self.correlations
            .iter()
            .find(|(k, _)| k == id_key)
            .map(|(_, tool)| tool.as_str())
    }

    /// Whether the response is worth buffering at all.
    pub fn is_empty(&self) -> bool {
        self.correlations.is_empty() && !self.lists_tools
    }
}

/// Dedupe scope from `dedupe.injectOncePer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupeScope {
    /// Every matching rule is injected on every call.
    Call,
    /// A rule already delivered to this session is suppressed until the TTL
    /// expires.
    Session,
}

impl DedupeScope {
    pub fn parse(s: &str) -> DedupeScope {
        match s {
            "session" => DedupeScope::Session,
            _ => DedupeScope::Call,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SeenAt {
    unix_secs: u64,
}

/// The gateway's clock, which `Clock::period` consumes to build a `Timer`.
/// Both halves can still report the time, so dedupe reads it from whichever
/// one this deployment ended up with rather than from `SystemTime::now()`,
/// which ignores the injected clock and is untestable.
pub enum TimeSource {
    Idle(Clock),
    Ticking(Timer),
}

impl TimeSource {
    pub fn now_secs(&self) -> u64 {
        let now = match self {
            TimeSource::Idle(clock) => clock.now(),
            TimeSource::Ticking(timer) => timer.now(),
        };
        now.duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn timer(&self) -> Option<&Timer> {
        match self {
            TimeSource::Ticking(timer) => Some(timer),
            TimeSource::Idle(_) => None,
        }
    }
}

/// Dedupe key. `Mcp-Session-Id` when present, otherwise the authenticated
/// subject, so a stateless deployment still suppresses repeats per caller.
fn dedupe_key(ctx: &CallContext, asset: &str, contract_id: &str, rule_id: &str) -> Option<String> {
    let scope = match (&ctx.session_id, &ctx.subject) {
        (Some(sid), _) => format!("sid:{}", sid),
        (None, Some(sub)) => format!("sub:{}", sub),
        (None, None) => return None,
    };
    Some(format!("{}|{}|{}|{}", scope, asset, contract_id, rule_id))
}

/// Returns true when this rule has already been delivered within the TTL, and
/// records it otherwise.
///
/// Storage errors resolve to "not seen": a dedupe failure must degrade to
/// injecting a rule twice, never to withholding guidance.
pub async fn already_delivered(
    storage: &impl DataStorage,
    ctx: &CallContext,
    asset: &str,
    contract_id: &str,
    rule_id: &str,
    ttl_secs: u64,
    time: &TimeSource,
) -> bool {
    let Some(key) = dedupe_key(ctx, asset, contract_id, rule_id) else {
        logger::warn!(
            "semantic_contract: dedupe scope `session` requested but the request carries \
             neither Mcp-Session-Id nor an authenticated subject; injecting without dedupe"
        );
        return false;
    };

    let now = time.now_secs();

    match storage.get::<SeenAt>(&key).await {
        Ok(Some((seen, version))) => {
            if now.saturating_sub(seen.unix_secs) < ttl_secs {
                return true;
            }
            let _ = storage
                .store(&key, &StoreMode::Cas(version), &SeenAt { unix_secs: now })
                .await;
            false
        }
        Ok(None) => {
            let _ = storage
                .store(&key, &StoreMode::Absent, &SeenAt { unix_secs: now })
                .await;
            false
        }
        Err(e) => {
            logger::warn!(
                "semantic_contract: dedupe store read failed ({:?}); injecting",
                e
            );
            false
        }
    }
}

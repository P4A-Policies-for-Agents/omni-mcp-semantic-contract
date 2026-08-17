// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Contract model and the loaders that produce it.
//!
//! A contract is parsed once, at policy init or on cache expiry, into a
//! [`Contract`] whose `when` expressions are already compiled to an AST. The
//! request path never parses a contract or an expression.

use crate::expr::{self, Expr};
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warn,
    Critical,
}

impl Severity {
    pub fn parse(s: &str) -> Option<Severity> {
        match s {
            "info" => Some(Severity::Info),
            "warn" => Some(Severity::Warn),
            "critical" => Some(Severity::Critical),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Markdown,
    Text,
    Url,
}

impl Format {
    pub fn parse(s: &str) -> Option<Format> {
        match s {
            "json" => Some(Format::Json),
            "markdown" => Some(Format::Markdown),
            "text" => Some(Format::Text),
            "url" => Some(Format::Url),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Markdown => "markdown",
            Format::Text => "text",
            Format::Url => "url",
        }
    }
}

/// When a rule fires.
#[derive(Debug, Clone)]
pub enum Cond {
    /// Fires on every matched call.
    Always,
    /// Fires when the compiled expression evaluates true against the payload.
    When(Expr),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub severity: Severity,
    pub guidance: String,
    pub cond: Cond,
}

impl Rule {
    /// Approximate token cost of this rule's line in the injected block.
    /// Characters divided by four, per the policy's budget definition.
    pub fn token_cost(&self) -> usize {
        // "<id>: <guidance>\n"
        (self.id.len() + 2 + self.guidance.len() + 1).div_ceil(4)
    }
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub contract_id: String,
    pub version: String,
    pub format: Format,
    /// Tools this contract governs. Guaranteed non-empty by the loader.
    pub tools: Vec<String>,
    pub rules: Vec<Rule>,
}

impl Contract {
    pub fn covers(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }
}

/// A non-fatal problem found while loading a contract. Each maps to a metric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadWarning {
    /// A `when` expression failed to parse. The rule is disabled; the rest of
    /// the contract loads normally.
    BadExpression { rule_id: String, reason: String },
    /// A rule was structurally invalid and was skipped.
    BadRule { rule_id: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError(pub String);

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
pub struct Loaded {
    pub contract: Contract,
    pub warnings: Vec<LoadWarning>,
}

// ---------------------------------------------------------------------------
// JSON loader
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct JsonContract {
    #[serde(rename = "semanticContractVersion")]
    semantic_contract_version: String,
    #[serde(rename = "contractId")]
    contract_id: String,
    version: String,
    #[serde(rename = "toolMapping")]
    tool_mapping: Vec<String>,
    rules: Vec<JsonRule>,
}

#[derive(Deserialize)]
struct JsonRule {
    id: String,
    severity: String,
    #[serde(default)]
    when: Option<String>,
    #[serde(default)]
    always: Option<bool>,
    guidance: String,
}

/// Loads a `format: json` contract. `tool_override`, when non-empty, replaces
/// the artifact's own `toolMapping`.
pub fn load_json(
    src: &str,
    tool_override: &[String],
    contract_id_override: &str,
) -> Result<Loaded, LoadError> {
    let doc: JsonContract = serde_json::from_str(src).map_err(|e| {
        LoadError(format!(
            "contract is not valid JSON against semantic-contract-v1: {}",
            e
        ))
    })?;

    if doc.semantic_contract_version != "1.0" {
        return Err(LoadError(format!(
            "unsupported semanticContractVersion `{}`; this policy implements 1.0",
            doc.semantic_contract_version
        )));
    }

    let tools = if tool_override.is_empty() {
        doc.tool_mapping.clone()
    } else {
        tool_override.to_vec()
    };
    if tools.is_empty() {
        return Err(LoadError(
            "contract resolves to an empty tool mapping".to_string(),
        ));
    }

    let mut rules = Vec::new();
    let mut warnings = Vec::new();

    for r in doc.rules {
        let severity = match Severity::parse(&r.severity) {
            Some(s) => s,
            None => {
                warnings.push(LoadWarning::BadRule {
                    rule_id: r.id.clone(),
                    reason: format!("unknown severity `{}`", r.severity),
                });
                continue;
            }
        };

        let cond = match (r.when.as_deref(), r.always) {
            (Some(_), Some(true)) => {
                warnings.push(LoadWarning::BadRule {
                    rule_id: r.id.clone(),
                    reason: "`when` and `always` are mutually exclusive".to_string(),
                });
                continue;
            }
            (None, Some(true)) => Cond::Always,
            (Some(src), _) => match expr::parse(src) {
                Ok(ast) => Cond::When(ast),
                Err(e) => {
                    // A malformed expression disables this rule only.
                    warnings.push(LoadWarning::BadExpression {
                        rule_id: r.id.clone(),
                        reason: e.0,
                    });
                    continue;
                }
            },
            (None, _) => {
                warnings.push(LoadWarning::BadRule {
                    rule_id: r.id.clone(),
                    reason: "rule has neither `when` nor `always`".to_string(),
                });
                continue;
            }
        };

        rules.push(Rule {
            id: r.id,
            severity,
            guidance: r.guidance,
            cond,
        });
    }

    let contract_id = if contract_id_override.is_empty() {
        doc.contract_id
    } else {
        contract_id_override.to_string()
    };

    Ok(Loaded {
        contract: Contract {
            contract_id,
            version: doc.version,
            format: Format::Json,
            tools,
            rules,
        },
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Markdown loader
// ---------------------------------------------------------------------------

/// Loads a `format: markdown` contract: YAML frontmatter for metadata and
/// tool mapping, the whole body as one unconditional rule.
///
/// The frontmatter parser deliberately supports only the flat subset this
/// policy needs — `key: scalar` and `key:` followed by `- item` lines. Nested
/// mappings, anchors, multi-line scalars and flow style are not supported;
/// anything else in the frontmatter is ignored.
pub fn load_markdown(
    src: &str,
    tool_override: &[String],
    severity: Severity,
    contract_id_override: &str,
) -> Result<Loaded, LoadError> {
    let (front, body) = split_frontmatter(src)
        .ok_or_else(|| LoadError("markdown contract has no `---` YAML frontmatter".to_string()))?;

    let meta = parse_frontmatter(front);

    let tools = if !tool_override.is_empty() {
        tool_override.to_vec()
    } else {
        meta.list("toolMapping")
    };
    if tools.is_empty() {
        return Err(LoadError(
            "markdown contract resolves to an empty tool mapping".to_string(),
        ));
    }

    let body = body.trim();
    if body.is_empty() {
        return Err(LoadError("markdown contract body is empty".to_string()));
    }

    let contract_id = if !contract_id_override.is_empty() {
        contract_id_override.to_string()
    } else {
        meta.scalar("contractId")
            .unwrap_or_else(|| "markdown-contract".to_string())
    };
    let version = meta
        .scalar("version")
        .unwrap_or_else(|| "0.0.0".to_string());
    let severity = meta
        .scalar("severity")
        .and_then(|s| Severity::parse(&s))
        .unwrap_or(severity);

    // Synthetic id: the whole body is one rule, so it needs a stable name.
    let rule_id = format!("{}-body", contract_id);

    Ok(Loaded {
        contract: Contract {
            contract_id,
            version,
            format: Format::Markdown,
            tools,
            rules: vec![Rule {
                id: rule_id,
                severity,
                guidance: collapse_whitespace(body),
                cond: Cond::Always,
            }],
        },
        warnings: Vec::new(),
    })
}

fn split_frontmatter(src: &str) -> Option<(&str, &str)> {
    let rest = src.strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let end = rest.find("\n---")?;
    let front = &rest[..end];
    let after = &rest[end + 4..];
    let body = after
        .strip_prefix('\n')
        .or_else(|| after.strip_prefix("\r\n"))
        .unwrap_or(after);
    Some((front, body))
}

#[derive(Default)]
struct Frontmatter {
    scalars: Vec<(String, String)>,
    lists: Vec<(String, Vec<String>)>,
}

impl Frontmatter {
    fn scalar(&self, key: &str) -> Option<String> {
        self.scalars
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    fn list(&self, key: &str) -> Vec<String> {
        self.lists
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
}

fn parse_frontmatter(front: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();
    let mut pending_list: Option<(String, Vec<String>)> = None;

    for raw in front.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some((_, items)) = pending_list.as_mut() {
                items.push(unquote(item.trim()));
            }
            continue;
        }

        if let Some((key, list)) = pending_list.take() {
            fm.lists.push((key, list));
        }

        if let Some(idx) = trimmed.find(':') {
            let key = trimmed[..idx].trim().to_string();
            let val = trimmed[idx + 1..].trim();
            if val.is_empty() {
                pending_list = Some((key, Vec::new()));
            } else if val.starts_with('[') && val.ends_with(']') {
                let items = val[1..val.len() - 1]
                    .split(',')
                    .map(|s| unquote(s.trim()))
                    .filter(|s| !s.is_empty())
                    .collect();
                fm.lists.push((key, items));
            } else {
                fm.scalars.push((key, unquote(val)));
            }
        }
    }

    if let Some((key, list)) = pending_list.take() {
        fm.lists.push((key, list));
    }

    fm
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Injected guidance is a single text line per rule, so prose contracts have
/// their line structure flattened.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Text loader
// ---------------------------------------------------------------------------

/// Loads a `format: text` contract. No metadata: tool mapping, severity and
/// contract id all come from the binding config.
pub fn load_text(
    src: &str,
    tool_override: &[String],
    severity: Severity,
    contract_id: &str,
) -> Result<Loaded, LoadError> {
    if tool_override.is_empty() {
        return Err(LoadError(
            "a text contract has no tool mapping of its own; set `toolMapping` on the binding"
                .to_string(),
        ));
    }
    let body = src.trim();
    if body.is_empty() {
        return Err(LoadError("text contract is empty".to_string()));
    }

    Ok(Loaded {
        contract: Contract {
            contract_id: contract_id.to_string(),
            version: "0.0.0".to_string(),
            format: Format::Text,
            tools: tool_override.to_vec(),
            rules: vec![Rule {
                id: format!("{}-body", contract_id),
                severity,
                guidance: collapse_whitespace(body),
                cond: Cond::Always,
            }],
        },
        warnings: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Fetched (url) contracts
// ---------------------------------------------------------------------------

/// Dispatches fetched bytes to the loader matching the response content type.
/// `format: url` inherits the capability tier of whatever it fetched.
pub fn load_fetched(
    body: &str,
    content_type: Option<&str>,
    tool_override: &[String],
    severity: Severity,
    contract_id: &str,
) -> Result<Loaded, LoadError> {
    let ct = content_type.unwrap_or("").to_ascii_lowercase();
    let mut loaded = if ct.contains("json") || body.trim_start().starts_with('{') {
        load_json(body, tool_override, contract_id)?
    } else if body.trim_start().starts_with("---") {
        load_markdown(body, tool_override, severity, contract_id)?
    } else {
        load_text(body, tool_override, severity, contract_id)?
    };
    loaded.contract.format = Format::Url;
    Ok(loaded)
}

/// Validates the shape of a `sha256:<64 hex>` pin and returns the hex digest.
pub fn parse_integrity_pin(pin: &str) -> Option<&str> {
    let hex = pin.strip_prefix("sha256:")?;
    if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(hex)
    } else {
        None
    }
}

/// Verifies a `sha256:<64 hex>` integrity pin against fetched bytes.
pub fn verify_integrity(pin: &str, body: &[u8]) -> Result<(), LoadError> {
    let hex = parse_integrity_pin(pin)
        .ok_or_else(|| LoadError("integrity pin must be `sha256:<64 hex chars>`".to_string()))?;
    let actual = crate::sha256::hex_digest(body);
    if actual.eq_ignore_ascii_case(hex) {
        Ok(())
    } else {
        Err(LoadError(format!(
            "integrity mismatch: expected {}, computed {}",
            hex.to_ascii_lowercase(),
            actual
        )))
    }
}

// ---------------------------------------------------------------------------
// Payload binding
// ---------------------------------------------------------------------------

/// How `payload` was bound for a given result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    StructuredContent,
    TextContent,
    /// Nothing JSON-shaped was found. Conditional rules all evaluate false and
    /// only `always` rules are injected.
    Unbindable,
}

/// Binds `payload` per the documented order of preference: structuredContent,
/// then the first `text` content element that parses as JSON.
pub fn bind_payload(result: &Value) -> (Value, Binding) {
    if let Some(sc) = result.get("structuredContent") {
        if !sc.is_null() {
            return (sc.clone(), Binding::StructuredContent);
        }
    }

    if let Some(Value::Array(content)) = result.get("content") {
        for el in content {
            if el.get("type").and_then(Value::as_str) != Some("text") {
                continue;
            }
            if let Some(text) = el.get("text").and_then(Value::as_str) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    if parsed.is_object() || parsed.is_array() {
                        return (parsed, Binding::TextContent);
                    }
                }
            }
        }
    }

    (Value::Null, Binding::Unbindable)
}

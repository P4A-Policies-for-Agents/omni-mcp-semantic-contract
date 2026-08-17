use serde::Deserialize;
#[derive(Deserialize, Clone, Debug)]
pub struct Contracts0Config {
    #[serde(alias = "contractId")]
    pub contract_id: String,
    #[serde(alias = "format")]
    pub format: String,
    #[serde(alias = "inline")]
    pub inline: Option<String>,
    #[serde(alias = "severity")]
    pub severity: Option<String>,
    #[serde(alias = "toolMapping")]
    pub tool_mapping: Option<Vec<String>>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct DedupeConfig {
    #[serde(alias = "injectOncePer")]
    pub inject_once_per: String,
    #[serde(alias = "sessionTtlSeconds")]
    pub session_ttl_seconds: Option<i64>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct EnvelopeConfig {
    #[serde(alias = "delimiter")]
    pub delimiter: String,
    #[serde(alias = "sanitizeUpstreamDelimiter")]
    pub sanitize_upstream_delimiter: bool,
}
#[derive(Deserialize, Clone, Debug)]
pub struct MergeConfig {
    #[serde(alias = "duplicateRuleIds")]
    pub duplicate_rule_ids: String,
    #[serde(alias = "globalMaxTokens")]
    pub global_max_tokens: i64,
    #[serde(alias = "onBudgetExceeded")]
    pub on_budget_exceeded: String,
    #[serde(alias = "order")]
    pub order: Option<Vec<String>>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct RemoteContractConfig {
    #[serde(alias = "cacheTtlSeconds")]
    pub cache_ttl_seconds: Option<i64>,
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
    #[serde(alias = "integrity")]
    pub integrity: Option<String>,
    #[serde(alias = "onFetchFailure")]
    pub on_fetch_failure: Option<String>,
    #[serde(alias = "severity")]
    pub severity: Option<String>,
    #[serde(alias = "toolMapping")]
    pub tool_mapping: Option<Vec<String>>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct SseConfig {
    #[serde(alias = "mode")]
    pub mode: Option<String>,
    #[serde(alias = "streamTimeoutMillis")]
    pub stream_timeout_millis: Option<i64>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(alias = "contracts")]
    pub contracts: Vec<Contracts0Config>,
    #[serde(alias = "dedupe")]
    pub dedupe: DedupeConfig,
    #[serde(alias = "envelope")]
    pub envelope: EnvelopeConfig,
    #[serde(alias = "merge")]
    pub merge: MergeConfig,
    #[serde(alias = "remoteContract")]
    pub remote_contract: Option<RemoteContractConfig>,
    #[serde(
        alias = "remoteContractUrl",
        default,
        deserialize_with = "pdk::serde::deserialize_service_opt"
    )]
    pub remote_contract_url: Option<pdk::hl::Service>,
    #[serde(alias = "sse")]
    pub sse: Option<SseConfig>,
    #[serde(alias = "warnOnUncoveredTools")]
    pub warn_on_uncovered_tools: Option<bool>,
}
#[pdk::hl::entrypoint_flex]
fn init(abi: &dyn pdk::flex_abi::api::FlexAbi) -> Result<(), anyhow::Error> {
    let config: Config = serde_json::from_slice(abi.get_configuration())
        .map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse configuration '{}'. Cause: {}",
                String::from_utf8_lossy(abi.get_configuration()), err
            )
        })?;
    if config.remote_contract_url.is_some() {
        let service = config.remote_contract_url.unwrap();
        abi.service_create(service)?;
    }
    abi.setup()?;
    Ok(())
}

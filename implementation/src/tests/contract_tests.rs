// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Contract loaders, integrity pinning and payload binding.

use crate::contract::{self, Binding, Cond, LoadWarning, Severity};
use serde_json::json;

const TOOLS: [&str; 1] = ["get_sales_order"];

fn tools() -> Vec<String> {
    TOOLS.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// JSON loader
// ---------------------------------------------------------------------------

mod json_loader {
    use super::*;

    #[test]
    fn loads_the_shipped_contract() {
        let loaded = contract::load_json(
            super::super::common::ERP_CONTRACT,
            &tools(),
            "erp-sales-order",
        )
        .unwrap();
        assert_eq!(loaded.contract.version, "3.1.0");
        assert_eq!(loaded.contract.rules.len(), 7);
        assert!(
            loaded.warnings.is_empty(),
            "the shipped contract must load clean: {:?}",
            loaded.warnings
        );
    }

    #[test]
    fn tool_mapping_comes_from_the_contract_when_config_is_silent() {
        let loaded = contract::load_json(super::super::common::ERP_CONTRACT, &[], "erp").unwrap();
        assert_eq!(loaded.contract.tools, vec!["get_sales_order".to_string()]);
    }

    #[test]
    fn the_configured_id_wins_over_the_artifacts_own() {
        // Dedupe keys and metric tags are namespaced by this id, so a fetched
        // artifact must not be able to rename itself into another contract's
        // namespace by editing its own `contractId`.
        let loaded = contract::load_json(
            super::super::common::ERP_CONTRACT,
            &tools(),
            "operator-label",
        )
        .unwrap();
        assert_eq!(loaded.contract.contract_id, "operator-label");
    }

    #[test]
    fn config_tool_mapping_overrides_the_contract() {
        let loaded = contract::load_json(
            super::super::common::ERP_CONTRACT,
            &["other_tool".to_string()],
            "erp",
        )
        .unwrap();
        assert_eq!(loaded.contract.tools, vec!["other_tool".to_string()]);
        assert!(loaded.contract.covers("other_tool"));
        assert!(!loaded.contract.covers("get_sales_order"));
    }

    #[test]
    fn a_malformed_when_disables_only_that_rule() {
        let src = json!({
            "semanticContractVersion": "1.0",
            "contractId": "c", "version": "1.0.0", "toolMapping": TOOLS,
            "rules": [
                { "id": "good", "severity": "warn", "when": "payload.a == 1", "guidance": "g" },
                { "id": "bad", "severity": "warn", "when": "payload.a === 1", "guidance": "g" },
                { "id": "also-good", "severity": "warn", "always": true, "guidance": "g" }
            ]
        })
        .to_string();

        let loaded = contract::load_json(&src, &tools(), "c").unwrap();
        let ids: Vec<&str> = loaded
            .contract
            .rules
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(ids, vec!["good", "also-good"]);
        assert_eq!(loaded.warnings.len(), 1);
        assert!(
            matches!(&loaded.warnings[0], LoadWarning::BadExpression { rule_id, .. } if rule_id == "bad"),
            "{:?}",
            loaded.warnings
        );
    }

    #[test]
    fn a_rule_with_neither_when_nor_always_is_dropped() {
        let src = json!({
            "semanticContractVersion": "1.0",
            "contractId": "c", "version": "1.0.0", "toolMapping": TOOLS,
            "rules": [{ "id": "r", "severity": "warn", "guidance": "g" }]
        })
        .to_string();
        let loaded = contract::load_json(&src, &tools(), "c").unwrap();
        assert!(loaded.contract.rules.is_empty());
        assert_eq!(loaded.warnings.len(), 1);
    }

    #[test]
    fn a_rule_with_both_when_and_always_is_dropped() {
        let src = json!({
            "semanticContractVersion": "1.0",
            "contractId": "c", "version": "1.0.0", "toolMapping": TOOLS,
            "rules": [{
                "id": "r", "severity": "warn", "always": true,
                "when": "payload.a == 1", "guidance": "g"
            }]
        })
        .to_string();
        let loaded = contract::load_json(&src, &tools(), "c").unwrap();
        assert!(loaded.contract.rules.is_empty());
    }

    #[test]
    fn an_empty_tool_mapping_is_rejected() {
        let src = json!({
            "semanticContractVersion": "1.0",
            "contractId": "c", "version": "1.0.0", "toolMapping": [],
            "rules": [{ "id": "r", "severity": "warn", "always": true, "guidance": "g" }]
        })
        .to_string();
        // A contract that governs nothing is a config mistake, not a no-op.
        assert!(contract::load_json(&src, &[], "c").is_err());
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(contract::load_json("{ not json", &tools(), "c").is_err());
    }

    #[test]
    fn a_missing_version_field_is_rejected() {
        let src = json!({
            "contractId": "c", "toolMapping": TOOLS,
            "rules": [{ "id": "r", "severity": "warn", "always": true, "guidance": "g" }]
        })
        .to_string();
        assert!(contract::load_json(&src, &tools(), "c").is_err());
    }
}

// ---------------------------------------------------------------------------
// Markdown loader
// ---------------------------------------------------------------------------

mod markdown_loader {
    use super::*;

    const DOC: &str = r#"---
contractId: erp-md
version: 2.0.0
toolMapping:
  - get_sales_order
---

# Sales order interpretation

- deliveredQuantity is staged stock, not shipped stock.
- An empty holds array may be role-filtered.
"#;

    #[test]
    fn reads_version_and_tool_mapping_from_frontmatter() {
        let loaded = contract::load_markdown(DOC, &[], Severity::Warn, "erp-md").unwrap();
        assert_eq!(loaded.contract.version, "2.0.0");
        assert_eq!(loaded.contract.tools, vec!["get_sales_order".to_string()]);
    }

    #[test]
    fn the_whole_body_becomes_one_unconditional_rule() {
        let loaded = contract::load_markdown(DOC, &tools(), Severity::Critical, "c").unwrap();
        assert_eq!(
            loaded.contract.rules.len(),
            1,
            "prose has no conditions to split on"
        );
        let rule = &loaded.contract.rules[0];
        assert!(matches!(rule.cond, Cond::Always));
        assert_eq!(rule.severity, Severity::Critical);
        assert!(rule.guidance.contains("deliveredQuantity is staged stock"));
        assert!(rule
            .guidance
            .contains("empty holds array may be role-filtered"));
    }

    #[test]
    fn frontmatter_severity_overrides_the_binding_default() {
        let doc = "---\nseverity: critical\n---\n\nQuantities are in base units.";
        let loaded = contract::load_markdown(doc, &tools(), Severity::Info, "c").unwrap();
        assert_eq!(loaded.contract.rules[0].severity, Severity::Critical);
    }

    #[test]
    fn the_synthetic_rule_id_is_stable_and_namespaced() {
        let first = contract::load_markdown(DOC, &tools(), Severity::Warn, "c").unwrap();
        let second = contract::load_markdown(DOC, &tools(), Severity::Warn, "c").unwrap();
        assert_eq!(first.contract.rules[0].id, second.contract.rules[0].id);
        assert!(first.contract.rules[0].id.starts_with("c"));
    }

    #[test]
    fn markdown_without_frontmatter_is_rejected_rather_than_guessed_at() {
        // `format: text` covers metadata-free prose; markdown without
        // frontmatter is more likely a mistake than an intent.
        let err = contract::load_markdown(
            "Quantities are in base units.",
            &tools(),
            Severity::Info,
            "c",
        )
        .unwrap_err();
        assert!(err.to_string().contains("frontmatter"), "{}", err);
    }

    #[test]
    fn an_empty_body_is_rejected() {
        assert!(
            contract::load_markdown(
                "---\nversion: 1.0.0\n---\n\n   ",
                &tools(),
                Severity::Warn,
                "c"
            )
            .is_err(),
            "a contract that would inject nothing is a config error"
        );
    }
}

// ---------------------------------------------------------------------------
// Text loader
// ---------------------------------------------------------------------------

mod text_loader {
    use super::*;

    #[test]
    fn the_whole_document_becomes_one_rule_at_the_configured_severity() {
        let loaded = contract::load_text(
            "First guidance line.\nSecond guidance line.\n",
            &tools(),
            Severity::Critical,
            "plain",
        )
        .unwrap();
        assert_eq!(loaded.contract.rules.len(), 1);
        assert_eq!(loaded.contract.rules[0].severity, Severity::Critical);
        assert!(loaded.contract.rules[0]
            .guidance
            .contains("Second guidance line."));
    }

    #[test]
    fn an_empty_document_is_rejected() {
        assert!(contract::load_text("   \n\n", &tools(), Severity::Warn, "p").is_err());
    }

    #[test]
    fn a_text_contract_without_a_tool_mapping_is_rejected() {
        // Plain text carries no metadata, so the binding must supply the mapping.
        assert!(contract::load_text("guidance", &[], Severity::Warn, "p").is_err());
    }
}

// ---------------------------------------------------------------------------
// Fetched artifacts
// ---------------------------------------------------------------------------

mod fetched {
    use super::*;

    #[test]
    fn content_type_selects_the_loader() {
        let md = contract::load_fetched(
            "guidance prose",
            Some("text/markdown; charset=utf-8"),
            &tools(),
            Severity::Warn,
            "c",
        )
        .unwrap();
        assert_eq!(md.contract.rules.len(), 1);

        let js = contract::load_fetched(
            super::super::common::ERP_CONTRACT,
            Some("application/json"),
            &tools(),
            Severity::Warn,
            "c",
        )
        .unwrap();
        assert_eq!(js.contract.rules.len(), 7);
    }

    #[test]
    fn a_json_body_is_detected_without_a_content_type() {
        let loaded = contract::load_fetched(
            super::super::common::ERP_CONTRACT,
            None,
            &tools(),
            Severity::Warn,
            "c",
        )
        .unwrap();
        assert_eq!(
            loaded.contract.rules.len(),
            7,
            "must not fall back to the text loader"
        );
    }
}

// ---------------------------------------------------------------------------
// Integrity pinning
// ---------------------------------------------------------------------------

mod integrity {
    use super::*;

    /// Known-answer test for the hand-rolled SHA-256.
    #[test]
    fn sha256_matches_published_vectors() {
        assert_eq!(
            crate::sha256::hex_digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            crate::sha256::hex_digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            crate::sha256::hex_digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn sha256_handles_the_length_padding_boundary() {
        // 55, 56 and 64 bytes straddle the block-padding edge cases.
        for len in [55usize, 56, 64, 119, 120] {
            let input = vec![b'a'; len];
            assert_eq!(
                crate::sha256::hex_digest(&input).len(),
                64,
                "digest for {} bytes",
                len
            );
        }
        assert_eq!(
            crate::sha256::hex_digest(&vec![b'a'; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
    }

    #[test]
    fn a_matching_pin_verifies() {
        let body = b"contract bytes";
        let pin = format!("sha256:{}", crate::sha256::hex_digest(body));
        assert!(contract::verify_integrity(&pin, body).is_ok());
    }

    #[test]
    fn pins_are_case_insensitive() {
        let body = b"contract bytes";
        let pin = format!("sha256:{}", crate::sha256::hex_digest(body).to_uppercase());
        assert!(contract::verify_integrity(&pin, body).is_ok());
    }

    #[test]
    fn a_single_flipped_byte_fails_verification() {
        let pin = format!("sha256:{}", crate::sha256::hex_digest(b"contract bytes"));
        assert!(contract::verify_integrity(&pin, b"contract byteS").is_err());
    }

    #[test]
    fn malformed_pins_are_rejected() {
        for pin in [
            "",
            "deadbeef",
            "sha256:",
            "sha256:tooshort",
            "md5:d41d8cd98f00b204e9800998ecf8427e",
            "sha256:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            assert!(
                contract::parse_integrity_pin(pin).is_none(),
                "`{}` must be rejected",
                pin
            );
            assert!(contract::verify_integrity(pin, b"x").is_err());
        }
    }
}

// ---------------------------------------------------------------------------
// Payload binding
// ---------------------------------------------------------------------------

mod binding {
    use super::*;

    #[test]
    fn prefers_structured_content() {
        let result = json!({
            "structuredContent": { "a": 1 },
            "content": [{ "type": "text", "text": "{\"a\":2}" }]
        });
        let (payload, binding) = contract::bind_payload(&result);
        assert_eq!(payload["a"], 1);
        assert_eq!(binding, Binding::StructuredContent);
    }

    #[test]
    fn parses_the_first_json_text_element() {
        let result = json!({
            "content": [
                { "type": "text", "text": "preamble, not JSON" },
                { "type": "text", "text": "{\"a\":2}" }
            ]
        });
        let (payload, binding) = contract::bind_payload(&result);
        assert_eq!(payload["a"], 2);
        assert_eq!(binding, Binding::TextContent);
    }

    #[test]
    fn ignores_non_text_content_elements() {
        let result = json!({
            "content": [
                { "type": "image", "data": "…", "mimeType": "image/png" },
                { "type": "text", "text": "{\"a\":3}" }
            ]
        });
        assert_eq!(contract::bind_payload(&result).1, Binding::TextContent);
    }

    #[test]
    fn reports_unbindable_when_nothing_parses() {
        let result = json!({ "content": [{ "type": "text", "text": "just prose" }] });
        assert_eq!(contract::bind_payload(&result).1, Binding::Unbindable);
    }

    #[test]
    fn reports_unbindable_for_an_empty_result() {
        assert_eq!(contract::bind_payload(&json!({})).1, Binding::Unbindable);
    }
}

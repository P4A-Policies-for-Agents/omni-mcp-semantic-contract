// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Exhaustive tests for the `jsonpath-subset` evaluator.

use crate::expr::{eval, parse};
use serde_json::{json, Value};

fn payload() -> Value {
    json!({
        "n": 10,
        "f": 2.5,
        "s": "abc",
        "t": true,
        "fa": false,
        "nul": null,
        "arr": [1, 2, 3],
        "empty": [],
        "obj": { "deep": { "x": 7 } },
        "items": [ { "uom": "EA", "salesUom": "CS" } ]
    })
}

/// Parses and evaluates against the shared payload; panics on a parse error.
fn ev(src: &str) -> bool {
    let ast = parse(src).unwrap_or_else(|e| panic!("parse of `{}` failed: {}", src, e));
    eval(&ast, &payload())
}

fn parse_err(src: &str) -> String {
    match parse(src) {
        Ok(_) => panic!("expected `{}` to be rejected", src),
        Err(e) => e.0,
    }
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

mod paths {
    use super::*;

    #[test]
    fn resolves_object_keys() {
        assert!(ev("payload.n == 10"));
        assert!(ev("payload.s == \"abc\""));
    }

    #[test]
    fn resolves_nested_keys() {
        assert!(ev("payload.obj.deep.x == 7"));
    }

    #[test]
    fn resolves_array_indices() {
        assert!(ev("payload.arr[0] == 1"));
        assert!(ev("payload.arr[2] == 3"));
        assert!(ev("payload.items[0].uom == \"EA\""));
    }

    #[test]
    fn missing_path_is_null_not_an_error() {
        assert!(ev("payload.nope == null"));
        assert!(ev("payload.obj.nope.deeper == null"));
    }

    #[test]
    fn out_of_bounds_index_is_null() {
        assert!(ev("payload.arr[99] == null"));
    }

    #[test]
    fn indexing_a_non_array_is_null() {
        assert!(ev("payload.s[0] == null"));
    }

    #[test]
    fn bare_payload_is_a_valid_path() {
        // An object is not boolean true, so a bare path is false, not an error.
        assert!(!ev("payload"));
    }
}

// ---------------------------------------------------------------------------
// Equality
// ---------------------------------------------------------------------------

mod equality {
    use super::*;

    #[test]
    fn null_equals_null() {
        assert!(ev("payload.nul == null"));
        assert!(!ev("payload.nul != null"));
    }

    #[test]
    fn non_null_is_not_equal_to_null() {
        // The delivery-block rule depends on exactly this.
        assert!(ev("payload.s != null"));
        assert!(!ev("payload.s == null"));
    }

    #[test]
    fn integers_and_floats_compare_by_value() {
        assert!(ev("payload.n == 10"));
        assert!(ev("payload.f == 2.5"));
    }

    #[test]
    fn cross_type_equality_is_false_and_inequality_is_true() {
        assert!(!ev("payload.s == 10"));
        assert!(ev("payload.s != 10"));
        assert!(!ev("payload.t == 1"));
    }

    #[test]
    fn booleans_compare() {
        assert!(ev("payload.t == true"));
        assert!(ev("payload.fa == false"));
        assert!(ev("payload.t != false"));
    }

    #[test]
    fn negative_numbers_parse() {
        assert!(ev("payload.n != -10"));
        assert!(parse("payload.n > -1").is_ok());
    }
}

// ---------------------------------------------------------------------------
// Ordering
// ---------------------------------------------------------------------------

mod ordering {
    use super::*;

    #[test]
    fn numeric_ordering() {
        assert!(ev("payload.n > 5"));
        assert!(ev("payload.n >= 10"));
        assert!(ev("payload.n < 11"));
        assert!(ev("payload.n <= 10"));
        assert!(!ev("payload.n > 10"));
    }

    #[test]
    fn string_ordering_is_lexicographic() {
        assert!(ev("payload.s > \"aba\""));
        assert!(ev("payload.s < \"abd\""));
    }

    #[test]
    fn null_ordering_is_always_false() {
        for op in [">", "<", ">=", "<="] {
            assert!(!ev(&format!("payload.nul {} 5", op)), "null {} 5", op);
            assert!(!ev(&format!("payload.n {} null", op)), "n {} null", op);
        }
    }

    #[test]
    fn missing_path_ordering_is_always_false() {
        assert!(!ev("payload.nope > 0"));
        assert!(!ev("payload.nope <= 0"));
    }

    #[test]
    fn type_mismatch_ordering_is_false_never_an_error() {
        assert!(!ev("payload.s > 5"));
        assert!(!ev("payload.t > 0"));
        assert!(!ev("payload.arr > 1"));
    }
}

// ---------------------------------------------------------------------------
// Calls
// ---------------------------------------------------------------------------

mod calls {
    use super::*;

    #[test]
    fn size_of_counts_array_elements() {
        assert!(ev("sizeOf(payload.arr) == 3"));
        assert!(ev("sizeOf(payload.empty) == 0"));
    }

    #[test]
    fn size_of_non_array_is_zero() {
        assert!(ev("sizeOf(payload.s) == 0"));
        assert!(ev("sizeOf(payload.obj) == 0"));
        assert!(ev("sizeOf(payload.nope) == 0"));
        assert!(ev("sizeOf(payload.nul) == 0"));
    }

    #[test]
    fn exists_is_true_for_non_null() {
        assert!(ev("exists(payload.s)"));
        assert!(ev("exists(payload.fa)"), "false is present, just falsy");
        assert!(ev("exists(payload.empty)"));
    }

    #[test]
    fn exists_is_false_for_null_and_missing() {
        assert!(!ev("exists(payload.nul)"));
        assert!(!ev("exists(payload.nope)"));
    }

    #[test]
    fn exists_composes_with_comparison() {
        assert!(ev("exists(payload.s) == true"));
        assert!(ev("exists(payload.nul) == false"));
    }
}

// ---------------------------------------------------------------------------
// Boolean structure
// ---------------------------------------------------------------------------

mod boolean_structure {
    use super::*;

    #[test]
    fn and_requires_all_terms() {
        assert!(ev("payload.n == 10 and payload.s == \"abc\""));
        assert!(!ev("payload.n == 10 and payload.s == \"zzz\""));
    }

    #[test]
    fn or_requires_any_term() {
        assert!(ev("payload.n == 99 or payload.s == \"abc\""));
        assert!(!ev("payload.n == 99 or payload.s == \"zzz\""));
    }

    #[test]
    fn and_binds_tighter_than_or() {
        // Parsed as (false and false) or true -> true.
        assert!(ev(
            "payload.n == 1 and payload.n == 2 or payload.s == \"abc\""
        ));
        // Parsed as true or (false and false) -> true.
        assert!(ev(
            "payload.s == \"abc\" or payload.n == 1 and payload.n == 2"
        ));
        // If `or` bound tighter this would be true; it must be false.
        assert!(!ev("payload.n == 1 or payload.n == 2 and payload.n == 3"));
    }

    #[test]
    fn three_way_chains() {
        assert!(ev(
            "payload.n == 10 and payload.t == true and exists(payload.s)"
        ));
        assert!(!ev(
            "payload.n == 10 and payload.t == true and exists(payload.nul)"
        ));
    }

    #[test]
    fn bare_boolean_operand_is_a_condition() {
        assert!(ev("payload.t"));
        assert!(!ev("payload.fa"));
    }

    #[test]
    fn bare_non_boolean_operand_is_false_not_truthy() {
        assert!(!ev("payload.n"), "a non-zero number must not be truthy");
        assert!(!ev("payload.s"));
        assert!(!ev("payload.nul"));
    }
}

// ---------------------------------------------------------------------------
// Parse errors
// ---------------------------------------------------------------------------

mod parse_errors {
    use super::*;

    #[test]
    fn parentheses_are_rejected_with_a_clear_message() {
        let msg = parse_err("(payload.n == 1 or payload.n == 2) and payload.t");
        assert!(
            msg.contains("parentheses"),
            "expected a parenthesis-specific message, got `{}`",
            msg
        );
    }

    #[test]
    fn empty_expression_is_rejected() {
        assert!(!parse_err("   ").is_empty());
    }

    #[test]
    fn unrooted_path_is_rejected() {
        let msg = parse_err("foo.bar == 1");
        assert!(msg.contains("payload"), "got `{}`", msg);
    }

    #[test]
    fn single_equals_is_rejected() {
        assert!(parse_err("payload.n = 1").contains("=="));
    }

    #[test]
    fn bare_bang_is_rejected() {
        assert!(parse_err("!payload.t").contains("!="));
    }

    #[test]
    fn unterminated_string_is_rejected() {
        assert!(parse_err("payload.s == \"abc").contains("unterminated"));
    }

    #[test]
    fn trailing_operator_is_rejected() {
        assert!(parse_err("payload.n ==").contains("unexpected end"));
    }

    #[test]
    fn dangling_conjunction_is_rejected() {
        assert!(parse("payload.n == 1 and").is_err());
    }

    #[test]
    fn call_on_a_literal_is_rejected() {
        assert!(parse("sizeOf(\"abc\")").is_err());
        assert!(parse("exists(5)").is_err());
    }

    #[test]
    fn unclosed_call_is_rejected() {
        assert!(parse("sizeOf(payload.arr").is_err());
    }

    #[test]
    fn non_integer_index_is_rejected() {
        assert!(parse("payload.arr[\"a\"] == 1").is_err());
        assert!(parse("payload.arr[1.5] == 1").is_err());
    }

    #[test]
    fn trailing_input_is_rejected() {
        assert!(parse("payload.n == 1 payload.s == \"a\"").is_err());
    }

    #[test]
    fn unknown_function_is_rejected() {
        assert!(parse("lengthOf(payload.arr) == 1").is_err());
    }

    #[test]
    fn dot_without_a_field_is_rejected() {
        assert!(parse("payload. == 1").is_err());
    }
}

// ---------------------------------------------------------------------------
// The shipped ERP rules, evaluated directly
// ---------------------------------------------------------------------------

mod erp_rules {
    use super::*;

    fn order() -> Value {
        let response: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/erp-sales-order.response.json"
        ))
        .unwrap();
        response["result"]["structuredContent"].clone()
    }

    fn fires(when: &str, payload: &Value) -> bool {
        eval(&parse(when).unwrap(), payload)
    }

    #[test]
    fn credit_rule_needs_both_status_and_exposure() {
        let when = "payload.credit.creditStatus == \"B\" and payload.credit.exposure > payload.credit.creditLimit";
        let mut p = order();
        assert!(fires(when, &p));

        p["credit"]["creditStatus"] = json!("A");
        assert!(!fires(when, &p), "status A must silence the rule");

        let mut p = order();
        p["credit"]["exposure"] = json!(1000.0);
        assert!(
            !fires(when, &p),
            "exposure under limit must silence the rule"
        );
    }

    #[test]
    fn holds_rule_needs_an_empty_array_and_a_block() {
        let when = "sizeOf(payload.holds) == 0 and payload.header.deliveryBlock != null";
        let mut p = order();
        assert!(fires(when, &p));

        p["holds"] = json!([{ "type": "credit", "blocking": true }]);
        assert!(!fires(when, &p), "a visible hold must silence the rule");

        let mut p = order();
        p["header"]["deliveryBlock"] = Value::Null;
        assert!(!fires(when, &p), "no block means no inferred hidden hold");
    }

    #[test]
    fn replica_rule_is_a_threshold() {
        let when = "payload.meta.replicationLagSeconds > 300";
        let mut p = order();
        assert!(fires(when, &p));
        p["meta"]["replicationLagSeconds"] = json!(120);
        assert!(!fires(when, &p));
        p["meta"]["replicationLagSeconds"] = json!(300);
        assert!(!fires(when, &p), "the threshold is exclusive");
    }

    #[test]
    fn goods_issue_rule_distinguishes_staged_from_shipped() {
        let when =
            "payload.items[0].deliveredQuantity > 0 and payload.items[0].goodsIssuedQuantity == 0";
        let mut p = order();
        assert!(fires(when, &p));
        p["items"][0]["goodsIssuedQuantity"] = json!(288);
        assert!(!fires(when, &p));
    }
}

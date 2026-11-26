//! M5c item 4: `lykaon grammar --export` — the EBNF and JSON exports.

#[test]
fn json_export_is_valid_and_complete() {
    let j = lykaon::grammar::export_json();
    let v: serde_json::Value = serde_json::from_str(&j).expect("valid JSON");
    assert_eq!(v["language"], "Lykaon");
    assert!(v["statement_starters"].as_array().unwrap().len() > 40);
    assert!(v["base_types"].as_array().unwrap().len() >= 8);
    assert!(v["error_codes"].as_array().unwrap().len() >= 10);
}

#[test]
fn ebnf_export_has_the_core_rules() {
    let e = lykaon::grammar::export_ebnf();
    for rule in ["program", "statement", "function", "record", "type", "cond"] {
        assert!(e.contains(rule), "missing rule {rule}");
    }
}

#[test]
fn starters_cover_the_common_sentences() {
    for s in [
        "Say", "Set", "If", "Otherwise", "Repeat", "For", "To", "A", "Test",
        "Check", "Try", "Look", "Use", "Return_missing_marker_never",
    ] {
        if s == "Return_missing_marker_never" {
            continue;
        }
        assert!(lykaon::grammar::STARTERS.contains(&s), "missing starter {s}");
    }
}
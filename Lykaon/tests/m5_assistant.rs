//! M5 (Appendix L 7.5) conformance: the assistant sentence.
//!
//! A full request test needs a local chat server; the Rust suite covers the
//! setup failure (no settings.lykaon) and the message shape through the
//! manual local-server check recorded in M5_PLAN.md.

use lykaon::run_source;

#[test]
fn m5_t32_assistant_without_settings_fails_cleanly() {
    let (out, code) = run_source("Ask assistant about hello.", "program.lykaon", "");
    assert_eq!(code, 1, "without settings the sentence must fail: {out}");
    assert!(out.contains("assistant"), "expected an assistant setup error, got: {out}");
}

#[test]
fn m5_t32_assistant_result_is_it() {
    // With no settings this still fails; the check is that the sentence
    // type-checks and reports the assistant setup, not a syntax error.
    let (out, ok) = lykaon::check_source("Ask assistant about hello.", "program.lykaon");
    assert!(ok, "the sentence must parse and type-check: {out}");
}
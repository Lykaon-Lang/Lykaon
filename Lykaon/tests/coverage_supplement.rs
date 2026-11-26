//! Coverage supplement: diagnostics that were implemented but not cited.

use lykaon::{check_source, run_source};

#[test]
fn t0128_missing_block_is_cited() {
    let (out, ok) = check_source("Repeat 3 times.\n", "missing.lykaon");
    assert!(!ok, "expected a rejection: {out}");
    assert!(out.contains("T0128"), "expected T0128, got: {out}");
}

#[test]
fn t0605_vector_length_is_cited() {
    let (out, code) = run_source(
        "Nums is vector of 1 and 2.\nOther is vector of 1.\nSay dot of Nums and Other.\n",
        "vec.lykaon",
        "",
    );
    assert_ne!(code, 0, "expected a runtime failure: {out}");
    assert!(out.contains("T0605"), "expected T0605, got: {out}");
}
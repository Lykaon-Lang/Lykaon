//! Linter conformance: T0141-T0147 (Ch.40).

use lykaon::lint::lint_source;

fn codes(src: &str) -> Vec<String> {
    lint_source(src, "lint.lykaon")
        .iter()
        .map(|d| d.code.to_string())
        .collect()
}

#[test]
fn t0141_name_in_text_by_accident() {
    let c = codes("Price is 20.\nSay price is 20.\n");
    assert!(c.contains(&"T0141".to_string()), "{c:?}");
}

#[test]
fn t0142_soft_word_shadowed() {
    let c = codes("Table is 5.\nSay table.\n");
    assert!(c.contains(&"T0142".to_string()), "{c:?}");
}

#[test]
fn t0143_long_sentence() {
    let src = "Say one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty one.\n";
    let c = codes(src);
    assert!(c.contains(&"T0143".to_string()), "{c:?}");
}

#[test]
fn t0144_unused_name() {
    let c = codes("Price is 20.\n");
    assert!(c.contains(&"T0144".to_string()), "{c:?}");
}

#[test]
fn t0145_empty_block() {
    let c = codes("If x is 1.\n    Note nothing.\n");
    assert!(c.contains(&"T0145".to_string()), "{c:?}");
}

#[test]
fn t0146_identical_branches() {
    let c = codes("If x is 1.\n    Say a.\nOtherwise.\n    Say a.\n");
    assert!(c.contains(&"T0146".to_string()), "{c:?}");
}

#[test]
fn t0147_magic_number() {
    let c = codes("X is 5.\nY is 5.\nZ is 5.\n");
    assert!(c.contains(&"T0147".to_string()), "{c:?}");
}
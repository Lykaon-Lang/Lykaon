//! M5 (Appendix L 7.3) conformance: Predict, the four models, accuracy /
//! error, importance, cleaning, save/load and vectors. The fixtures are
//! `data/sales.csv`, `data/clean.csv` and `data/tiny.csv`.
//!
//! Run: cargo test --test m5_ai -- --nocapture

use lykaon::{check_source, run_source};

fn run(src: &str) -> (String, i32) {
    run_source(src, "program.lykaon", "")
}

fn out(src: &str) -> String {
    let (out, code) = run(src);
    assert_eq!(code, 0, "the program failed: {out}");
    out.trim_end().to_string()
}

const LOAD: &str = "Load sales.\n";

#[test]
fn m5_t15_accuracy_is_a_fraction_and_deterministic() {
    for model in [
        "random forest",
        "decision tree",
        "linear model",
        "neural network",
    ] {
        let src = format!("{LOAD}Predict churned from the rest using {model}.\nSay accuracy.");
        let a = out(&src);
        let b = out(&src);
        assert_eq!(a, b, "{model} must be deterministic");
        let value: f64 = a
            .strip_prefix("accuracy ")
            .unwrap_or_else(|| panic!("{model}: expected accuracy, got {a:?}"))
            .parse()
            .unwrap_or_else(|_| panic!("{model}: bad accuracy {a:?}"));
        assert!((0.0..=1.0).contains(&value), "{model}: {value}");
    }
}

#[test]
fn m5_t16_importance_is_shares_or_nothing() {
    let text = out(&format!(
        "{LOAD}Predict churned from the rest.\nSay what matters most."
    ));
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("what matters most"));
    let rest: Vec<&str> = lines.collect();
    if rest == ["nothing stands out"] {
        return;
    }
    assert!(!rest.is_empty(), "importance printed no rows");
    assert!(rest.len() <= 5, "at most 5 rows: {rest:?}");
    let mut sum = 0.0;
    for line in &rest {
        let share: f64 = line
            .rsplit(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap_or_else(|_| panic!("bad share line {line:?}"));
        sum += share;
    }
    assert!((sum - 1.0).abs() < 1e-9, "shares must add to 1.00: {sum}");
}

#[test]
fn m5_t17_cleaning_nothing_needed() {
    assert_eq!(
        out(&format!(
            "{LOAD}Predict churned from the rest.\nSay what cleaning did."
        )),
        "nothing needed cleaning"
    );
}

#[test]
fn m5_t18_cleaning_reports_the_fill() {
    assert_eq!(
        out("Load clean.\nPredict region from size and price.\nSay what cleaning did."),
        "filled 1 missing numbers in price with the median"
    );
}

#[test]
fn m5_t20_accuracy_for_a_number_target_is_t0180() {
    // T0180 is a run-time failure: the sentence type-checks, the model is
    // a regression, and accuracy is the wrong measure for it.
    let (text, code) = run(&format!(
        "{LOAD}Predict price from the rest.\nSay accuracy."
    ));
    assert_eq!(code, 1, "expected the run to fail: {text}");
    assert!(text.contains("T0180"), "expected T0180, got: {text}");
    assert!(text.contains("Wrong measure for the target."), "got: {text}");
}

#[test]
fn m5_t21_save_and_load_keep_the_prediction() {
    let dir = std::path::Path::new("output");
    let model_path = dir.join("m5aimodel.model");
    let _ = std::fs::remove_file(&model_path);
    let before = out(&format!(
        "{LOAD}Predict churned from the rest.\nSay prediction for size 40 and price 100."
    ));
    let src = format!(
        "{LOAD}Predict churned from the rest.\nSave model as m5aimodel.\nLoad model m5aimodel.\nSay prediction for size 40 and price 100."
    );
    let after = out(&src);
    let _ = std::fs::remove_file(&model_path);
    assert_eq!(before, after, "save/load must keep the answer");
}

#[test]
fn m5_t22_unknown_target_and_model() {
    let (text, ok) = check_source(
        &format!("{LOAD}Predict nope from the rest."),
        "program.lykaon",
    );
    assert!(!ok, "expected T0179, got: {text}");
    assert!(text.contains("T0179"), "expected T0179, got: {text}");
    assert!(text.contains("Target is not a column."), "got: {text}");

    let (text, ok) = check_source(
        &format!("{LOAD}Predict churned from the rest using forest."),
        "program.lykaon",
    );
    assert!(!ok, "expected T0183, got: {text}");
    assert!(text.contains("T0183"), "expected T0183, got: {text}");
    assert!(text.contains("Unknown model."), "got: {text}");
}

#[test]
fn m5_too_few_rows_is_t0187() {
    let (text, ok) = check_source("Load tiny.\nPredict b from a.", "program.lykaon");
    // T0187 is a run-time failure, not a compile error.
    if ok {
        let (out, code) = run("Load tiny.\nPredict b from a.");
        assert_eq!(code, 1, "expected the run to fail: {out}");
        assert!(out.contains("Too few rows."), "got: {out}");
    } else {
        assert!(text.contains("T0187"), "expected T0187, got: {text}");
    }
}

#[test]
fn m5_regression_error_is_a_number() {
    let text = out(&format!(
        "{LOAD}Predict price from the rest.\nSay error."
    ));
    let value: f64 = text
        .strip_prefix("error ")
        .unwrap_or_else(|| panic!("expected error, got {text:?}"))
        .parse()
        .unwrap_or_else(|_| panic!("bad error {text:?}"));
    assert!(value >= 0.0, "error must not be negative: {value}");
}

#[test]
fn m5_s31_vectors() {
    let text = out(
        "Nums is vector of 1 and 2 and 3.\nSay dot of Nums and vector of 4 and 5 and 6.\nSay norm of Nums.\nSay Nums plus vector of 4 and 5 and 6.\nSay Nums times 2.",
    );
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "32.00");
    assert_eq!(lines[1], "3.74");
    assert_eq!(lines[2], "5.00, 7.00 and 9.00");
    assert_eq!(lines[3], "2.00, 4.00 and 6.00");
}
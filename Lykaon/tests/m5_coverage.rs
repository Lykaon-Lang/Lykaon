//! M5 (Appendix L 9.4): `lykaon test coverage` — the report counts the
//! sentences the tests ran and excludes the Test/Check drivers.

use std::path::PathBuf;

/// Coverage state is global in the interpreter, so these tests must not run
/// concurrently with each other.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lykaon_cov_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn covered_function_reports_100() {
    let _g = LOCK.lock().unwrap();
    let d = dir("full");
    std::fs::write(
        d.join("m.lykaon"),
        "To f n as whole number.\n    Give back n plus 1 as whole number.\nTest f.\n    Check that f 1 is 2.\n",
    )
    .unwrap();
    let report = lykaon::test_coverage::run(&d, None).expect("report");
    assert!(report.contains("100.0%"), "{report}");
    assert!(report.contains("1 sentences, 1 run"), "{report}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn untested_function_reports_0() {
    let _g = LOCK.lock().unwrap();
    let d = dir("none");
    std::fs::write(
        d.join("m.lykaon"),
        "To g n as whole number.\n    Give back n plus 1 as whole number.\n",
    )
    .unwrap();
    let report = lykaon::test_coverage::run(&d, None).expect("report");
    assert!(report.contains("0.0%"), "{report}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn min_gate_fails_below_threshold() {
    let _g = LOCK.lock().unwrap();
    let d = dir("min");
    std::fs::write(
        d.join("m.lykaon"),
        "To g n as whole number.\n    Give back n plus 1 as whole number.\n",
    )
    .unwrap();
    let res = lykaon::test_coverage::run(&d, Some(100.0));
    assert!(res.is_err(), "0% coverage must fail a 100% minimum");
    let _ = std::fs::remove_dir_all(&d);
}
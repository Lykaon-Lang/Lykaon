//! M5g item g: `lykaon run-many` - one program, many runs, throughput.
//!
//! The tests drive the real module (and one real CLI call) and assert the
//! things a benchmark must get right: every run is counted, the outputs
//! agree, warmups are excluded, a broken program is an error, and the JSON
//! is machine-readable.

use lykaon::run_many::{Options, run};

fn opts(runs: usize) -> Options {
    Options {
        runs,
        ..Options::default()
    }
}

#[test]
fn every_run_is_counted_and_the_output_agrees() {
    let src = "Changeable n is 0.\nRepeat 100 times.\n    Set n to n plus 1.\nSay n.\n";
    let summary = run(src, "program.lykaon", &opts(200)).expect("run-many");
    assert_eq!(summary.runs, 200);
    assert!(summary.outputs_identical, "every run must print the same bytes");
    assert!(summary.runs_per_sec > 0.0);
    assert!(summary.p50 <= summary.p95);
    assert!(summary.p95 <= summary.p99);
    assert_eq!(summary.first_output.trim(), "100");
}

#[test]
fn warmup_runs_are_not_counted() {
    let src = "Say ok.\n";
    let mut o = opts(20);
    o.warmup = 5;
    let summary = run(src, "program.lykaon", &o).expect("run-many");
    assert_eq!(summary.runs, 20);
    assert_eq!(summary.warmup, 5);
}

#[test]
fn threads_do_not_change_the_answer() {
    let src = "Changeable n is 1.\nRepeat 50 times.\n    Set n to n plus 1.\nSay n.\n";
    let mut o = opts(60);
    o.threads = 4;
    let summary = run(src, "program.lykaon", &o).expect("run-many");
    assert_eq!(summary.runs, 60);
    assert!(summary.outputs_identical, "threaded runs must agree");
    assert_eq!(summary.first_output.trim(), "51");
}

#[test]
fn a_broken_program_is_an_error_not_a_benchmark() {
    let err = run("Say (.\n", "bad.lykaon", &opts(10)).unwrap_err();
    assert!(err.contains("T0"), "expected a diagnostic, got: {err}");
}

#[test]
fn a_fuel_budget_stops_each_run() {
    let src = "Changeable n is 0.\nRepeat forever.\n    Set n to n plus 1.\n";
    let mut o = opts(5);
    o.fuel = 10;
    let err = run(src, "program.lykaon", &o).unwrap_err();
    assert!(
        err.contains("too long"),
        "the budget must stop the run, got: {err}"
    );
}

#[test]
fn json_is_versioned_and_has_the_numbers() {
    let src = "Say ok.\n";
    let summary = run(src, "program.lykaon", &opts(10)).expect("run-many");
    let text = lykaon::run_many::json(&summary);
    let doc: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(doc["schema"], "lykaon-run-many-v1");
    assert_eq!(doc["runs"], 10);
    assert!(doc["runs_per_sec"].as_f64().unwrap() > 0.0);
    assert!(doc["p50_ms"].as_f64().unwrap() >= 0.0);
}

#[test]
fn the_cli_prints_a_json_summary() {
    let dir = std::env::temp_dir().join(format!("lykaon_runmany_cli_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let prog = dir.join("ok.lykaon");
    std::fs::write(&prog, "Say ok.\n").expect("write");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_lykaonc-cli"))
        .args([
            "run-many",
            prog.to_str().expect("path"),
            "--runs",
            "10",
            "--threads",
            "2",
            "--json",
        ])
        .output()
        .expect("run cli");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let doc: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(doc["runs"], 10);
    assert_eq!(doc["threads"], 2);
    let _ = std::fs::remove_dir_all(&dir);
}
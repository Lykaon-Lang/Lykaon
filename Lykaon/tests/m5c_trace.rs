//! M5c item 9: `lykaon run --trace-json` (the library side).

use lykaon::run_source;

#[test]
fn m5c_trace_records_steps() {
    lykaon_core::interp::enable_trace();
    let (out, code) = run_source("X is 1.\nSay X plus 1.\n", "program.lykaon", "");
    lykaon_core::interp::disable_trace();
    let trace = lykaon_core::interp::take_trace();
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "2");
    assert!(trace.len() >= 2, "expected at least two steps: {trace:?}");
    assert!(trace[0].contains("Define"), "{}", trace[0]);
    assert!(trace.iter().any(|s| s.contains("Say")), "{trace:?}");
}
//! M5c item 11: the real tested corpus under tools/corpus.

use lykaon::run_source;

#[test]
fn m5c_corpus_programs_run() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tools")
        .join("corpus")
        .join("corpus.jsonl");
    let text = std::fs::read_to_string(&path).expect("tools/corpus/corpus.jsonl");
    let mut total = 0;
    let mut failures = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).expect("corpus json");
        let code = v["code"].as_str().unwrap();
        let expected = v["expected"].as_str().unwrap().trim();
        let (out, status) = run_source(code, "program.lykaon", "");
        total += 1;
        if status != 0 || out.trim() != expected {
            failures.push(format!(
                "{}: code={} expected={:?} got={:?}",
                v["id"].as_str().unwrap_or("?"),
                status,
                expected,
                out.trim()
            ));
        }
    }
    assert!(total >= 50, "expected a real corpus, got {total}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
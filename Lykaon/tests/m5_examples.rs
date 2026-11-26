//! M5d item 4: the 50 runnable examples in docs/examples.

use lykaon::run_source;

#[test]
fn m5d_examples_run() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("examples");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("docs/examples")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("lykaon"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 50, "expected 50 examples");
    let mut failures = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).unwrap();
        let expected_file = file.with_extension("out");
        let expected = std::fs::read_to_string(&expected_file)
            .unwrap()
            .trim_end()
            .to_string();
        let (out, code) = run_source(&src, "program.lykaon", "");
        if code != 0 || out.trim_end() != expected {
            failures.push(format!(
                "{}: code={} expected={:?} got={:?}",
                file.display(),
                code,
                expected,
                out.trim_end()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
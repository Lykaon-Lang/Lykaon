//! M5 (Appendix L 9.3): the `lykaon doc` generator.
//!
//! Run: cargo test --test m5_doc -- --nocapture

#[test]
fn m5_doc_generates_signature_about_and_example() {
    let dir = std::env::temp_dir().join(format!("lykaon_doc_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("demo.lykaon"),
        "About double gives its input times 2.\n    Example doubling 21.\n        Check that double 21 is 42.\nTo double n give n times 2.\n",
    )
    .unwrap();
    let out = dir.join("docout");
    let pages = lykaon::docgen::generate(&dir, &out).expect("doc generation");
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    let page = std::fs::read_to_string(out.join("demo-double.html")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(pages, 1, "one documented function");
    assert!(index.contains("double"), "index lists the function: {index}");
    assert!(page.contains("To double n."), "signature: {page}");
    assert!(
        page.contains("double gives its input times 2."),
        "about: {page}"
    );
    assert!(
        page.contains("Check that double 21 is 42."),
        "example: {page}"
    );
}
//! M5c quick tools: test --json, docs --compact, explain.

#[test]
fn m5c_test_json_reports_ok() {
    let dir = std::env::temp_dir().join(format!("lykaon_m5c_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("t.lykaon");
    std::fs::write(
        &file,
        "To double n give n times 2.\nTest double works.\n    Check that double 21 is 42.\n",
    )
    .unwrap();
    let json = lykaon::m5c::test_json(&file.to_string_lossy()).unwrap();
    assert!(json.contains("\"ok\": true"), "{json}");
    assert!(json.contains("passed"), "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m5c_cheat_sheet_is_compact() {
    let s = lykaon::m5c::cheat_sheet();
    assert!(s.contains("Say hello world."));
    assert!(s.contains("Predict churned from the rest."));
    assert!(s.len() < 4000, "the cheat sheet should stay compact");
}

#[test]
fn m5c_explain_names_the_code() {
    let s = lykaon::m5c::explain("T0205");
    assert!(s.contains("T0205"));
    assert!(s.contains("not defined"));
}
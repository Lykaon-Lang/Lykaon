//! M5c item 3: `lykaon outline`.

#[test]
fn m5c_outline_lists_names_and_signatures() {
    let dir = std::env::temp_dir().join(format!("lykaon_outline_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("p.lykaon");
    std::fs::write(&file, "X is 5.\nTo double n give n times 2.\n").unwrap();
    let text = lykaon::outline::run(&file.to_string_lossy(), false).unwrap();
    assert!(text.contains("x: whole number"), "{text}");
    assert!(text.contains("To double n."), "{text}");
    let json = lykaon::outline::run(&file.to_string_lossy(), true).unwrap();
    assert!(json.contains("\"name\": \"double\""), "{json}");
    assert!(json.contains("\"kind\": \"whole number\""), "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}
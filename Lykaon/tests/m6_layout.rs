//! M6 C layout phrases on record declarations.

use lykaon::compile_tree;

#[test]
fn record_as_c_does_parses() {
    let src = "A thing has n as whole number and as C does.\n";
    assert!(compile_tree(src, "layout.lykaon").is_ok());
}

#[test]
fn packed_and_aligned_conflict_is_t0192() {
    let src = "A thing has n as whole number and packed and aligned to 16.\n";
    let err = match compile_tree(src, "layout.lykaon") {
        Err(e) => e,
        Ok(_) => panic!("expected T0192, but the program compiled"),
    };
    let text = err
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("T0192"), "expected T0192, got: {text}");
}

#[test]
fn use_c_library_parses() {
    assert!(compile_tree("Use C library math.\n", "cbridge.lykaon").is_ok());
}

#[test]
fn share_for_c_parses() {
    let src = "Share for C.\nTo double n as whole number give n times 2 as whole number.\n";
    assert!(compile_tree(src, "cbridge.lykaon").is_ok());
}

#[test]
fn load_plugin_parses() {
    assert!(compile_tree("Load plugin demo.\n", "plugin.lykaon").is_ok());
}
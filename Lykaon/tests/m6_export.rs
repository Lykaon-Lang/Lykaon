//! M6 `Share for C.` header/export generation.

use lykaon::{c_header, compile_tree};

#[test]
fn share_for_c_writes_a_c_header() {
    let src = "Share for C.\nTo double n as whole number give n times 2 as whole number.\n";
    let prog = compile_tree(src, "export.lykaon").expect("compile");
    let h = c_header(&prog);
    assert!(h.contains("int64_t double(int64_t a0);"), "{h}");
    assert!(h.contains("extern \"C\""), "{h}");
}
//! M6b bindgen tests.

use lykaon::bindgen::bindgen;

#[test]
fn bindgen_simple_header() {
    let h = "int add(int a, int b);\ndouble cos(double x);\nconst char *name(void);\n";
    let s = bindgen(h);
    assert!(s.contains("C function add as add takes whole number and whole number gives whole number."), "{s}");
    assert!(s.contains("C function cos as cos takes decimal gives decimal."), "{s}");
    assert!(s.contains("C function name as name takes nothing gives text."), "{s}");
}
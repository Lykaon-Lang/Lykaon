//! M6 C library runtime loading.

use lykaon::run_source;

#[cfg(windows)]
#[test]
fn use_c_library_loads_msvcrt() {
    let (out, code) = run_source("Use C library msvcrt.\nSay hi.\n", "clib.lykaon", "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "hi");
}

#[cfg(unix)]
#[test]
fn use_c_library_loads_libm() {
    let (out, code) = run_source("Use C library m.\nSay hi.\n", "clib.lykaon", "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "hi");
}

#[test]
fn missing_c_library_is_t0525() {
    let (out, code) = run_source("Use C library nope.\nSay hi.\n", "clib.lykaon", "");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("T0525"), "expected T0525, got: {out}");
}
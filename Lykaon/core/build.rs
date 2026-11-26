//! Build script: compile the vendored SQLite amalgamation with the same
//! C runtime as the Rust program (static CRT on MSVC). libsqlite3-sys is
//! used without its bundled feature; this script provides the `sqlite3`
//! static library so the native one-binary link keeps working.

fn main() {
    println!("cargo:rerun-if-changed=vendor/sqlite3.c");
    println!("cargo:rerun-if-changed=vendor/sqlite3.h");
    let want = std::env::var("CARGO_FEATURE_SQLITE").is_ok();
    let target = std::env::var("TARGET").unwrap_or_default();
    if !want || target.contains("wasm32") {
        return;
    }
    let mut build = cc::Build::new();
    build
        .file("vendor/sqlite3.c")
        .include("vendor")
        .warnings(false)
        .static_crt(true)
        .define("SQLITE_THREADSAFE", "1")
        .define("SQLITE_DEFAULT_MEMSTATUS", "0")
        .define("SQLITE_OMIT_LOAD_EXTENSION", "1");
    build.compile("sqlite3");
}
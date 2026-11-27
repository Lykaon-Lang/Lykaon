//! M6b: call a Python library from Lykaon through `.py.shelf`.
//!
//! Each scenario lives in its own test binary and makes a single Python call:
//! the Python C API leaves the process with state that can block the test
//! harness at exit when a process makes several calls, so one call per process
//! keeps the verification reliable.

use lykaon::run_source;
use std::path::PathBuf;

fn stage(name: &str, module: &str, stub: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_py_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{module}.py.shelf")), stub).unwrap();
    dir
}

#[test]
fn call_math_sqrt() {
    let dir = stage(
        "math",
        "math",
        "Python function sqrt as pysqrt takes decimal gives decimal.\n",
    );
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source("Say pysqrt 9.\n", file.to_str().unwrap(), "");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("3.00"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}
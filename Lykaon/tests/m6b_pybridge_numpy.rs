//! M6b: call a third-party Python library (numpy) from Lykaon.
//!
//! This lives in its own test binary: importing numpy starts worker threads,
//! and the Python C API leaves the process with a non-daemon thread that can
//! block the test harness at exit. One process per binary keeps it isolated.
//!
//! The numpy wheel is verified on Windows. On Linux the embedded interpreter
//! does not load the distribution numpy (its C extension is built against a
//! different Python), so the case is Windows-only rather than a silent skip.

use lykaon::run_source;
use std::path::PathBuf;

fn stage(name: &str, module: &str, stub: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_pynp_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{module}.py.shelf")), stub).unwrap();
    dir
}

#[cfg(windows)]
#[test]
fn call_numpy_sqrt() {
    let dir = stage(
        "numpy",
        "numpy",
        "Python function sqrt as npsqrt takes decimal gives decimal.\n",
    );
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source("Say npsqrt 16.\n", file.to_str().unwrap(), "");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("4.00"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}
//! M6b: a Python error is a normal Lykaon failure, never a silent workaround.
//! One call per process (see `m6b_pybridge.rs`).

use lykaon::run_source;
use std::path::PathBuf;

#[test]
fn python_error_is_a_normal_failure() {
    let dir = std::env::temp_dir().join(format!("lykaon_py_err_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // sqrt("x") raises TypeError; the boundary must report a normal failure.
    std::fs::write(
        dir.join("math.py.shelf"),
        "Python function sqrt as pysqrt takes text gives decimal.\n",
    )
    .unwrap();
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source("Say pysqrt x.\n", file.to_str().unwrap(), "");
    assert_ne!(code, 0, "a Python error must fail: {out}");
    let _ = std::fs::remove_dir_all(dir);
}
//! M6b: call `json.dumps` from Lykaon. One call per process (see
//! `m6b_pybridge.rs`).

use lykaon::run_source;
use std::path::PathBuf;

#[test]
fn call_json_dumps() {
    let dir = std::env::temp_dir().join(format!("lykaon_py_json_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("json.py.shelf"),
        "Python function dumps as pydumps takes text gives text.\n",
    )
    .unwrap();
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source("Say pydumps hi.\n", file.to_str().unwrap(), "");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"hi\""), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}
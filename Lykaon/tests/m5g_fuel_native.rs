//! M5g item a: native fuel metering.
//!
//! A fully native build (no interpreter in the exe) gets one
//! `lykaon_rt_tick()` per sentence when the runner asks for a budget. The
//! tests drive the real `lykaonc-cli run --fuel` path, so they cover
//! codegen, linking and the runtime tick together.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_fuel_native_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write");
    path
}

fn run_cli(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_lykaonc-cli"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("spawn lykaonc-cli");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const COUNT_10: &str = "Changeable n is 0.\nRepeat 10 times.\n    Set n to n plus 1.\nSay n.\n";

#[test]
fn native_fuel_stops_the_run() {
    let dir = temp_dir("stop");
    let prog = write(&dir, "prog.lykaon", COUNT_10);
    let path = prog.to_str().expect("utf8 path");
    let (code, out, err) = run_cli(&["run", "--fuel", "3", path]);
    assert_eq!(code, 1, "out={out:?} err={err:?}");
    // The interpreter reports a failed run on its own output stream; the
    // native tick reports on stderr. Either is the program's message.
    let text = format!("{out}{err}");
    assert!(
        text.contains("ran for too long"),
        "expected the budget message, got: {text:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_fuel_allows_a_sufficient_budget() {
    let dir = temp_dir("allow");
    let prog = write(&dir, "prog.lykaon", COUNT_10);
    let path = prog.to_str().expect("utf8 path");
    let (code, out, err) = run_cli(&["run", "--fuel", "1000000", path]);
    assert_eq!(code, 0, "out={out:?} err={err:?}");
    assert!(out.contains("10"), "out={out:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_without_fuel_is_unchanged() {
    let dir = temp_dir("nofuel");
    let prog = write(&dir, "prog.lykaon", COUNT_10);
    let path = prog.to_str().expect("utf8 path");
    let (code, out, err) = run_cli(&["run", path]);
    assert_eq!(code, 0, "out={out:?} err={err:?}");
    assert!(out.contains("10"), "out={out:?}");
    assert!(!err.contains("ran for too long"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_fuel_works_in_fast_and_debug() {
    let dir = temp_dir("modes");
    let prog = write(&dir, "prog.lykaon", COUNT_10);
    let path = prog.to_str().expect("utf8 path");
    for mode in ["--fast", "--small"] {
        let (code, out, err) = run_cli(&["run", mode, "--fuel", "3", path]);
        assert_eq!(code, 1, "{mode}: out={out:?} err={err:?}");
        let text = format!("{out}{err}");
        assert!(text.contains("ran for too long"), "{mode}: {text:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
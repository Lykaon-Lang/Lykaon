//! M5g item d: snapshots at top-level sentence boundaries (d1-d4).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_lykaonc-cli");

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_snap_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).expect("write");
}

fn cli(args: &[&str], stdin: &str) -> (i32, String, String) {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const SPLIT_SRC: &str = "Say one.\nChangeable x is 0.\nRepeat 3 times.\n    Set x to x plus 1.\nSay x.\n";

#[test]
fn d1_split_output_equals_the_full_run() {
    let dir = temp("d1");
    let prog = dir.join("p.lykaon");
    write(&prog, SPLIT_SRC);
    let snap = dir.join("s.json");

    let (fc, full, ferr) = cli(&["run", "--interpret", prog.to_str().unwrap()], "");
    assert_eq!(fc, 0, "{ferr}");
    assert_eq!(full, "one\n3\n");

    let (cc, prefix, cerr) = cli(
        &[
            "checkpoint",
            prog.to_str().unwrap(),
            "--after",
            "3",
            "--out",
            snap.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(cc, 0, "{cerr}");
    assert_eq!(prefix, "one\n");
    assert!(snap.is_file());

    let (rc, rest, rerr) = cli(
        &["resume", prog.to_str().unwrap(), "--snapshot", snap.to_str().unwrap()],
        "",
    );
    assert_eq!(rc, 0, "{rerr}");
    assert_eq!(rest, "3\n");
    assert_eq!(format!("{prefix}{rest}"), full, "split output must equal the full run");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d1_snapshot_zero_resumes_the_whole_program() {
    let dir = temp("d0");
    let prog = dir.join("p.lykaon");
    write(&prog, SPLIT_SRC);
    let snap = dir.join("s.json");

    let (cc, prefix, cerr) = cli(
        &["checkpoint", prog.to_str().unwrap(), "--after", "0", "--out", snap.to_str().unwrap()],
        "",
    );
    assert_eq!(cc, 0, "{cerr}");
    assert_eq!(prefix, "");

    let (rc, out, rerr) = cli(
        &["resume", prog.to_str().unwrap(), "--snapshot", snap.to_str().unwrap()],
        "",
    );
    assert_eq!(rc, 0, "{rerr}");
    assert_eq!(out, "one\n3\n");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d1_checkpoints_are_byte_identical_for_the_same_state() {
    let dir = temp("det");
    let prog = dir.join("p.lykaon");
    write(&prog, SPLIT_SRC);
    let a = dir.join("a.json");
    let b = dir.join("b.json");

    assert!(cli(
        &["checkpoint", prog.to_str().unwrap(), "--after", "2", "--out", a.to_str().unwrap()],
        ""
    )
    .0 == 0);
    assert!(cli(
        &["checkpoint", prog.to_str().unwrap(), "--after", "2", "--out", b.to_str().unwrap()],
        ""
    )
    .0 == 0);
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "no timestamps or random ids in a snapshot"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d3_resume_many_times_with_different_input() {
    let dir = temp("d3");
    let prog = dir.join("ask.lykaon");
    write(&prog, "Ask name.\nSay name.\n");
    let snap = dir.join("s.json");

    // Checkpoint before the Ask: the input is not consumed yet.
    let (cc, prefix, cerr) = cli(
        &[
            "checkpoint",
            prog.to_str().unwrap(),
            "--after",
            "0",
            "--out",
            snap.to_str().unwrap(),
        ],
        "Ada\n",
    );
    assert_eq!(cc, 0, "{cerr}");
    assert_eq!(prefix, "");

    for name in ["Grace", "Bob"] {
        let (rc, out, rerr) = cli(
            &[
                "resume",
                prog.to_str().unwrap(),
                "--snapshot",
                snap.to_str().unwrap(),
                "--input",
                &format!("{name}\n"),
            ],
            "",
        );
        assert_eq!(rc, 0, "{rerr}");
        assert_eq!(out.trim(), name, "resume must use the new input");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d4_stop_after_works_plain_and_with_replay() {
    let dir = temp("d4");
    let prog = dir.join("p.lykaon");
    write(&prog, SPLIT_SRC);

    let (fc, full, ferr) = cli(&["run", "--interpret", prog.to_str().unwrap()], "");
    assert_eq!(fc, 0, "{ferr}");
    assert_eq!(full, "one\n3\n");

    let (sc, out, serr) = cli(
        &["run", "--interpret", "--stop-after", "2", prog.to_str().unwrap()],
        "",
    );
    assert_eq!(sc, 0, "{serr}");
    assert_eq!(out, "one\n");

    let trace = dir.join("run.trace.json");
    let (rc, _, rerr) = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(rc, 0, "{rerr}");

    let (pc, pout, perr) = cli(
        &[
            "run",
            "--replay",
            trace.to_str().unwrap(),
            "--stop-after",
            "2",
            prog.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(pc, 0, "{perr}");
    assert_eq!(pout, "one\n", "replay to sentence 2");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_snapshot_from_another_program_is_refused() {
    let dir = temp("other");
    let a = dir.join("a.lykaon");
    let b = dir.join("b.lykaon");
    write(&a, "Say one.\n");
    write(&b, "Say two.\n");
    let snap = dir.join("s.json");

    assert_eq!(
        cli(
            &["checkpoint", a.to_str().unwrap(), "--after", "1", "--out", snap.to_str().unwrap()],
            ""
        )
        .0,
        0
    );
    let (rc, _, err) = cli(
        &["resume", b.to_str().unwrap(), "--snapshot", snap.to_str().unwrap()],
        "",
    );
    assert_eq!(rc, 1);
    assert!(err.contains("different program"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pending_timers_make_a_checkpoint_fail_cleanly() {
    let dir = temp("timer");
    let prog = dir.join("t.lykaon");
    write(&prog, "After 1 milliseconds.\n    Say later.\nSay done.\n");
    let snap = dir.join("s.json");

    let (cc, out, cerr) = cli(
        &["checkpoint", prog.to_str().unwrap(), "--after", "2", "--out", snap.to_str().unwrap()],
        "",
    );
    assert_eq!(cc, 1);
    assert!(out.contains("done"), "{out}");
    assert!(cerr.contains("does not support"), "{cerr}");
    assert!(!snap.exists(), "no snapshot is written for a refused state");

    let _ = std::fs::remove_dir_all(&dir);
}
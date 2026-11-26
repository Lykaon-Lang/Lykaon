//! M5g item b: deterministic mode (virtual clock, fixed seed, native).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

const BIN: &str = env!("CARGO_BIN_EXE_lykaonc-cli");

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_det_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).expect("write program");
}

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(BIN)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run lykaonc-cli");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// Run with the interpreter and the virtual clock (fast; no native build).
fn run_det(args: &[&str]) -> (i32, String) {
    let mut full = vec!["run", "--deterministic", "--interpret"];
    full.extend_from_slice(args);
    run(&full)
}

const CLOCK_SRC: &str = "\
Say now.
Say today.
Say random number from 1 to 1000.
Sleep for 2000 milliseconds.
After 10 milliseconds.
    Say later.
Say done.
";

#[test]
fn a_thousand_deterministic_runs_are_byte_identical() {
    let dir = temp("thousand");
    let prog = dir.join("d.lykaon");
    write(&prog, CLOCK_SRC);

    let mut first: Option<String> = None;
    for i in 0..1000 {
        let (code, out) = run_det(&[prog.to_str().unwrap()]);
        assert_eq!(code, 0, "run {i} failed: {out}");
        match &first {
            None => first = Some(out),
            Some(expected) => assert_eq!(
                &out, expected,
                "run {i} differs from the first deterministic run"
            ),
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_virtual_clock_does_not_move_during_computation() {
    let dir = temp("compute");
    let prog = dir.join("twice.lykaon");
    write(&prog, "Say now.\nSay now.\n");

    let (code, det) = run_det(&[prog.to_str().unwrap()]);
    assert_eq!(code, 0, "{det}");
    let lines: Vec<&str> = det.lines().collect();
    assert_eq!(lines.len(), 2, "{det}");
    assert_eq!(lines[0], lines[1], "pure computation must not tick the clock");

    let (code, real) = run(&["run", "--interpret", prog.to_str().unwrap()]);
    assert_eq!(code, 0, "{real}");
    let lines: Vec<&str> = real.lines().collect();
    assert_ne!(
        lines[0], lines[1],
        "the real clock should move between two reads"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_seed_changes_the_draws_but_not_the_clock() {
    let dir = temp("seed");
    let prog = dir.join("rand.lykaon");
    write(&prog, "Say now.\nSay random number from 1 to 1000.\n");

    let (code1, one) = run_det(&["--seed", "1", prog.to_str().unwrap()]);
    let (code2, two) = run_det(&["--seed", "2", prog.to_str().unwrap()]);
    assert_eq!(code1, 0, "{one}");
    assert_eq!(code2, 0, "{two}");
    assert_ne!(one, two, "a different seed must change the draws");
    assert_eq!(
        one.lines().next(),
        two.lines().next(),
        "the clock must not depend on the seed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_recorded_sleep_is_not_waited_for() {
    let dir = temp("sleep");
    let prog = dir.join("nap.lykaon");
    write(&prog, "Sleep for 3 seconds.\nSay done.\n");

    let started = Instant::now();
    let (code, out) = run_det(&[prog.to_str().unwrap()]);
    let elapsed = started.elapsed();
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "done");
    assert!(
        elapsed < std::time::Duration::from_millis(2500),
        "a deterministic sleep must not wait {elapsed:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_timers_fire_on_the_virtual_calendar() {
    let dir = temp("every");
    let prog = dir.join("every.lykaon");
    write(
        &prog,
        "Every 5 milliseconds.\n    Say tick.\nSleep for 12 milliseconds.\nSay done.\n",
    );

    let mut first: Option<String> = None;
    for i in 0..50 {
        let (code, out) = run_det(&[prog.to_str().unwrap()]);
        assert_eq!(code, 0, "run {i}: {out}");
        match &first {
            None => first = Some(out.clone()),
            Some(expected) => assert_eq!(&out, expected, "run {i} differs"),
        }
    }
    let out = first.expect("output");
    assert_eq!(out, "tick\ntick\ndone\n", "5 ms and 10 ms rounds");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deterministic_recording_replays_byte_identically() {
    let dir = temp("record");
    let prog = dir.join("d.lykaon");
    // No timers: trace v1 refuses timers on replay by design.
    write(
        &prog,
        "Say now.\nSay today.\nSay random number from 1 to 1000.\nSleep for 2000 milliseconds.\nSay done.\n",
    );
    let trace = dir.join("run.trace.json");

    let (code, recorded) = run(&[
        "run",
        "--deterministic",
        "--record",
        trace.to_str().unwrap(),
        prog.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{recorded}");

    let (code, replayed) = run(&[
        "run",
        "--replay",
        trace.to_str().unwrap(),
        prog.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{replayed}");
    assert_eq!(replayed, recorded);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fuel_and_deterministic_can_be_combined() {
    let dir = temp("fuel");
    let prog = dir.join("loop.lykaon");
    write(
        &prog,
        "Changeable t is 0.\nRepeat 1000 times.\n    Set t to t plus 1.\nSay t.\n",
    );

    let (code, out) = run_det(&["--fuel", "5", prog.to_str().unwrap()]);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("ran for too long"), "{out}");

    let (code, out) = run_det(&["--fuel", "2004", prog.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "1000");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_watchers_are_refused_because_they_are_not_deterministic() {
    let dir = temp("watcher");
    let prog = dir.join("watch.lykaon");
    write(
        &prog,
        "When file wend changes.\n    Say changed.\nSay done.\n",
    );

    let (code, out) = run_det(&[prog.to_str().unwrap()]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("File watchers are not deterministic"),
        "{out}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limit_time_runs_on_the_virtual_clock() {
    let dir = temp("limit");
    let prog = dir.join("lim.lykaon");
    write(&prog, "Limit time 1 seconds.\nSleep for 2 seconds.\nSay done.\n");

    let (code, out) = run_det(&[prog.to_str().unwrap()]);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("Time limit reached"), "{out}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_deterministic_matches_the_interpreter() {
    let dir = temp("native");
    let prog = dir.join("d.lykaon");
    write(
        &prog,
        concat!(
            "Say now.\n",
            "Say today.\n",
            "Say random number from 1 to 1000.\n",
            "Changeable x is 0.1.\n",
            "Repeat 1000 times.\n",
            "    Set x to x times 1.0000001.\n",
            "    Set x to x plus 0.0000001.\n",
            "Say x.\n",
            "Sleep for 2000 milliseconds.\n",
            "Say done.\n",
        ),
    );

    let exe = dir.join("prog.exe");
    let built = Command::new(BIN)
        .arg("build")
        .arg("fast")
        .arg(&prog)
        .arg(&exe)
        .output()
        .expect("run native build");
    assert!(
        built.status.success(),
        "native build failed: {} {}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(exe.is_file(), "the native build did not produce {exe:?}");

    let (ic, interp) = run_det(&[prog.to_str().unwrap()]);
    assert_eq!(ic, 0, "{interp}");

    let mut first: Option<String> = None;
    for _ in 0..1000 {
        let out = Command::new(&exe)
            .env("LYKAON_DETERMINISTIC", "1")
            .env("LYKAON_SEED", "42")
            .stdin(Stdio::null())
            .output()
            .expect("run native program");
        assert!(
            out.status.success(),
            "native run failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        match &first {
            None => first = Some(text.clone()),
            Some(expected) => assert_eq!(&text, expected, "native run differs"),
        }
        assert_eq!(text, interp, "native and interpreter differ");
    }

    let _ = std::fs::remove_dir_all(&dir);
}
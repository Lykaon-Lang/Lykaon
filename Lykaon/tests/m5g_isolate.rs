//! M5g item e: `lykaon run --isolate` OS-level limits and the escape suite.
//!
//! Every test drives the real binary (`lykaonc-cli`), so the Windows Job
//! Object and the Unix rlimits are exercised through the shipped code path.
//! The limits are opt-in; the parity test proves the flag does not change a
//! normal run.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_lykaonc-cli");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_iso_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create dir");
    dir
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("create");
    f.write_all(text.as_bytes()).expect("write");
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("spawn lykaonc-cli")
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The same program under `--isolate` and on the plain interpreter path must
/// be byte-identical: the flag adds a container, not a different run.
#[test]
fn isolated_run_matches_the_plain_run() {
    let dir = temp_dir("parity");
    let prog = write(
        &dir,
        "hello.lykaon",
        "Changeable n is 1.\nRepeat 5 times.\n    Say n.\n    Set n to n plus 1.\n",
    );
    let path = prog.to_str().expect("utf8 path");
    let plain = run(&["run", "--interpret", path]);
    let isolated = run(&["run", "--isolate", path]);
    assert_eq!(plain.status.code(), Some(0), "{}", stderr_of(&plain));
    assert_eq!(isolated.status.code(), Some(0), "{}", stderr_of(&isolated));
    assert_eq!(stdout_of(&plain), "1\n2\n3\n4\n5\n");
    assert_eq!(stdout_of(&plain), stdout_of(&isolated));
    assert!(stderr_of(&isolated).is_empty(), "{}", stderr_of(&isolated));
    let _ = fs::remove_dir_all(&dir);
}

/// The wall-clock cap stops a loop that would otherwise never end, and it
/// stops the whole child (not just the sentence).
#[test]
fn time_limit_stops_an_infinite_loop() {
    let dir = temp_dir("time");
    let prog = write(
        &dir,
        "spin.lykaon",
        "Changeable n is 0.\nRepeat forever.\n    Set n to n plus 1.\n",
    );
    let start = Instant::now();
    let out = run(&[
        "run",
        "--isolate",
        "--cpu-seconds",
        "1",
        prog.to_str().expect("utf8 path"),
    ]);
    let elapsed = start.elapsed();
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    assert!(
        stderr_of(&out).contains("T0612"),
        "expected the time limit message, got: {}",
        stderr_of(&out)
    );
    // The limit is one second; the poll loop adds at most a few hundred ms.
    assert!(
        elapsed < Duration::from_secs(15),
        "the limit took {elapsed:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The output cap is a hard byte cap on what the parent relays, and the
/// child stops itself with T0613.
#[test]
fn output_limit_stops_a_talkative_program() {
    let dir = temp_dir("output");
    let prog = write(
        &dir,
        "noisy.lykaon",
        "Repeat 500 times.\n    Say this line is long enough to fill a small cap quickly.\n",
    );
    let out = run(&[
        "run",
        "--isolate",
        "--max-output-bytes",
        "400",
        prog.to_str().expect("utf8 path"),
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    // The interpreter reports a failed run on the program's own output
    // stream (the merged one), so the message can be on either side.
    let text = format!("{}{}", stdout_of(&out), stderr_of(&out));
    assert!(
        text.contains("T0613"),
        "expected the output limit message, got: {text}"
    );
    assert!(
        out.stdout.len() <= 400,
        "relayed {} bytes, cap was 400",
        out.stdout.len()
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The memory cap is enforced by the OS (a job object on Windows, an
/// address-space rlimit on Linux): a read far larger than the cap fails,
/// while the same read with a cap above it succeeds.
#[test]
fn memory_limit_stops_an_oversized_read() {
    let dir = temp_dir("memory");
    // A sparse file: one gigabyte of zeros at no disk cost.
    let big = dir.join("big");
    let f = fs::File::create(&big).expect("create big");
    f.set_len(1024 * 1024 * 1024).expect("set len");
    drop(f);
    let prog = write(&dir, "read.lykaon", "Read file big.\n");
    let path = prog.to_str().expect("utf8 path");

    // 256 MB is below the read; the program cannot even allocate the text.
    let capped = run(&[
        "run",
        "--isolate",
        "--memory-mb",
        "256",
        "--cpu-seconds",
        "30",
        path,
    ]);
    let err = stderr_of(&capped);
    assert_ne!(
        capped.status.code(),
        Some(0),
        "the big read should fail under the cap: {err}"
    );
    assert!(
        !err.contains("cannot start"),
        "the capped child must run, not fail to start: {err}"
    );

    // With a cap above the read the same sentence succeeds (the file is
    // zeros, so nothing is printed).
    let roomy = run(&[
        "run",
        "--isolate",
        "--memory-mb",
        "2048",
        "--cpu-seconds",
        "60",
        path,
    ]);
    assert_eq!(
        roomy.status.code(),
        Some(0),
        "the same read must fit under a roomier cap: {}",
        stderr_of(&roomy)
    );
    assert!(stdout_of(&roomy).is_empty());
    let _ = fs::remove_dir_all(&dir);
}

/// A program that would run forever is stopped; a program that ends on its
/// own is not, even with the default caps.
#[test]
fn default_caps_do_not_disturb_a_normal_run() {
    let dir = temp_dir("default");
    let prog = write(&dir, "ok.lykaon", "Say still fine.\n");
    let out = run(&["run", "--isolate", prog.to_str().expect("utf8 path")]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr_of(&out));
    assert_eq!(stdout_of(&out), "still fine\n");
    let _ = fs::remove_dir_all(&dir);
}
/// The job's active-process cap is an OS guarantee, not a language check:
/// a helper that starts four children fits under a roomy cap and fails
/// under a cap of two. On Linux this needs a writable cgroups v2; where
/// there is none the test says so and does not pretend.
#[test]
fn process_cap_is_enforced_by_the_os() {
    if !lykaon::isolate::process_cap_ready() {
        eprintln!("skipping: this host cannot enforce a process cap");
        return;
    }
    let helper = env!("CARGO_BIN_EXE_iso_spawn_helper");
    let exe = std::path::Path::new(helper);
    let roomy = lykaon::isolate::Limits {
        memory_mb: 0,
        cpu_seconds: 30,
        max_output_bytes: 1 << 20,
        max_processes: 8,
    };
    let out = lykaon::isolate::run_exe(exe, &["4".to_string()], &roomy);
    assert_eq!(
        out.code,
        0,
        "a roomy cap should let all four children run"
    );

    let tight = lykaon::isolate::Limits {
        max_processes: 2,
        ..roomy
    };
    let out = lykaon::isolate::run_exe(exe, &["4".to_string()], &tight);
    assert_ne!(
        out.code, 0,
        "a cap of 2 must refuse the second child"
    );
}

/// Landlock must block a read that leaves the program's folder: a symlink
/// inside the folder points at a file in the home directory. Without
/// `--isolate` the read succeeds; with it the kernel refuses.
#[cfg(target_os = "linux")]
#[test]
fn landlock_blocks_a_read_outside_the_program_folder() {
    if !lykaon::isolate::landlock_ready() {
        eprintln!("skipping: Landlock is not available on this kernel");
        return;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let dir = temp_dir("landlock");
    let secret = std::path::PathBuf::from(&home)
        .join(format!(".lykaon_iso_secret_{}", std::process::id()));
    fs::write(&secret, "top-secret\n").expect("write secret");
    let link = dir.join("link");
    std::os::unix::fs::symlink(&secret, &link).expect("symlink");
    let prog = write(&dir, "prog.lykaon", "Read file link.\nSay it.\n");
    let path = prog.to_str().expect("utf8 path");

    let plain = run(&["run", "--interpret", path]);
    assert_eq!(plain.status.code(), Some(0), "{}", stderr_of(&plain));
    assert!(stdout_of(&plain).contains("top-secret"), "{}", stdout_of(&plain));

    let isolated = run(&["run", "--isolate", path]);
    assert_ne!(
        isolated.status.code(),
        Some(0),
        "Landlock must refuse the read outside the program folder: {}",
        stdout_of(&isolated)
    );
    let _ = fs::remove_file(&secret);
    let _ = fs::remove_dir_all(&dir);
}

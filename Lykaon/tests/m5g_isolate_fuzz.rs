//! M5g item e: boundary fuzzing for `lykaon run --isolate`.
//!
//! The caps are varied around their edges (small memory, one-second
//! deadlines, tiny output budgets, tight process counts) and the programs
//! are the four shapes that hurt: a loop, a talkative program, an oversized
//! read and a normal run. Every iteration must finish inside the deadline,
//! keep the relayed output within the cap, and exit through the CLI's own
//! exit codes (never a crash code, never a panic).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_lykaonc-cli");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_isofuzz_{name}_{}", std::process::id()));
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

fn run_isolated(file: &Path, extra: &[&str]) -> Output {
    let mut args: Vec<&str> = vec!["run", "--isolate"];
    args.extend_from_slice(extra);
    args.push(file.to_str().expect("utf8 path"));
    Command::new(BIN)
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .expect("spawn lykaonc-cli")
}

fn text_of(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn isolate_boundaries_hold_under_random_caps() {
    let dir = temp_dir("main");
    // A sparse gigabyte: reads far above every memory cap in the loop.
    let big = dir.join("big");
    let f = fs::File::create(&big).expect("create big");
    f.set_len(1024 * 1024 * 1024).expect("set len");
    drop(f);

    let programs = vec![
        write(&dir, "hello.lykaon", "Say Hello.\n"),
        write(
            &dir,
            "spin.lykaon",
            "Changeable n is 0.\nRepeat forever.\n    Set n to n plus 1.\n",
        ),
        write(
            &dir,
            "noisy.lykaon",
            "Repeat 500 times.\n    Say this line is long enough to fill a small cap quickly.\n",
        ),
        write(&dir, "read.lykaon", "Read file big.\n"),
        write(&dir, "exit.lykaon", "Exit with 7.\n"),
        write(&dir, "bad.lykaon", "Say (.\n"),
        write(&dir, "count.lykaon", "Changeable n is 1.\nRepeat 4 times.\n    Say n.\n    Set n to n plus 1.\n"),
    ];

    let mut seed: u64 = 0x5eed_1234_abcd_0001;
    for i in 0..24 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let memory_mb = 64 + seed % 256; // 64..320
        let cpu_seconds = 1 + (seed >> 8) % 3; // 1..3
        let max_output = 100 + (seed >> 16) % 1500; // 100..1600
        let prog = &programs[(seed >> 24) as usize % programs.len()];
        let extra = [
            "--memory-mb",
            &memory_mb.to_string(),
            "--cpu-seconds",
            &cpu_seconds.to_string(),
            "--max-output-bytes",
            &max_output.to_string(),
        ];
        let start = Instant::now();
        let out = run_isolated(prog, &extra);
        let elapsed = start.elapsed();
        let text = text_of(&out);
        assert!(
            !text.contains("panicked"),
            "iteration {i} panicked: {text}"
        );
        assert!(
            elapsed < Duration::from_secs(cpu_seconds + 15),
            "iteration {i} ({:?}) took {elapsed:?}",
            prog.file_name().unwrap_or_default()
        );
        let code = out.status.code();
        assert!(
            matches!(code, Some(0..=255)),
            "iteration {i} exited with {code:?}, not a CLI code"
        );
        assert!(
            out.stdout.len() <= max_output as usize,
            "iteration {i} relayed {} bytes, cap {max_output}",
            out.stdout.len()
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn isolate_flag_parser_never_panics_on_bad_values() {
    let dir = temp_dir("flags");
    let prog = write(&dir, "ok.lykaon", "Say ok.\n");
    let path = prog.to_str().expect("utf8 path").to_string();
    let cases: Vec<Vec<String>> = vec![
        vec!["--memory-mb".into()],
        vec!["--memory-mb".into(), "abc".into()],
        vec!["--cpu-seconds".into(), "-1".into()],
        vec!["--max-output-bytes".into(), "999999999999999999999".into()],
        vec!["--max-processes".into(), "0".into()],
        vec!["--nonsense".into(), "x".into()],
        vec!["--memory-mb".into(), "64".into(), "--cpu-seconds".into(), "1".into()],
    ];
    for extra in cases {
        let mut args: Vec<&str> = vec!["run", "--isolate"];
        for a in &extra {
            args.push(a.as_str());
        }
        args.push(path.as_str());
        let out = Command::new(BIN)
            .args(&args)
            .stdin(Stdio::null())
            .output()
            .expect("spawn");
        let text = text_of(&out);
        assert!(!text.contains("panicked"), "flags {extra:?}: {text}");
        assert!(
            matches!(out.status.code(), Some(0..=255)),
            "flags {extra:?} exited with {:?}",
            out.status.code()
        );
    }
    let _ = fs::remove_dir_all(&dir);
}
//! M5g item c: trace v1 record/replay over the real CLI.

use serde_json::{json, Value};
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

const BIN: &str = env!("CARGO_BIN_EXE_lykaonc-cli");

struct Run {
    code: i32,
    out: String,
    err: String,
}

fn cli(args: &[&str], stdin: &str) -> Run {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lykaonc-cli");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    Run {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_trace_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).expect("write file");
}

fn trace_lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .expect("trace exists")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("trace line is JSON"))
        .collect()
}

fn rewrite(path: &Path, lines: &[Value]) {
    let text = lines
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    write(path, &text);
}

#[test]
fn record_then_replay_is_byte_identical() {
    let dir = temp("basic");
    let prog = dir.join("demo.lykaon");
    write(
        &prog,
        "Ask name.\nSay name.\nSay now.\nSay random number from 1 to 1000.\n",
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "Ada\n",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert!(recorded.out.starts_with("Ada\n"), "{}", recorded.out);

    for _ in 0..2 {
        let replayed = cli(
            &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
            "",
        );
        assert_eq!(replayed.code, 0, "{}", replayed.err);
        assert_eq!(
            replayed.out, recorded.out,
            "replay must reproduce the recording byte for byte"
        );
    }

    // The trace records the outside-world reads, not just stdin.
    let lines = trace_lines(&trace);
    assert_eq!(lines[0]["format"], "lykaon-trace");
    assert_eq!(lines[0]["version"], 1);
    let kinds: Vec<&str> = lines
        .iter()
        .filter(|l| l["t"] == "event")
        .map(|l| l["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"stdin"), "{kinds:?}");
    assert!(kinds.contains(&"clock"), "{kinds:?}");
    assert!(kinds.contains(&"random"), "{kinds:?}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tampered_stdin_is_detected() {
    let dir = temp("stdin");
    let prog = dir.join("demo.lykaon");
    write(&prog, "Ask name.\nSay name.\n");
    let trace = dir.join("run.trace.json");
    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "Ada\n",
    );
    assert_eq!(recorded.code, 0);

    let mut lines = trace_lines(&trace);
    for line in &mut lines {
        if line["kind"] == "stdin" {
            line["value"] = json!("Bob\n");
        }
    }
    rewrite(&trace, &lines);

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1, "a changed input must not pass silently");
    assert!(
        replayed.err.contains("Trace replay diverged") || replayed.out.contains("Trace replay diverged"),
        "stdout: {}\nstderr: {}",
        replayed.out,
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tampered_clock_is_detected() {
    let dir = temp("clock");
    let prog = dir.join("demo.lykaon");
    write(&prog, "Say now.\n");
    let trace = dir.join("run.trace.json");
    assert_eq!(
        cli(
            &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
            ""
        )
        .code,
        0
    );

    let mut lines = trace_lines(&trace);
    for line in &mut lines {
        if line["kind"] == "clock" {
            let nanos: i128 = line["nanos"].as_str().unwrap().parse().unwrap();
            line["nanos"] = json!((nanos + 86_400_000_000_000).to_string());
        }
    }
    rewrite(&trace, &lines);

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1);
    assert!(
        replayed.err.contains("Trace replay diverged") || replayed.out.contains("Trace replay diverged"),
        "stdout: {}\nstderr: {}",
        replayed.out,
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changed_file_is_detected() {
    let dir = temp("file");
    let prog = dir.join("read.lykaon");
    write(&prog, "Read file data.\nSay it.\n");
    let data = dir.join("data");
    write(&data, "hello\n");
    let trace = dir.join("run.trace.json");
    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out.trim(), "hello");

    // The trace stores the content, so a changed file on disk is ignored.
    write(&data, "HELLO\n");
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(
        replayed.out, recorded.out,
        "replay reads the recorded content, not the disk"
    );

    // Tampering with the stored content is different: the hash check fires.
    let mut lines = trace_lines(&trace);
    for line in &mut lines {
        if line["kind"] == "file_read" {
            line["body"] = json!("HELLO\n");
        }
    }
    rewrite(&trace, &lines);
    let tampered = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(tampered.code, 1);
    assert!(
        tampered.out.contains("Trace replay diverged")
            || tampered.err.contains("Trace replay diverged"),
        "stdout: {}\nstderr: {}",
        tampered.out,
        tampered.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn leftover_events_are_detected() {
    let dir = temp("leftover");
    let prog = dir.join("rand.lykaon");
    write(&prog, "Say random number from 1 to 10.\n");
    let trace = dir.join("run.trace.json");
    assert_eq!(
        cli(
            &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
            ""
        )
        .code,
        0
    );

    let mut lines = trace_lines(&trace);
    let end = lines.pop().expect("end record");
    lines.push(json!({"t": "event", "kind": "random", "value": 1}));
    lines.push(end);
    rewrite(&trace, &lines);

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1);
    assert!(
        replayed.err.contains("were not used") || replayed.out.contains("were not used"),
        "stdout: {}\nstderr: {}",
        replayed.out,
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn env_is_recorded_and_tampering_is_detected() {
    let dir = temp("env");
    let prog = dir.join("env.lykaon");
    write(&prog, "Say environment path.\n");
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    let lines = trace_lines(&trace);
    assert!(
        lines
            .iter()
            .any(|l| l["kind"] == "env" && l["name"] == "PATH"),
        "the environment read is in the trace"
    );

    let mut lines = lines;
    for line in &mut lines {
        if line["kind"] == "env" && line["name"] == "PATH" {
            line["value"] = json!("TAMPERED");
        }
    }
    rewrite(&trace, &lines);

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1);
    assert!(
        replayed.err.contains("Trace replay diverged") || replayed.out.contains("Trace replay diverged"),
        "stdout: {}\nstderr: {}",
        replayed.out,
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bytes_reads_are_traced() {
    let dir = temp("bytes");
    let prog = dir.join("bytes.lykaon");
    write(&prog, "Read bytes of file data.\nSay them.\n");
    let data = dir.join("data");
    write(&data, "hi");
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert!(recorded.out.contains("104"));
    assert!(
        trace_lines(&trace)
            .iter()
            .any(|l| l["kind"] == "file_read" && l["len"] == 2),
        "the bytes read is in the trace"
    );

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn csv_load_replays_without_the_file() {
    let dir = temp("csv");
    write(
        &dir.join("sales.csv"),
        "region,size,price,churned\nnorth,55,140,yes\nnorth,40,100,no\neast,60,160,yes\n",
    );
    let prog = dir.join("load.lykaon");
    write(&prog, "Load sales.\nSay rows.\n");
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out.trim(), "rows 3");

    // The CSV is gone: the content must come from the trace.
    std::fs::remove_file(dir.join("sales.csv")).unwrap();
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);
    assert!(
        trace_lines(&trace)
            .iter()
            .any(|l| l["kind"] == "file_read" && l["encoding"] == "utf8"),
        "the CSV content is in the trace"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn model_save_and_load_replay_without_files() {
    let dir = temp("model");
    write(
        &dir.join("sales.csv"),
        "region,size,price,churned\nnorth,55,140,yes\nnorth,40,100,no\neast,60,160,yes\neast,35,90,no\nsouth,45,125,no\nsouth,30,80,no\nwest,50,130,no\nwest,25,70,yes\n",
    );
    let prog = dir.join("model.lykaon");
    write(
        &prog,
        "Load sales.\nPredict churned from the rest.\nSave model as m5aimodel.\nLoad model m5aimodel.\nSay prediction for size 40 and price 100.\n",
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert!(dir.join("output").join("m5aimodel.model").is_file());

    std::fs::remove_file(dir.join("sales.csv")).unwrap();
    std::fs::remove_file(dir.join("output").join("m5aimodel.model")).unwrap();
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);
    assert!(
        !dir.join("output").join("m5aimodel.model").exists(),
        "the replay must not re-save the model"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_is_skipped_and_its_bytes_are_verified() {
    let dir = temp("write");
    let prog = dir.join("w.lykaon");
    write(
        &prog,
        "Write 42 to file outfile.\nRead file outfile.\nSay it.\n",
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out.trim(), "42");
    std::fs::remove_file(dir.join("outfile")).unwrap();

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);
    assert!(
        !dir.join("outfile").exists(),
        "a replay must not write the file again"
    );

    // Tamper with the recorded bytes: the replay must refuse.
    let mut lines = trace_lines(&trace);
    for line in &mut lines {
        if line["kind"] == "file_write" {
            line["sha256"] = json!("0".repeat(64));
        }
    }
    rewrite(&trace, &lines);
    let tampered = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(tampered.code, 1);
    assert!(
        tampered.err.contains("Trace replay diverged")
            || tampered.out.contains("Trace replay diverged"),
        "stdout: {}\nstderr: {}",
        tampered.out,
        tampered.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_and_replace_replay_without_the_file() {
    let dir = temp("append");
    let prog = dir.join("log.lykaon");
    write(
        &prog,
        "Write 1 to file logtrace in temporary folder.\nAppend 2 to file logtrace in temporary folder.\nReplace file logtrace in temporary folder with 3.\nRead file logtrace in temporary folder.\nSay it.\n",
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out.trim(), "3");
    let _ = std::fs::remove_file(std::env::temp_dir().join("logtrace"));

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn temp_file_path_is_stable_on_replay() {
    let dir = temp("tempfile");
    let prog = dir.join("t.lykaon");
    write(
        &prog,
        "Make temporary file scratch.\nWrite 11 to file scratch.\nRead file scratch.\nSay it.\nSay scratch.\n",
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert!(recorded.out.contains("scratch_"));

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(
        replayed.out, recorded.out,
        "the temporary path must be the recorded one"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remove_is_not_performed_on_replay() {
    let dir = temp("remove");
    let keep = dir.join("keep");
    write(&keep, "important");
    let prog = dir.join("r.lykaon");
    write(&prog, "Remove file keep.\nSay done.\n");
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert!(!keep.exists(), "the recording run removes the file");

    write(&keep, "important");
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);
    assert!(keep.exists(), "a replay must not delete anything");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn for_each_line_replays_without_the_file() {
    let dir = temp("foreach");
    write(&dir.join("lines"), "a\nb\nc\n");
    let prog = dir.join("each.lykaon");
    write(&prog, "For each line in file lines.\n    Say line.\n");
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out, "a\nb\nc\n");

    std::fs::remove_file(dir.join("lines")).unwrap();
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sqlite_transcript_replays_without_the_database() {
    let dir = temp("sqlite");
    let db = dir.join("shop");
    lykaon::db::close("setup");
    lykaon::db::open_sqlite("setup", db.to_str().unwrap()).expect("open setup db");
    lykaon::db::execute_sql(
        "setup",
        "CREATE TABLE stock (name TEXT, count INTEGER, price INTEGER)",
    )
    .expect("create table");
    lykaon::db::close("setup");

    let prog = dir.join("shop.lykaon");
    write(
        &prog,
        "A stockitem has name as text and count as whole number and price as whole number.\n\
         Open database shop using sqlite file shop.\n\
         Add a stockitem with name apples and count 5 and price 3 to table stock of shop.\n\
         Load stock from database shop.\n\
         Say rows.\n\
         Set count to 9 in rows of stock of shop where name is apples.\n\
         Load stock from database shop.\n\
         Say rows.\n\
         Remove rows where name is apples from table stock of shop.\n\
         Load stock from database shop.\n\
         Say rows.\n",
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out, "rows 1\nrows 1\nrows 0\n");
    assert!(db.is_file());

    std::fs::remove_file(&db).unwrap();
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out, recorded.out);
    assert!(!db.exists(), "the replay must not recreate the database");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_cookie_is_recorded_and_replayed() {
    use std::io::{Read as _, Write as _};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    const PORT: u16 = 23777;
    let dir = temp("cookie");
    let prog = dir.join("server.lykaon");
    write(
        &prog,
        &format!(
            "Serve on port {PORT}.\n    When someone gets home.\n        Put hello in session under name.\n        Respond with value of name in session.\n"
        ),
    );
    let trace = dir.join("run.trace.json");

    fn start(args: &[&str]) -> std::process::Child {
        Command::new(BIN)
            .arg("run")
            .args(args)
            .env("LYKAON_SERVE_MAX_REQUESTS", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn server")
    }

    fn cookie() -> String {
        let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
        for _ in 0..300 {
            if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
                let _ = stream.write_all(
                    b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                );
                let mut text = String::new();
                let _ = stream.read_to_string(&mut text);
                let found = text.lines().find(|l| {
                    l.to_ascii_lowercase().starts_with("set-cookie:")
                });
                if let Some(line) = found {
                    return line
                        .split("lykaon_session=")
                        .nth(1)
                        .and_then(|v| v.split(';').next())
                        .expect("session cookie")
                        .to_string();
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the server never set a session cookie");
    }

    let mut recorded = start(&["--record", trace.to_str().unwrap(), prog.to_str().unwrap()]);
    let first = cookie();
    let rec_out = recorded.wait_with_output().expect("record server exits");
    assert!(rec_out.status.success(), "record server failed");

    let mut replayed = start(&["--replay", trace.to_str().unwrap(), prog.to_str().unwrap()]);
    let second = cookie();
    let rep_out = replayed.wait_with_output().expect("replay server exits");
    assert!(rep_out.status.success(), "replay server failed");

    assert_eq!(first, second, "the replay must reuse the recorded cookie");
    assert!(
        trace_lines(&trace).iter().any(|l| l["kind"] == "cookie"),
        "the cookie is in the trace"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn legacy_plain_text_replay_still_works() {
    let dir = temp("legacy");
    let prog = dir.join("ask.lykaon");
    write(&prog, "Ask name.\nSay name.\n");
    let input = dir.join("input.txt");
    write(&input, "Grace\n");

    let replayed = cli(
        &["run", "--replay", input.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out.trim(), "Grace");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_version_is_refused() {
    let dir = temp("version");
    let prog = dir.join("demo.lykaon");
    write(&prog, "Say 1.\n");
    let trace = dir.join("run.trace.json");
    write(
        &trace,
        "{\"format\":\"lykaon-trace\",\"version\":2,\"program_sha256\":\"\"}\n",
    );

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1);
    assert!(
        replayed.err.contains("unsupported trace version 2"),
        "{}",
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trace_from_another_program_is_refused() {
    let dir = temp("program");
    let prog = dir.join("a.lykaon");
    write(&prog, "Say 1.\n");
    let other = dir.join("b.lykaon");
    write(&other, "Say 2.\n");
    let trace = dir.join("run.trace.json");
    assert_eq!(
        cli(
            &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
            ""
        )
        .code,
        0
    );

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), other.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1);
    assert!(
        replayed.err.contains("different program"),
        "{}",
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn record_and_replay_together_is_a_usage_error() {
    let dir = temp("both");
    let prog = dir.join("demo.lykaon");
    write(&prog, "Say 1.\n");
    let trace = dir.join("run.trace.json");
    let run = cli(
        &[
            "run",
            "--record",
            trace.to_str().unwrap(),
            "--replay",
            trace.to_str().unwrap(),
            prog.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(run.code, 2);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sleep_is_recorded_and_skipped_on_replay() {
    let dir = temp("sleep");
    let prog = dir.join("nap.lykaon");
    write(&prog, "Sleep for 2 seconds.\nSay done.\n");
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out.trim(), "done");
    assert!(
        trace_lines(&trace)
            .iter()
            .any(|l| l["kind"] == "sleep" && l["ms"] == 2000),
        "the sleep request is in the trace"
    );

    let started = Instant::now();
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    let elapsed = started.elapsed();
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out.trim(), "done");
    assert!(
        elapsed < std::time::Duration::from_millis(1200),
        "replay waited {elapsed:?}; a recorded sleep should not wait again"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn timers_are_refused_on_replay() {
    let dir = temp("timer");
    let prog = dir.join("timer.lykaon");
    write(&prog, "After 1 milliseconds.\n    Say later.\nSay done.\n");
    let trace = dir.join("run.trace.json");
    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);

    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 1);
    assert!(
        replayed.out.contains("timers are not replayable")
            || replayed.err.contains("timers are not replayable"),
        "stdout: {}\nstderr: {}",
        replayed.out,
        replayed.err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn net_replays_without_the_server() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
            );
        }
    });

    let dir = temp("net");
    let prog = dir.join("fetch.lykaon");
    write(
        &prog,
        &format!(
            "Fetch plain 127 dot 0 dot 0 dot 1 on port {port} home.\nanswer is it.\nSay status of answer.\nSay body of answer.\n"
        ),
    );
    let trace = dir.join("run.trace.json");

    let recorded = cli(
        &["run", "--record", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(recorded.code, 0, "{}", recorded.err);
    assert_eq!(recorded.out.trim(), "200\nhello");
    server.join().expect("server thread");

    // The listener is gone: replay must use the recorded response.
    let replayed = cli(
        &["run", "--replay", trace.to_str().unwrap(), prog.to_str().unwrap()],
        "",
    );
    assert_eq!(replayed.code, 0, "{}", replayed.err);
    assert_eq!(replayed.out.trim(), "200\nhello");
    assert!(
        trace_lines(&trace).iter().any(|l| l["kind"] == "net"),
        "the response is in the trace"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
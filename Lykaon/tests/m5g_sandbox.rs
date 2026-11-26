//! M5g: research sandbox pack (serve, sbom, fuel, record/replay, report).

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_lykaonc-cli");

/// Drive one server session: write every request, close stdin, read every
/// response line. The server exits on EOF, so this cannot hang on a missing
/// response (a missing line shows up as a shorter vector).
fn serve(args: &[&str], requests: &[&str]) -> Vec<Value> {
    let mut child = Command::new(BIN)
        .arg("serve")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn lykaonc-cli serve");
    {
        let mut stdin = child.stdin.take().expect("child stdin");
        for r in requests {
            writeln!(stdin, "{r}").expect("write request");
        }
    }
    let out = child.wait_with_output().expect("wait for serve");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("response is JSON"))
        .collect()
}

fn run_request(id: u64, code: &str, fuel: Option<u64>) -> String {
    let mut params = json!({ "code": code });
    if let Some(f) = fuel {
        params["fuel"] = json!(f);
    }
    json!({ "jsonrpc": "2.0", "id": id, "method": "run", "params": params }).to_string()
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_m5g_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn sbom_is_valid_cyclonedx() {
    let s = lykaon::sbom::sbom();
    let v: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
    assert_eq!(v["bomFormat"], "CycloneDX");
    assert!(v["metadata"]["component"]["name"].is_string());
}

#[test]
fn sbom_is_deterministic_and_has_a_dependency_graph() {
    let a = lykaon::sbom::sbom_in(Path::new("."));
    let b = lykaon::sbom::sbom_in(Path::new("."));
    assert_eq!(a, b, "same inputs must give byte-identical SBOMs");

    let v: Value = serde_json::from_str(&a).expect("valid JSON");
    assert_eq!(v["specVersion"], "1.5");
    assert!(v["serialNumber"]
        .as_str()
        .expect("serialNumber")
        .starts_with("urn:uuid:"));
    assert_eq!(
        v["metadata"]["tools"]["components"][0]["name"], "lykaonc-cli",
        "the tool that produced the BOM is recorded"
    );
    assert!(v["dependencies"].is_array(), "dependency graph present");
}

#[test]
fn sbom_reads_project_and_lock_without_changing_cwd() {
    let dir = temp_dir("sbom");
    std::fs::write(
        dir.join("Lykaon.toml"),
        "Name is demo.\nVersion is 1 point 2 point 3.\nNeeds wolf from git https://example.com/wolf at version 0 point 4 point 0.\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("lykaon.lock"),
        "Name is demo.\nVersion is 1 point 2 point 3.\nLocked wolf from git https://example.com/wolf at 0 point 4 point 0 with checksum ABCD12.\n",
    )
    .unwrap();

    let text = lykaon::sbom::sbom_in(&dir);
    let v: Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(v["metadata"]["component"]["name"], "demo");
    assert_eq!(v["metadata"]["component"]["version"], "1.2.3");
    assert_eq!(v["components"][0]["name"], "wolf");
    assert_eq!(v["components"][0]["version"], "0.4.0");
    assert_eq!(v["components"][0]["hashes"][0]["alg"], "SHA-256");
    assert_eq!(v["components"][0]["hashes"][0]["content"], "abcd12");
    assert_eq!(
        v["dependencies"][0]["dependsOn"][0], "pkg:generic/wolf@0.4.0",
        "root depends on the locked package"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sbom_output_flag_writes_the_file() {
    let dir = temp_dir("sbomout");
    let out = dir.join("bom.json");
    let status = Command::new(BIN)
        .arg("sbom")
        .arg("--output")
        .arg(&out)
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run sbom");
    assert!(status.status.success());
    assert!(status.stdout.is_empty(), "document goes to the file only");
    let text = std::fs::read_to_string(&out).expect("bom written");
    let v: Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(v["bomFormat"], "CycloneDX");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn serve_ping_run_and_check() {
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();
    let run = run_request(2, "Say 5.\n", None);
    let check = json!({
        "jsonrpc": "2.0", "id": 3, "method": "check",
        "params": { "code": "Say 5.\n" }
    })
    .to_string();
    let responses = serve(&[], &[&ping, &run, &check]);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["result"]["pong"], true);
    assert_eq!(responses[1]["result"]["output"].as_str().unwrap().trim(), "5");
    assert_eq!(responses[1]["result"]["exit"].as_i64().unwrap(), 0);
    assert_eq!(responses[1]["result"]["reason"], "ok");
    assert_eq!(
        responses[1]["result"]["fuel_used"].as_u64().unwrap(),
        1,
        "one Say sentence"
    );
    assert!(responses[1]["result"]["peak_alloc_bytes"].is_u64());
    assert!(responses[2]["result"]["diagnostics"].is_array());
}

#[test]
fn serve_sbom_method_returns_the_document() {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "sbom" }).to_string();
    let responses = serve(&[], &[&req]);
    assert_eq!(responses[0]["result"]["bomFormat"], "CycloneDX");
}

#[test]
fn serve_default_fuel_stops_a_loop() {
    let code = "Changeable t is 0.\nRepeat 1000 times.\n    Set t to t plus 1.\nSay t.\n";
    let req = run_request(1, code, None);
    let responses = serve(&["--fuel", "5"], &[&req]);
    assert_ne!(responses[0]["result"]["exit"].as_i64().unwrap(), 0);
    assert_eq!(responses[0]["result"]["reason"], "fuel");
    assert_eq!(responses[0]["result"]["fuel_used"].as_u64().unwrap(), 5);
    assert!(responses[0]["result"]["output"]
        .as_str()
        .unwrap()
        .contains("ran for too long"));
}

#[test]
fn serve_request_fuel_overrides_the_server_default() {
    let code = "Changeable t is 0.\nRepeat 1000 times.\n    Set t to t plus 1.\nSay t.\n";
    let req = run_request(1, code, Some(1_000_000));
    let responses = serve(&["--fuel", "5"], &[&req]);
    assert_eq!(responses[0]["result"]["exit"].as_i64().unwrap(), 0);
    assert_eq!(responses[0]["result"]["output"].as_str().unwrap().trim(), "1000");
    assert_eq!(responses[0]["result"]["fuel"].as_u64().unwrap(), 1_000_000);
    assert_eq!(responses[0]["result"]["reason"], "ok");
    assert!(
        responses[0]["result"]["fuel_used"].as_u64().unwrap() > 1000,
        "the loop's sentences are counted even under a large budget"
    );
}

#[test]
fn serve_rejects_a_too_large_request_and_keeps_going() {
    let long = json!({
        "jsonrpc": "2.0", "id": 1, "method": "run",
        "params": { "code": "Say 1.\n".repeat(80) }
    })
    .to_string();
    let ping = json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }).to_string();
    let responses = serve(&["--max-request-bytes", "64"], &[&long, &ping]);
    assert_eq!(responses[0]["error"]["code"].as_i64().unwrap(), -32600);
    assert_eq!(responses[1]["result"]["pong"], true, "server survives");
}

#[test]
fn serve_uses_jsonrpc_error_objects() {
    let unknown = json!({ "jsonrpc": "2.0", "id": 1, "method": "nope" }).to_string();
    let broken = "{not json".to_string();
    let no_params = json!({ "jsonrpc": "2.0", "id": 3, "method": "run" }).to_string();
    let responses = serve(&[], &[&unknown, &broken, &no_params]);
    assert_eq!(responses[0]["error"]["code"].as_i64().unwrap(), -32601);
    assert_eq!(responses[1]["error"]["code"].as_i64().unwrap(), -32700);
    assert_eq!(responses[2]["error"]["code"].as_i64().unwrap(), -32602);
}

#[test]
fn run_with_stdin_replays() {
    // the record/replay primitive: run_source with a fixed stdin.
    let (out, code) = lykaon::run_source("Ask name.\nSay name.\n", "t.lykaon", "Ada\n");
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "Ada");
}

#[test]
fn fuel_budget_stops_run() {
    // run_source_budgeted with a tiny budget must stop, not hang.
    let src = "Changeable t is 0.\nRepeat 1000 times.\n    Set t to t plus 1.\nSay t.\n";
    let (_out, code) = lykaon::run_source_budgeted(src, "t.lykaon", "", 5);
    assert_ne!(code, 0, "a 5-step budget must stop the loop");
}

fn report_schema() -> Value {
    serde_json::from_str(include_str!("../../docs/schemas/lykaon-report-v1.schema.json"))
        .expect("the published schema is JSON")
}

/// The document must satisfy the schema a consumer will validate against:
/// every required key present, and the exit reason in the published enum.
fn assert_report_matches_schema(report: &Value) {
    let schema = report_schema();
    assert_eq!(report["schema"], "lykaon-report-v1");
    for key in schema["required"].as_array().expect("schema.required") {
        let key = key.as_str().expect("required key is a string");
        assert!(
            report.get(key).is_some(),
            "report lacks required key {key}: {report}"
        );
    }
    let allowed: Vec<&str> = schema["properties"]["exit_reason"]["enum"]
        .as_array()
        .expect("schema exit_reason.enum")
        .iter()
        .map(|v| v.as_str().expect("reason is a string"))
        .collect();
    let reason = report["exit_reason"].as_str().expect("exit_reason");
    assert!(
        allowed.contains(&reason),
        "exit_reason {reason} is not in the published enum"
    );
}

fn report_command(program: &Path, extra: &[&str]) -> Value {
    let out = Command::new(BIN)
        .arg("report")
        .args(extra)
        .arg(program)
        .output()
        .expect("run report");
    assert!(
        out.status.success(),
        "report exits 0 when it can write the document; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("report prints one JSON document")
}

#[test]
fn report_matches_the_published_schema() {
    let dir = temp_dir("report");
    let prog = dir.join("ok.lykaon");
    std::fs::write(&prog, "Say 5.\n").unwrap();

    let report = report_command(&prog, &[]);
    assert_report_matches_schema(&report);
    assert_eq!(report["exit"].as_i64().unwrap(), 0);
    assert_eq!(report["exit_reason"], "ok");
    assert!(report["input_bytes"].is_number(), "input_bytes: {report}");
    assert!(report["permissions_used"].is_array(), "permissions_used: {report}");
    assert_eq!(report["output_bytes"].as_u64().unwrap(), 2);
    assert_eq!(report["fuel"].as_u64().unwrap(), 0, "0 = unlimited");
    assert!(
        report["fuel_used"].as_u64().unwrap() >= 1,
        "an unmetered run still counts its sentences"
    );
    assert!(
        report["peak_alloc_bytes"].as_u64().unwrap() > 0,
        "the report should observe the run's own allocations"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_fuel_stop_names_the_reason_and_exact_usage() {
    let dir = temp_dir("reportfuel");
    let prog = dir.join("loop.lykaon");
    std::fs::write(
        &prog,
        "Changeable t is 0.\nRepeat 1000 times.\n    Set t to t plus 1.\nSay t.\n",
    )
    .unwrap();

    let report = report_command(&prog, &["--fuel", "5"]);
    assert_report_matches_schema(&report);
    assert_ne!(report["exit"].as_i64().unwrap(), 0);
    assert_eq!(report["exit_reason"], "fuel");
    assert_eq!(report["fuel"].as_u64().unwrap(), 5);
    assert_eq!(
        report["fuel_used"].as_u64().unwrap(),
        5,
        "a budget of 5 runs exactly 5 sentences; the sixth is refused"
    );

    // Pin the off-by-one: 2,004 executed sentences finish, 2,003 do not.
    let exact = report_command(&prog, &["--fuel", "2004"]);
    assert_eq!(exact["exit_reason"], "ok");
    assert_eq!(exact["fuel_used"].as_u64().unwrap(), 2004);
    let short = report_command(&prog, &["--fuel", "2003"]);
    assert_eq!(short["exit_reason"], "fuel");
    assert_eq!(short["fuel_used"].as_u64().unwrap(), 2003);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_compile_error_is_named_and_ran_no_sentences() {
    let dir = temp_dir("reportcompile");
    let prog = dir.join("bad.lykaon");
    std::fs::write(&prog, "Set x to 1.\n").unwrap();

    let report = report_command(&prog, &[]);
    assert_report_matches_schema(&report);
    assert_eq!(report["exit"].as_i64().unwrap(), 1);
    assert_eq!(report["exit_reason"], "compile");
    assert_eq!(report["fuel_used"].as_u64().unwrap(), 0);
    assert!(
        report["output_bytes"].as_u64().unwrap() > 0,
        "the diagnostics are the output"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_output_flag_writes_the_document() {
    let dir = temp_dir("reportout");
    let prog = dir.join("ok.lykaon");
    let out = dir.join("report.json");
    std::fs::write(&prog, "Say 1.\n").unwrap();

    let status = Command::new(BIN)
        .arg("report")
        .arg(&prog)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run report --output");
    assert!(status.status.success());
    assert!(
        status.stdout.is_empty(),
        "the document goes to the file, not stdout"
    );
    let text = std::fs::read_to_string(&out).expect("report written");
    let report: Value = serde_json::from_str(&text).expect("valid JSON");
    assert_report_matches_schema(&report);
    assert_eq!(report["exit_reason"], "ok");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn metered_api_counts_sentences_without_a_budget() {
    let outcome = lykaon::run_source_metered("Say 6 times 7.\n", "t.lykaon", "");
    assert_eq!(outcome.exit, 0);
    assert_eq!(outcome.reason, lykaon::interp::ExitReason::Ok);
    assert_eq!(outcome.output.trim(), "42");
    assert_eq!(outcome.fuel_used, 1, "one Say sentence");

    let stopped = lykaon::run_source_budgeted_metered(
        "Changeable t is 0.\nRepeat 1000 times.\n    Set t to t plus 1.\nSay t.\n",
        "t.lykaon",
        "",
        5,
    );
    assert_eq!(stopped.reason, lykaon::interp::ExitReason::Fuel);
    assert_eq!(stopped.fuel_used, 5);
}

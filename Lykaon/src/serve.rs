//! M5g item g: `lykaon serve` — a warm JSON-RPC server over stdio.
//!
//! One JSON object per line. Methods:
//!   - `run`   {code, stdin?, fuel?} -> {output, exit, fuel, fuel_used, reason, peak_alloc_bytes, wall_ms}
//!   - `check` {code}                -> {diagnostics}
//!   - `sbom`                        -> the project's CycloneDX document
//!   - `ping`                        -> {pong: true}
//!
//! Each `run` compiles and executes on a 256 MiB worker thread (the evaluator
//! needs a large stack), so a long-lived server stays warm between calls.
//!
//! Sandbox knobs (all optional, off/unlimited by default):
//!   - `--fuel N`               default sentence budget for every `run`
//!   - `--max-request-bytes N`  reject a request line larger than N bytes
//!
//! A request may override the server budget with `params.fuel`; `0` means
//! unlimited. Protocol failures use the JSON-RPC 2.0 error object instead of a
//! result, so a client can tell a bad request from a program failure.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// Server-wide sandbox defaults.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Default `run` sentence budget (0 = unlimited).
    pub fuel: u64,
    /// Longest accepted request line, in bytes.
    pub max_request_bytes: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fuel: 0,
            max_request_bytes: 1024 * 1024,
        }
    }
}

fn run_code(code: &str, stdin_text: &str, fuel: u64) -> crate::RunOutcome {
    let code = code.to_string();
    let stdin_text = stdin_text.to_string();
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || crate::run_source_budgeted_metered(&code, "serve.lykaon", &stdin_text, fuel))
        .map_err(|e| e.to_string())
        .and_then(|h| h.join().map_err(|_| "worker panicked".to_string()))
        .unwrap_or_else(|e| crate::RunOutcome {
            output: format!("Error: {e}\n"),
            exit: 1,
            reason: crate::interp::ExitReason::Failure,
            fuel_used: 0,
            peak_alloc_bytes: 0,
            permissions_used: Vec::new(),
        })
}

enum Line {
    Eof,
    TooLong,
    Data(Vec<u8>),
}

/// Read one newline-terminated request without buffering an unbounded line.
fn read_request<R: BufRead>(reader: &mut R, max: usize) -> std::io::Result<Line> {
    let mut out: Vec<u8> = Vec::new();
    let mut too_long = false;
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            if out.is_empty() && !too_long {
                return Ok(Line::Eof);
            }
            return Ok(if too_long { Line::TooLong } else { Line::Data(out) });
        }
        if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            if !too_long {
                if out.len() + pos > max {
                    too_long = true;
                    out.clear();
                } else {
                    out.extend_from_slice(&buf[..pos]);
                }
            }
            reader.consume(pos + 1);
            return Ok(if too_long { Line::TooLong } else { Line::Data(out) });
        }
        let len = buf.len();
        if !too_long {
            if out.len() + len > max {
                too_long = true;
                out.clear();
            } else {
                out.extend_from_slice(buf);
            }
        }
        reader.consume(len);
    }
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message}
    })
}

pub fn serve(opts: Options) {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut out = std::io::stdout();
    loop {
        let line = match read_request(&mut reader, opts.max_request_bytes) {
            Ok(Line::Eof) => break,
            Ok(Line::TooLong) => {
                let _ = writeln!(
                    out,
                    "{}",
                    error(&Value::Null, -32600, "request too large")
                );
                let _ = out.flush();
                continue;
            }
            Ok(Line::Data(bytes)) => bytes,
            Err(_) => break,
        };
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let req: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                let _ = writeln!(out, "{}", error(&Value::Null, -32700, "parse error"));
                let _ = out.flush();
                continue;
            }
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        let response = match method {
            "run" => match params.get("code").and_then(|c| c.as_str()) {
                None => error(&id, -32602, "run needs params.code"),
                Some(code) => {
                    let stdin_text = params.get("stdin").and_then(|c| c.as_str()).unwrap_or("");
                    let fuel = params
                        .get("fuel")
                        .and_then(|f| f.as_u64())
                        .unwrap_or(opts.fuel);
                    let started = std::time::Instant::now();
                    let outcome = run_code(code, stdin_text, fuel);
                    let wall_ms = started.elapsed().as_millis() as u64;
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "output": outcome.output,
                            "exit": outcome.exit,
                            "fuel": fuel,
                            "fuel_used": outcome.fuel_used,
                            "reason": outcome.reason.as_str(),
                            "peak_alloc_bytes": outcome.peak_alloc_bytes,
                            "input_bytes": stdin_text.len(),
                            "permissions_used": outcome.permissions_used,
                            "wall_ms": wall_ms
                        }
                    })
                }
            },
            "check" => match params.get("code").and_then(|c| c.as_str()) {
                None => error(&id, -32602, "check needs params.code"),
                Some(code) => {
                    let diags: Vec<Value> = crate::check_diagnostics(code, "serve.lykaon")
                        .iter()
                        .map(|d| d.to_json())
                        .collect();
                    json!({"jsonrpc": "2.0", "id": id, "result": {"diagnostics": diags}})
                }
            },
            "sbom" => match serde_json::from_str::<Value>(&crate::sbom::sbom()) {
                Ok(doc) => json!({"jsonrpc": "2.0", "id": id, "result": doc}),
                Err(e) => error(&id, -32603, &format!("cannot build sbom: {e}")),
            },
            "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {"pong": true}}),
            "" => error(&id, -32600, "missing method"),
            other => error(&id, -32601, &format!("unknown method {other}")),
        };
        let _ = writeln!(out, "{response}");
        let _ = out.flush();
    }
}

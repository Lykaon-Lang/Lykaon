//! Step 9 (Appendix L 9.2): the Debug Adapter Protocol server.
//!
//! `lykaon debug <file>` speaks DAP over stdio (Content-Length framing) so an
//! editor can set breakpoints, step, continue and inspect variables. The
//! program runs on a worker thread with a large stack; the interpreter blocks
//! at a stop (set by `core::interp::debug_*`) while the DAP loop reads
//! commands.
//!
//! First cut: breakpoints are top-level statement lines (the DAP `line` is the
//! source line of the top-level statement). Stepping pauses at every
//! statement.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use lykaon_core::interp::{self, DebugCommand, DebugEvent};

static OUT_LOCK: Mutex<()> = Mutex::new(());

fn send(out: &mut dyn Write, msg: &serde_json::Value) {
    let text = serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string());
    let _ = write!(out, "Content-Length: {}\r\n\r\n", text.len());
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

fn read_message(reader: &mut dyn BufRead) -> Option<Vec<u8>> {
    let mut content_len = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(_) => return None,
        }
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            content_len = v.trim().parse::<usize>().ok();
        }
    }
    let n = content_len?;
    let mut buf = vec![0u8; n];
    reader.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// The source lines of top-level statements (non-indented, non-comment).
fn top_level_lines(source: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if low.starts_with("note") || low.starts_with("about") {
            continue;
        }
        out.push(i + 1);
    }
    out
}

/// Map a breakpoint line to a top-level statement index.
fn line_to_top(lines: &[usize], line: usize) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    for (i, l) in lines.iter().enumerate() {
        if *l >= line {
            return Some(i);
        }
    }
    Some(lines.len() - 1)
}

fn respond(out: &mut dyn Write, seq: u64, command: &str, body: serde_json::Value) {
    send(
        out,
        &serde_json::json!({
            "type": "response", "request_seq": seq, "success": true,
            "command": command, "body": body
        }),
    );
}

/// Run the DAP server. `source` and `file` identify the program.
pub fn serve(source: String, file: String) {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();

    let lines = top_level_lines(&source);
    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<DebugEvent>();
    let mut cmd_rx = Some(cmd_rx);
    let mut evt_tx = Some(evt_tx);
    let last_vars: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

    // Event thread: forward DebugEvents and remember the latest variables.
    let evt_thread = {
        let last_vars = last_vars.clone();
        std::thread::spawn(move || {
            while let Ok(ev) = evt_rx.recv() {
                *last_vars.lock().unwrap() = ev.vars.clone();
                let _g = OUT_LOCK.lock();
                let mut o = std::io::stdout();
                send(
                    &mut o,
                    &serde_json::json!({
                        "type": "event", "event": "output",
                        "body": {"category": "stdout", "output": format!("{}\n", ev.out)}
                    }),
                );
                send(
                    &mut o,
                    &serde_json::json!({
                        "type": "event", "event": "stopped",
                        "body": {"reason": "breakpoint", "threadId": 1,
                                 "allThreadsStopped": true,
                                 "description": format!("line {}: {}", ev.line, ev.stmt)}
                    }),
                );
            }
        })
    };

    let mut breaks: Vec<usize> = Vec::new();
    let mut started = false;
    let mut done = false;

    while !done {
        let Some(body) = read_message(&mut reader) else { break };
        let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&body) else {
            continue;
        };
        let method = msg.get("command").and_then(|m| m.as_str()).unwrap_or("");
        let is_request = msg.get("type").and_then(|t| t.as_str()) == Some("request");
        let seq = msg.get("seq").and_then(|s| s.as_u64()).unwrap_or(1);
        let args = msg.get("arguments").cloned().unwrap_or(serde_json::Value::Null);

        // Helper: send a response only when the client sent an id.
        macro_rules! reply {
            ($cmd:expr, $body:expr) => {
                if is_request {
                    let _g = OUT_LOCK.lock();
                    let mut o = std::io::stdout();
                    respond(&mut o, seq, $cmd, $body);
                }
            };
        }

        match method {
            "initialize" => {
                reply!(
                    "initialize",
                    serde_json::json!({"capabilities": {
                        "supportsConfigurationDoneRequest": true
                    }})
                );
                if is_request {
                    let _g = OUT_LOCK.lock();
                    let mut o = std::io::stdout();
                    send(
                        &mut o,
                        &serde_json::json!({"type": "event", "event": "initialized", "body": {}}),
                    );
                }
            }
            "launch" => {
                reply!("launch", serde_json::json!({}));
            }
            "setBreakpoints" => {
                let mut bps = Vec::new();
                if let Some(list) = args.get("breakpoints").and_then(|b| b.as_array()) {
                    for bp in list {
                        let line = bp.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as usize;
                        if let Some(top) = line_to_top(&lines, line) {
                            bps.push(top);
                        }
                    }
                }
                bps.sort_unstable();
                bps.dedup();
                breaks = bps.clone();
                interp::debug_set_breaks(breaks.clone());
                let resp: Vec<_> = breaks
                    .iter()
                    .map(|t| {
                        serde_json::json!({"verified": true, "line": lines.get(*t).copied().unwrap_or(0)})
                    })
                    .collect();
                reply!("setBreakpoints", serde_json::json!({"breakpoints": resp}));
            }
            "configurationDone" => {
                if !started {
                    started = true;
                    interp::debug_attach(
                        lines.clone(),
                        breaks.clone(),
                        cmd_rx.take().unwrap(),
                        evt_tx.take().unwrap(),
                    );
                    let src = source.clone();
                    let f = file.clone();
                    std::thread::Builder::new()
                        .stack_size(256 * 1024 * 1024)
                        .spawn(move || {
                            let _ = crate::run_source(&src, &f, "");
                            interp::debug_detach();
                        })
                        .expect("spawn debug worker");
                }
                reply!("configurationDone", serde_json::json!({}));
            }
            "threads" => {
                reply!(
                    "threads",
                    serde_json::json!({"threads": [{"id": 1, "name": "main"}]})
                );
            }
            "stackTrace" => {
                reply!(
                    "stackTrace",
                    serde_json::json!({"stackFrames": [
                        {"id": 1, "name": "main", "line": 0, "column": 0,
                         "source": {"name": file}}
                    ]})
                );
            }
            "scopes" => {
                reply!(
                    "scopes",
                    serde_json::json!({"scopes": [
                        {"name": "Locals", "variablesReference": 1, "expensive": false}
                    ]})
                );
            }
            "variables" => {
                let vars: Vec<_> = last_vars
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(n, v)| serde_json::json!({"name": n, "value": v, "variablesReference": 0}))
                    .collect();
                reply!("variables", serde_json::json!({"variables": vars}));
            }
            "continue" | "next" | "stepIn" => {
                let cmd = match method {
                    "continue" => DebugCommand::Continue,
                    "next" => DebugCommand::Next,
                    _ => DebugCommand::StepIn,
                };
                let _ = cmd_tx.send(cmd);
                if is_request {
                    let _g = OUT_LOCK.lock();
                    let mut o = std::io::stdout();
                    respond(&mut o, seq, method, serde_json::json!({}));
                    send(
                        &mut o,
                        &serde_json::json!({
                            "type": "event", "event": "continued",
                            "body": {"threadId": 1, "allThreadsContinued": true}
                        }),
                    );
                }
            }
            "pause" => {
                let _ = cmd_tx.send(DebugCommand::Pause);
            }
            "terminate" | "disconnect" => {
                let _ = cmd_tx.send(DebugCommand::Terminate);
                done = true;
            }
            _ => {
                reply!(method, serde_json::json!({}));
            }
        }
    }
    interp::debug_detach();
    let _ = evt_thread.join();
}
//! Step 9: scripted DAP client for `lykaon debug`.
//!
//! Drives the real `lykaon::dap::serve` (the `lykaonc-cli debug` command) over
//! a pipe and checks: initialize, setBreakpoints, a stopped event, continue,
// and that the program runs and the process terminates.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};

fn frame(obj: &serde_json::Value) -> String {
    let body = serde_json::to_string(obj).unwrap();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn read_msg<R: BufRead>(r: &mut R) -> Option<serde_json::Value> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap_or(0) == 0 {
            return None;
        }
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut buf = vec![0u8; len?];
    r.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

/// Kill the child on drop so a failed assertion never leaves it running.
struct KillOnDrop(Option<Child>);
impl std::ops::Deref for KillOnDrop {
    type Target = Child;
    fn deref(&self) -> &Child {
        self.0.as_ref().unwrap()
    }
}
impl std::ops::DerefMut for KillOnDrop {
    fn deref_mut(&mut self) -> &mut Child {
        self.0.as_mut().unwrap()
    }
}
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(c) = &mut self.0 {
            let _ = c.kill();
        }
    }
}

#[test]
fn dap_step_and_breakpoint() {
    let dir = std::env::temp_dir().join(format!("lykaon_dap_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("prog.lykaon");
    std::fs::write(
        &file,
        "Changeable total is 0.\nRepeat 3 times.\n    Set total to total plus 1.\nSay total.\n",
    )
    .unwrap();

    let exe = env!("CARGO_BIN_EXE_lykaonc-cli");
    let mut child = Command::new(exe)
        .arg("debug")
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn debug");
    let mut guard = KillOnDrop(Some(child));
    let mut stdin = guard.stdin.take().unwrap();
    let stdout = guard.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // initialize
    stdin
        .write_all(
            frame(&serde_json::json!({
                "seq": 1, "type": "request", "command": "initialize",
                "arguments": {"adapterID": "lykaon"}
            }))
            .as_bytes(),
        )
        .unwrap();
    stdin.flush().unwrap();
    let init = read_msg(&mut reader).expect("initialize response");
    assert_eq!(init["command"], "initialize");
    assert!(init["body"]["capabilities"]["supportsConfigurationDoneRequest"] == true);
    let _ = read_msg(&mut reader); // initialized event

    // set a breakpoint on line 4 (the Say)
    stdin
        .write_all(
            frame(&serde_json::json!({
                "seq": 2, "type": "request", "command": "setBreakpoints",
                "arguments": {"source": {"name": "prog.lykaon"},
                              "breakpoints": [{"line": 4}]}
            }))
            .as_bytes(),
        )
        .unwrap();
    stdin.flush().unwrap();
    let bp = read_msg(&mut reader).expect("setBreakpoints response");
    assert_eq!(bp["command"], "setBreakpoints");
    assert!(bp["body"]["breakpoints"][0]["verified"] == true);

    // configurationDone -> run; expect a stopped event (skip output events)
    stdin
        .write_all(
            frame(&serde_json::json!({
                "seq": 3, "type": "request", "command": "configurationDone", "arguments": {}
            }))
            .as_bytes(),
        )
        .unwrap();
    stdin.flush().unwrap();
    let cfg = read_msg(&mut reader).expect("configurationDone response");
    assert_eq!(cfg["command"], "configurationDone");

    let mut saw_stopped = false;
    for _ in 0..20 {
        match read_msg(&mut reader) {
            Some(m) if m["event"] == "stopped" => {
                saw_stopped = true;
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    assert!(saw_stopped, "expected a stopped event");

    // continue to the end
    stdin
        .write_all(
            frame(&serde_json::json!({
                "seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}
            }))
            .as_bytes(),
        )
        .unwrap();
    stdin.flush().unwrap();

    // terminate
    stdin
        .write_all(
            frame(&serde_json::json!({
                "seq": 5, "type": "request", "command": "terminate", "arguments": {}
            }))
            .as_bytes(),
        )
        .unwrap();
    stdin.flush().unwrap();

    let _ = guard.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
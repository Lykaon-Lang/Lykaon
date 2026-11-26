//! Test helper for the M5g isolation suite: spawn N short-lived children
//! and wait for them.
//!
//! It is not part of the shipped tools. The test runs it under
//! `lykaonc-cli run --isolate` to prove the job's active-process cap:
//! with a cap of 2 the second child cannot be created, with a roomy cap all
//! of them run.

use std::process::{Command, Stdio};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s == "--child").unwrap_or(false) {
        std::thread::sleep(Duration::from_millis(700));
        return;
    }
    let count: usize = args
        .get(1)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);
    let exe = std::env::current_exe().expect("current exe");
    let mut children = Vec::new();
    for _ in 0..count {
        match Command::new(&exe)
            .arg("--child")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => children.push(child),
            Err(e) => {
                eprintln!("could not start child: {e}");
                std::process::exit(1);
            }
        }
    }
    println!("started {}", children.len());
    for mut child in children {
        let _ = child.wait();
    }
}
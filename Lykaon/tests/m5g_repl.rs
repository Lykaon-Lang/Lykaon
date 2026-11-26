//! M5g/REPL: pasting a multi-line program into the REPL runs it as one unit.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_lykaon-cli");

fn repl(input: &str) -> (i32, String) {
    let mut child = Command::new(BIN)
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn repl");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write input");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn repl_keeps_state_across_pasted_lines() {
    let (code, out) = repl("Price is 20.\nTax is 5.\nSay price plus tax.\n");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("25"), "{out}");
}

#[test]
fn repl_runs_a_pasted_function_and_loop() {
    let src = concat!(
        "To classify n as whole number.\n",
        "    Changeable r15 is remainder of n by 15.\n",
        "    If r15 is 0.\n",
        "        Give back FizzBuzz as text.\n",
        "    Changeable r3 is remainder of n by 3.\n",
        "    If r3 is 0.\n",
        "        Give back Fizz as text.\n",
        "    Changeable r5 is remainder of n by 5.\n",
        "    If r5 is 0.\n",
        "        Give back Buzz as text.\n",
        "    Give back n as text.\n",
        "\n",
        "Changeable n is 1.\n",
        "Repeat 15 times.\n",
        "    Say classify n.\n",
        "    Set n to n plus 1.\n",
    );
    let (code, out) = repl(src);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("FizzBuzz"), "{out}");
    assert!(out.contains("13\n14\nFizzBuzz"), "{out}");
}
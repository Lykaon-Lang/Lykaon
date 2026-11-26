//! M5 (Appendix L 7.6) conformance: encryption, keys and the time limit.
//!
//! Run: cargo test --test m5_crypto -- --nocapture

use lykaon::run_source;

fn run(src: &str) -> (String, i32) {
    run_source(src, "program.lykaon", "")
}

#[test]
fn m5_t30_random_key_round_trip() {
    let src = "Key is random key.\nEnc is encrypted hello with key.\nSay decrypted enc with key.";
    let (out, code) = run(src);
    assert_eq!(code, 0, "the round trip failed: {out}");
    assert_eq!(out.trim(), "hello");
}

#[test]
fn m5_t30_password_round_trip() {
    let src = "Sesame is sesame.\nPwd is key from Sesame.\nEnc is encrypted hello with Pwd.\nSay decrypted enc with Pwd.";
    let (out, code) = run(src);
    assert_eq!(code, 0, "the password round trip failed: {out}");
    assert_eq!(out.trim(), "hello");
}

#[test]
fn m5_t30_wrong_key_is_bad_format() {
    let src = "Key is random key.\nEnc is encrypted hello with key.\nOther is random key.\nSay decrypted enc with other.";
    let (out, code) = run(src);
    assert_eq!(code, 1, "a wrong key must fail: {out}");
    // a bad-format failure is anonymous in this cut (logged)
    assert!(
        out.contains("can fail here") || out.contains("bad format"),
        "expected a bad-format failure, got: {out}"
    );
}

#[test]
fn m5_t21_limit_time_is_t0612() {
    let src = "Limit time 1 seconds.\nSleep for 2 seconds.\nSay done.";
    let (out, code) = run(src);
    assert_eq!(code, 1, "the time limit must stop the run: {out}");
    assert!(out.contains("T0612"), "expected T0612, got: {out}");
    assert!(out.contains("Time limit reached."), "got: {out}");
}

#[test]
fn m5_limit_memory_parses() {
    // memory enforcement is a later step; the sentence must parse and the
    // program must run normally with the limit recorded.
    let (out, code) = run("Limit memory 1 megabytes.\nSay ok.");
    assert_eq!(code, 0, "Limit memory must parse: {out}");
    assert_eq!(out.trim(), "ok");
}
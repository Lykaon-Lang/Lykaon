//! M1a conformance test runner.
//! Walks tests/conformance/m1a and checks:
//!  - valid/*.lykaon parse cleanly (exit 0, empty output)
//!  - errors/*/input.lykaon produce the exact expected.txt
//!  - fmt/*.lykaon format to expected.lykaon

use lykaon::{check_bytes, check_source, try_format_source};
use std::path::{Path, PathBuf};

const ROOT: &str = "tests/conformance/m1a";

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn run() {
    let root = Path::new(ROOT);
    let mut failures: Vec<String> = Vec::new();

    // ---- valid ----
    let valid = root.join("valid");
    if valid.is_dir() {
        for entry in std::fs::read_dir(&valid).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "lykaon") {
                let source = read(&p);
                let (out, ok) = check_source(&source, p.to_str().unwrap());
                if !ok || !out.is_empty() {
                    failures.push(format!(
                        "VALID {}: expected clean parse, ok={} output={:?}",
                        p.display(),
                        ok,
                        out
                    ));
                }
            }
        }
    }

    // ---- errors ----
    let errors = root.join("errors");
    if errors.is_dir() {
        for entry in std::fs::read_dir(&errors).unwrap() {
            let entry = entry.unwrap();
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let input = dir.join("input.lykaon");
            let expected = dir.join("expected.txt");
            if !input.is_file() || !expected.is_file() {
                continue;
            }
            let source = read(&input);
            let want = read(&expected);
            // Normalize path separators for cross-platform comparison.
            let want = want.replace('\\', "/");
            let (out, ok) = check_source(&source, input.to_str().unwrap());
            let out = out.replace('\\', "/");
            if ok {
                failures.push(format!(
                    "ERROR {}: expected rejection, but parsed cleanly",
                    dir.display()
                ));
            } else if out != want {
                failures.push(format!(
                    "ERROR {}: output mismatch.\n--- expected ---\n{:?}\n--- actual ---\n{:?}\n--------------",
                    dir.display(),
                    want,
                    out
                ));
            }
        }
    }

    // ---- warn: warnings print with ok=true (the exact output) ----
    let warn = root.join("warn");
    if warn.is_dir() {
        for entry in std::fs::read_dir(&warn).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "lykaon") {
                let expected = p.with_extension("expected.txt");
                if !expected.is_file() {
                    continue;
                }
                let source = read(&p);
                let want = read(&expected);
                let want = want.replace('\\', "/");
                let (out, ok) = check_source(&source, p.to_str().unwrap());
                let out = out.replace('\\', "/");
                if !ok {
                    failures.push(format!(
                        "WARN {}: expected a warning, but the program was rejected:\n{out}",
                        p.display()
                    ));
                } else if out != want {
                    failures.push(format!(
                        "WARN {}: output mismatch.\n--- expected ---\n{:?}\n--- actual ---\n{:?}\n--------------",
                        p.display(),
                        want,
                        out
                    ));
                }
            }
        }
    }

    // ---- fmt ----
    let fmt = root.join("fmt");
    if fmt.is_dir() {
        for entry in std::fs::read_dir(&fmt).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "lykaon") {
                let stem = p.file_stem().unwrap().to_str().unwrap();
                let expected = p.with_extension("expected.lykaon");
                if !expected.is_file() {
                    continue;
                }
                let source = read(&p);
                let want = read(&expected);
                let got = match try_format_source(&source, p.to_str().unwrap()) {
                    Ok(g) => g,
                    Err(diags) => {
                        let text = diags
                            .iter()
                            .map(|d| d.to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        failures.push(format!(
                            "FMT {}: expected formatting to succeed, got errors:\n{text}\n--------------",
                            p.display()
                        ));
                        continue;
                    }
                };
                if got != want {
                    failures.push(format!(
                        "FMT {}: output mismatch.\n--- expected ---\n{:?}\n--- actual ---\n{:?}\n--------------",
                        p.display(),
                        want,
                        got
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            println!("FAIL: {f}");
        }
        panic!("{} M1a conformance test(s) failed", failures.len());
    }
}

#[test]
fn m1a_conformance() {
    run();
}

/// T0018: a file that is not UTF-8 is refused by the byte-level check with
/// the book's wording (the text-level API cannot see such bytes).
#[test]
fn m1a_file_not_utf8_is_t0018() {
    let bytes = b"Say hi.\nSay \xff\xfe goodbye.\n";
    let (out, ok) = check_bytes(bytes, "bad.lykaon");
    assert!(!ok, "expected a rejection, got: {out}");
    assert!(out.contains("T0018"), "expected T0018, got: {out}");
    assert!(
        out.contains("The file holds bytes that are not valid UTF-8."),
        "expected the book's cause, got: {out}"
    );
    assert!(
        out.contains("Save the file as UTF-8."),
        "expected the book's fix, got: {out}"
    );
    // a BOM is not a failure: the same check passes a UTF-8 file with one
    let (out, ok) = check_bytes(b"\xef\xbb\xbfSay hi.\n", "bom.lykaon");
    assert!(ok, "expected the BOM to be ignored, got: {out}");
}

/// G42/G43: the count after Repeat is a NUMBER literal. A name, a number
/// word, a negative or a decimal must be an error, never a silent no-op.
#[test]
fn repeat_count_must_be_a_number_literal() {
    let cases = [
        ("Count is 3.\nRepeat count times.\n    Say hi.\n", "T0113"),
        ("Repeat three times.\n    Say hi.\n", "T0006"),
        ("Repeat minus 3 times.\n    Say hi.\n", "T0122"),
        ("Repeat 2.5 times.\n    Say hi.\n", "T0122"),
        ("Repeat 3.\n    Say hi.\n", "T0113"),
        ("Repeat 3 say hi.\n", "T0113"),
    ];
    for (src, code) in cases {
        let (out, ok) = check_source(src, "repeat.lykaon");
        assert!(!ok, "expected {code} for {src:?}, got a clean parse: {out}");
        assert!(
            out.contains(code),
            "expected {code} for {src:?}, got: {out}"
        );
    }

    // The run path must reject it too, not quietly run zero iterations.
    let (out, code) = lykaon::run_source(
        "Count is 3.\nRepeat count times.\n    Say hi.\n",
        "repeat.lykaon",
        "",
    );
    assert_ne!(code, 0, "expected a non-zero exit, got output: {out}");
    assert!(
        !out.lines().any(|l| l.trim() == "hi"),
        "the body must not run: {out}"
    );

    // Valid forms stay valid.
    for src in [
        "Repeat 0 times.\n    Say hi.\n",
        "Repeat 3 times say hi.\n",
        "Changeable n is 0.\nRepeat while n is less than 2.\n    Set n to n plus 1.\n",
    ] {
        let (out, ok) = check_source(src, "ok.lykaon");
        assert!(ok, "expected a clean parse for {src:?}, got: {out}");
    }
}

/// The formatter must refuse a file whose indentation tree is broken. In
/// particular it must not flatten a 2-space block into root-level sentences.
#[test]
fn fmt_refuses_invalid_block_indentation() {
    let src = "If age is at least 18.\n  Say adult.\nOtherwise.\n  Say minor.\n";
    let diags = try_format_source(src, "bad.lykaon")
        .expect_err("expected the formatter to refuse invalid indentation");
    let text = diags
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("T0002"), "expected T0002, got: {text}");
    assert!(text.contains("T0014"), "expected T0014, got: {text}");
}

/// Chapter 40: capital sentence start, single spaces, at most one blank line,
/// and formatting twice changes nothing.
#[test]
fn fmt_canonicalizes_and_is_idempotent() {
    let src = "say    hello.   \n\n\nSay   world.   \n";
    let once = try_format_source(src, "messy.lykaon").expect("valid messy source");
    assert_eq!(once, "Say hello.\n\nSay world.\n");
    let twice = try_format_source(&once, "messy.lykaon").expect("formatted source");
    assert_eq!(twice, once, "formatting must be idempotent");
}

/// Raw text keeps its capitals and its internal spaces.
#[test]
fn fmt_preserves_raw_text() {
    let src = "Say hello   world.\nSay lines.\n    Line   One\n    line two\n";
    let out = try_format_source(src, "raw.lykaon").expect("valid raw text");
    assert_eq!(
        out,
        "Say hello   world.\nSay lines.\n    Line   One\n    line two\n"
    );
}

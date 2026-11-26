//! M5f: audit and review framework.

use lykaon::audit::{self, Severity};

fn prog(src: &str) -> lykaon_core::sem::Program {
    lykaon::compile_tree(src, "t.lykaon").expect("compiles")
}

#[test]
fn audit_finds_taint_secret_and_permissions() {
    let src = "Ask name.\nPassword is hunter2.\nRun name with name.\nWrite name to file out in temporary folder.\n";
    let p = prog(src);
    let a = audit::audit(&p, src, "t.lykaon");
    let rules: Vec<&str> = a.findings.iter().map(|f| f.rule).collect();
    assert!(rules.contains(&"secret"), "{rules:?}");
    assert!(rules.contains(&"taint-run"), "{rules:?}");
    assert!(rules.contains(&"taint-file"), "{rules:?}");
    assert!(rules.contains(&"perm-files"), "{rules:?}");
    assert!(rules.contains(&"perm-process"), "{rules:?}");
    // the secret is proved (the literal is in the source); the taint is opinion
    assert!(a.proved() >= 1, "secret should be reproduced");
}

#[test]
fn audit_clean_program_has_no_findings() {
    let src = "Say hello.\nTo addup x as whole number with y as whole number.\n    Give back x plus y as whole number.\nSay addup 2 with 3.\n";
    let p = prog(src);
    let a = audit::audit(&p, src, "t.lykaon");
    assert!(a.findings.is_empty(), "{:?}", a.findings);
}

#[test]
fn sarif_is_valid_json() {
    let src = "Password is hunter2.\n";
    let p = prog(src);
    let a = audit::audit(&p, src, "t.lykaon");
    let s = a.to_sarif();
    let v: serde_json::Value = serde_json::from_str(&s).expect("valid SARIF JSON");
    assert_eq!(v["version"], "2.1.0");
    assert!(v["runs"][0]["results"].as_array().unwrap().len() >= 1);
}

#[test]
fn review_diff_finds_added_function() {
    let old = prog("To addup x as whole number with y as whole number.\n    Give back x plus y as whole number.\n");
    let new = prog("To addup x as whole number with y as whole number.\n    Give back x plus y as whole number.\nTo multiplier x as whole number with y as whole number.\n    Give back x times y as whole number.\n");
    let f = audit::review_diff(&old, &new, "t.lykaon");
    assert!(f.iter().any(|x| x.rule == "func-added"), "{f:?}");
}

#[test]
fn severity_words() {
    assert_eq!(Severity::Error.word(), "Error");
    assert_eq!(Severity::Warning.word(), "Warning");
    assert_eq!(Severity::Note.word(), "Note");
}

#[test]
fn mutation_catches_operator_flip() {
    let src = "To addup x as whole number with y as whole number.\n    Give back x plus y as whole number.\nTest addup.\n    Check that addup 2 with 3 is 5.\n";
    let m = lykaon::mutation::run(src, "t.lykaon");
    assert!(m.has_tests, "the program has a Test block");
    assert!(m.caught >= 1, "at least one mutant should be caught");
}


#[test]
fn intent_check_reports_mismatch() {
    let src = "About this prints hello.\nSay bye.\n";
    let mut f = Vec::new();
    lykaon::audit::check_intent(src, "t.lykaon", &mut f);
    assert!(f.iter().any(|x| x.rule == "intent"), "{f:?}");
}

#[test]
fn dependency_checksum_mismatch() {
    let d = std::env::temp_dir().join(format!("lykaon_deptest_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("dep")).unwrap();
    std::fs::write(d.join("dep").join("a.txt"), "hello").unwrap();
    std::fs::write(
        d.join("lykaon.lock"),
        "Name is app.\nLocked dep from dep at 1 point 0 point 0 with checksum deadbeef.\n",
    )
    .unwrap();
    let f = lykaon::audit::check_dependencies(&d, "t.lykaon");
    assert!(f.iter().any(|x| x.rule == "dep-checksum"), "{f:?}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn behavior_diff_reports_change() {
    let old_src = "To bump x as whole number.\n    Give back x plus 1 as whole number.\nSay bump 1.\n";
    let new_src = "To bump x as whole number.\n    Give back x plus 2 as whole number.\nSay bump 1.\n";
    let op = prog(old_src);
    let np = prog(new_src);
    let f = lykaon::audit::diff_behavior(old_src, new_src, &op, &np, "t.lykaon");
    assert!(f.iter().any(|x| x.rule == "behavior-change"), "{f:?}");
}

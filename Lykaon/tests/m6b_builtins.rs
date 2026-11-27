//! M6b standard-library built-ins (the 9 audit gaps).

use lykaon::run_source;
use lykaon_core::sem::Value;
use std::path::PathBuf;

fn stage(name: &str, src: &str) -> (PathBuf, String, i32) {
    let dir = std::env::temp_dir().join(format!("lykaon_bi_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source(src, file.to_str().unwrap(), "");
    (dir, out, code)
}

fn call(name: &str, args: &[Value]) -> Result<Value, String> {
    let b = lykaon_core::builtins::find(name).expect("builtin exists");
    (b.f)(args)
}

// ---- 1. math ----

#[test]
fn math_functions() {
    let (d, out, code) = stage(
        "math",
        "Say sqrt 9.\nSay floor 3.7.\nSay ceiling 3.2.\nSay logarithm 1.\nSay power 2 and 10.\n",
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("3"), "{out}");
    assert!(out.contains("4"), "{out}");
    assert!(out.contains("1024"), "{out}");
    let _ = std::fs::remove_dir_all(d);
}

// ---- 2. hashlib ----

#[test]
fn hash_functions() {
    let (d, out, code) = stage("hash", "Say sha256 hi.\nSay md5 hi.\n");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4"), "{out}");
    assert!(out.contains("49f68a5c8493ec2c0bf489821c21fc3b"), "{out}");
    let _ = std::fs::remove_dir_all(d);
}

// ---- 3. base64 ----

#[test]
fn base64_functions() {
    let (d, out, code) = stage("b64", "Say b64encode hi.\nSay b64decode aGk.\n");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("aGk="), "{out}");
    assert!(out.lines().any(|l| l.trim() == "hi"), "{out}");
    let _ = std::fs::remove_dir_all(d);
}

// ---- 4. struct ----

#[test]
fn struct_pack_unpack() {
    let packed = call("packwhole", &[Value::Whole(258)]).unwrap();
    let back = call("unpackwhole", &[packed]).unwrap();
    assert_eq!(back, Value::Whole(258));
    let pd = call("packdecimal", &[Value::Dec(3.5)]).unwrap();
    let bd = call("unpackdecimal", &[pd]).unwrap();
    assert_eq!(bd, Value::Dec(3.5));
}

// ---- 5. subprocess ----

#[test]
fn subprocess_capture() {
    // Call the built-in directly with the command text: a source-level
    // multi-word text argument is a separate parser question (the call
    // grammar takes one operand), and this test pins the built-in itself.
    let out = call("capture", &[Value::Text("echo hi".into())]).unwrap();
    let Value::Text(text) = out else {
        panic!("capture must give text");
    };
    assert!(text.contains("hi"), "capture produced no output: {text:?}");
}

// ---- 6. collections ----

#[test]
fn collections_default_map() {
    let m = Value::Map(vec![(Value::Text("a".into()), Value::Whole(1))]);
    let hit = call(
        "mapdefault",
        &[m.clone(), Value::Text("a".into()), Value::Whole(9)],
    )
    .unwrap();
    assert_eq!(hit, Value::Whole(1));
    let miss = call(
        "mapdefault",
        &[m, Value::Text("z".into()), Value::Whole(9)],
    )
    .unwrap();
    assert_eq!(miss, Value::Whole(9));
}

// ---- 7. re ----

#[test]
fn regex_replace_and_split() {
    let r = call(
        "regexreplace",
        &[
            Value::Text("hello".into()),
            Value::Text("l".into()),
            Value::Text("L".into()),
        ],
    )
    .unwrap();
    assert_eq!(r, Value::Text("heLLo".into()));
    let s = call(
        "regexsplit",
        &[
            Value::Text("a1b2c".into()),
            Value::Text("\\d".into()),
        ],
    )
    .unwrap();
    assert_eq!(
        s,
        Value::List(vec![
            Value::Text("a".into()),
            Value::Text("b".into()),
            Value::Text("c".into()),
        ])
    );
}

// ---- 8. socket ----

#[test]
fn socket_close_unknown_is_zero() {
    let n = call("tcpclose", &[Value::Conn(9999)]).unwrap();
    assert_eq!(n, Value::Whole(0));
}

// ---- 9. statistics ----

#[test]
fn statistics_mode_and_stdev() {
    let (d, out, code) = stage(
        "stats",
        "Say mode list of 1 and 2 and 2 and 3.\nSay stdev list of 2 and 4 and 4 and 4 and 5 and 5 and 7 and 9.\n",
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.lines().any(|l| l.trim() == "2"), "{out}");
    assert!(out.contains("2.00"), "{out}");
    let _ = std::fs::remove_dir_all(d);
}
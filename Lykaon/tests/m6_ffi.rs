//! M6 real C FFI calls through `.stub.shelf`.

use lykaon::run_source;
use std::path::PathBuf;

fn stage(name: &str, lib: &str, stub: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lykaon_ffi_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{lib}.stub.shelf")), stub).unwrap();
    dir
}

#[cfg(windows)]
#[test]
fn call_cos_and_strlen_from_msvcrt() {
    let dir = stage(
        "win",
        "msvcrt",
        "C safe function cos as cosine takes decimal gives decimal.\n\
         C function strlen as length takes text gives whole number.\n",
    );
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source(
        "Use C library msvcrt.\nSay cosine 0.\nSay length hello.\n",
        file.to_str().unwrap(),
        "",
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("1.00"), "{out}");
    assert!(out.lines().any(|l| l.trim() == "5"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn call_cos_from_libm() {
    let dir = stage(
        "nix",
        "m",
        "C safe function cos as cosine takes decimal gives decimal.\n",
    );
    let file = dir.join("prog.lykaon");
    let (out, code) = run_source(
        "Use C library m.\nSay cosine 0.\n",
        file.to_str().unwrap(),
        "",
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("1.00"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn record_return_from_c_struct() {
    let dir = std::env::temp_dir().join(format!("lykaon_ffi_record_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("point.rs"),
        "#[repr(C)]\npub struct Point { pub x: i64, pub y: i64 }\n\
         #[no_mangle]\npub extern \"C\" fn make_point(n: i64) -> *mut Point {\n\
         Box::into_raw(Box::new(Point { x: n, y: n + 1 }))\n}\n",
    )
    .unwrap();
    let out_lib = if cfg!(windows) {
        dir.join("point.dll")
    } else {
        dir.join("libpoint.so")
    };
    let status = std::process::Command::new("rustc")
        .arg("--crate-type")
        .arg("cdylib")
        .arg("--edition")
        .arg("2021")
        .arg(dir.join("point.rs"))
        .arg("-o")
        .arg(&out_lib)
        .status()
        .expect("rustc");
    assert!(status.success(), "rustc failed");
    std::fs::write(
        dir.join("point.stub.shelf"),
        "C function make_point as makePoint takes whole number gives point.\n",
    )
    .unwrap();
    let src = "A point has x as whole number and y as whole number as C does.\n\
               Use C library point.\n\
               P is makePoint 7.\n\
               Say x of P.\n\
               Say y of P.\n";
    let file = dir.join("prog.lykaon");
    std::fs::write(&file, src).unwrap();
    let (out, code) = run_source(src, file.to_str().unwrap(), "");
    assert_eq!(code, 0, "{out}");
    assert!(out.lines().any(|l| l.trim() == "7"), "{out}");
    assert!(out.lines().any(|l| l.trim() == "8"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}

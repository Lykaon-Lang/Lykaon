//! M6 conformance: C bridge, Python extension, JS/WASM build target.

use lykaon::cbridge::{generate_header, layout_record, parse_stub, CKind, Layout};
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lykaon_m6_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn c_layout_and_header() {
    let fields = vec![
        ("x".to_string(), CKind::Whole),
        ("y".to_string(), CKind::Whole),
    ];
    let (offsets, size, align) = layout_record(&fields, &Layout::default()).unwrap();
    assert_eq!(offsets[1].offset, 8);
    assert_eq!(size, 16);
    assert_eq!(align, 8);
    let h = generate_header(&[("make_point".to_string(), vec![CKind::Whole], CKind::Record("point".to_string()), false)]);
    assert!(h.contains("void * make_point(int64_t a0);"), "{h}");
}

#[test]
fn stub_parse() {
    let d = parse_stub("C safe function cos as cosine takes decimal gives decimal.\n").unwrap();
    assert_eq!(d[0].name, "cosine");
    assert_eq!(d[0].inputs, vec![CKind::Decimal]);
}

#[test]
fn js_build_target() {
    let cli = env!("CARGO_BIN_EXE_lykaonc-cli");
    let d = temp("js");
    let src = "Say hi.\n";
    std::fs::write(d.join("prog.lykaon"), src).unwrap();
    let out = d.join("out");
    let status = std::process::Command::new(cli)
        .args(["build", "for", "js"])
        .arg(d.join("prog.lykaon"))
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(out.join("prog.wasm").is_file());
    assert!(out.join("prog.js").is_file());
    assert!(out.join("prog.d.ts").is_file());
    let _ = std::fs::remove_dir_all(d);
}

#[cfg(windows)]
#[test]
fn python_build_target() {
    let cli = env!("CARGO_BIN_EXE_lykaonc-cli");
    let d = temp("py");
    std::fs::write(d.join("prog.lykaon"), "Say hi.\n").unwrap();
    let out = d.join("out");
    let status = std::process::Command::new(cli)
        .args(["build", "for", "python"])
        .arg(d.join("prog.lykaon"))
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(out.join("prog.pyd").is_file(), "prog.pyd missing");
    let _ = std::fs::remove_dir_all(d);
}
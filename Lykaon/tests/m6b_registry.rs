//! M6b local registry tests.

use lykaon::registry::{add, publish, status};
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lykaon_m6b_reg_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn publish_add_status_roundtrip() {
    let root = temp("rt");
    let pkg = root.join("demo");
    std::fs::create_dir_all(pkg.join("src")).unwrap();
    std::fs::write(
        pkg.join("Lykaon.toml"),
        "Name is demo.\nVersion is 1.0.0.\n",
    )
    .unwrap();
    std::fs::write(pkg.join("src").join("main.lykaon"), "Say hi.\n").unwrap();
    let reg = root.join("registry");
    let (name, version) = publish(&pkg, &reg).unwrap();
    assert_eq!(name, "demo");
    assert_eq!(version, "1.0.0");
    assert!(reg.join("demo").join("1.0.0.ship").is_file());
    assert!(status(&reg).contains("\"demo\""));

    let dest = root.join("out");
    std::fs::create_dir_all(&dest).unwrap();
    let got = add("demo", &reg, &dest).unwrap();
    assert_eq!(got, dest.join("demo").join("1.0.0"));
    assert_eq!(
        std::fs::read_to_string(got.join("src").join("main.lykaon")).unwrap(),
        "Say hi.\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn add_missing_package_fails() {
    let root = temp("miss");
    let reg = root.join("registry");
    std::fs::create_dir_all(&reg).unwrap();
    let err = add("nope", &reg, &root).unwrap_err();
    assert!(err.contains("not in the registry"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn checksum_mismatch_is_reported() {
    let root = temp("bad");
    let pkg = root.join("demo");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("Lykaon.toml"), "Name is demo.\nVersion is 1.0.0.\n").unwrap();
    let reg = root.join("registry");
    publish(&pkg, &reg).unwrap();
    // Corrupt the index checksum so verification must fail.
    let index_path = reg.join("index.json");
    let mut index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).unwrap()).unwrap();
    index["packages"]["demo"]["versions"]["1.0.0"] = serde_json::json!("0".repeat(64));
    std::fs::write(&index_path, serde_json::to_vec_pretty(&index).unwrap()).unwrap();
    let err = add("demo", &reg, &root).unwrap_err();
    assert!(err.contains("checksum mismatch"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}
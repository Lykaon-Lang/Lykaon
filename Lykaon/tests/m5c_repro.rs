//! M5c item 13: reproducible builds — the same source must give the same bytes.
//!
//! Two levels are checked:
//!   - object bytes from the compiler (always; no toolchain needed),
//!   - linked executable bytes (when the runtime library and linker are found).

const SRC: &str = "Say reproducible.
To totalup x as whole number with y as whole number.
    Give back x plus y as whole number.
Say totalup 2 with 3.
";

#[test]
fn object_bytes_are_deterministic() {
    let a = lykaon::native_object(SRC, "repro.lykaon", "fast", false)
        .ok()
        .expect("build a")
        .0;
    let b = lykaon::native_object(SRC, "repro.lykaon", "fast", false)
        .ok()
        .expect("build b")
        .0;
    assert_eq!(a, b, "the same source must compile to the same object bytes");
}

#[test]
fn linked_exe_is_reproducible() {
    let rt = match lykaon::linker::find_rt_lib() {
        Ok(rt) => rt,
        Err(_) => {
            eprintln!("skipping: no runtime library / linker on this machine");
            return;
        }
    };
    let a = lykaon::native_object(SRC, "repro.lykaon", "fast", false)
        .ok()
        .expect("build a")
        .0;
    let b = lykaon::native_object(SRC, "repro.lykaon", "fast", false)
        .ok()
        .expect("build b")
        .0;
    let dir = std::env::temp_dir().join(format!("lykaon_repro_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let oa = dir.join("a.obj");
    let ob = dir.join("b.obj");
    std::fs::write(&oa, &a).unwrap();
    std::fs::write(&ob, &b).unwrap();
    let ea = dir.join("a.exe");
    let eb = dir.join("b.exe");
    lykaon::linker::link_exe(&oa, &rt, &ea).expect("link a");
    lykaon::linker::link_exe(&ob, &rt, &eb).expect("link b");
    let ba = std::fs::read(&ea).unwrap();
    let bb = std::fs::read(&eb).unwrap();
    assert_eq!(
        ba,
        bb,
        "the same source must link to the same executable bytes"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
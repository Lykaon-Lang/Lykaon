//! M5 (Appendix L 7.6, M5-S43..S48): the native permission sandbox.

use lykaon_core::sandbox::Sandbox;

#[test]
fn m5_t27_files_are_limited_to_the_allowed_folders() {
    let dir = std::env::temp_dir().join(format!("lykaon_sandbox_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(dir.join("settings.lykaon"), "Allows files in folder data.\n").unwrap();
    std::fs::write(dir.join("data").join("inside.txt"), "inside").unwrap();
    std::fs::write(dir.join("outside.txt"), "outside").unwrap();
    let mut s = Sandbox::from_settings(&dir);
    s.activate();
    assert!(s.check_file(&dir.join("data").join("inside.txt")).is_ok());
    assert!(s.check_file(&dir.join("outside.txt")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m5_t27_processes_are_refused_unless_allowed() {
    let dir = std::env::temp_dir().join(format!("lykaon_sandbox_proc_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.lykaon"), "Allows files in folder data.\n").unwrap();
    let mut s = Sandbox::from_settings(&dir);
    s.activate();
    assert!(s.check_process().is_err());
    std::fs::write(
        dir.join("settings.lykaon"),
        "Allows files in folder data and processes.\n",
    )
    .unwrap();
    let mut s = Sandbox::from_settings(&dir);
    s.activate();
    assert!(s.check_process().is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m5_t27_hosts_and_environment_are_limited() {
    let dir = std::env::temp_dir().join(format!("lykaon_sandbox_net_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.lykaon"),
        "Allows network to host example.com and environment variable LYKAON_TEST.\n",
    )
    .unwrap();
    let mut s = Sandbox::from_settings(&dir);
    s.activate();
    assert!(s.check_host("example.com").is_ok());
    assert!(s.check_host("api.example.com").is_ok());
    assert!(s.check_host("evil.com").is_err());
    assert!(s.check_env("LYKAON_TEST").is_ok());
    assert!(s.check_env("SECRET").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn m5_usage_records_the_permissions_a_run_touched() {
    let dir = std::env::temp_dir().join(format!("lykaon_sandbox_usage_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let mut s = lykaon_core::sandbox::Sandbox::from_settings(&dir);
    s.activate();
    let _ = s.check_file(&dir.join("data.csv"));
    let _ = s.check_host("example.com");
    let _ = s.check_env("HOME");
    let _ = s.check_process();
    let used = s.usage();
    assert!(used.iter().any(|u| u.starts_with("file ")), "{used:?}");
    assert!(used.iter().any(|u| u == "host example.com"), "{used:?}");
    assert!(used.iter().any(|u| u == "env HOME"), "{used:?}");
    assert!(used.iter().any(|u| u == "processes 1"), "{used:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

//! M5g/IDE: register the `.lykaon` file type with Windows Explorer.
//!
//! On Windows, Explorer shows a file-type icon from the registry
//! (HKCU\Software\Classes), the same way the Python and .NET installers do.
//! This writes those keys for the current user (no admin needed).
//!
//! On other platforms this is a no-op (Explorer uses its own conventions).

pub fn register(icon: &str, opener: &str) -> Result<String, String> {
    if !cfg!(windows) {
        return Ok("file-type registration is Windows-only; nothing to do".to_string());
    }
    // A .reg file (imported by reg.exe) preserves quotes and backslashes,
    // which reg.exe's /d argument strips. Paths with spaces need the quotes.
    fn escape(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
    let icon_value = format!("\"{icon}\",0");
    let command_value = format!("\"{opener}\" run \"%1\"");
    let body = format!(
        "Windows Registry Editor Version 5.00\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\Classes\\.lykaon]\r\n\
@=\"Lykaon.File\"\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\Classes\\Lykaon.File]\r\n\
@=\"Lykaon Source File\"\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\Classes\\Lykaon.File\\DefaultIcon]\r\n\
@=\"{}\"\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\Classes\\Lykaon.File\\shell\\open\\command]\r\n\
@=\"{}\"\r\n",
        escape(&icon_value),
        escape(&command_value)
    );
    // reg.exe expects a UTF-16LE .reg file (with BOM) for non-ASCII paths.
    let mut bytes = vec![0xFF, 0xFE];
    for unit in body.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let path = std::env::temp_dir().join(format!("lykaon-register-{}.reg", std::process::id()));
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let out = std::process::Command::new("reg")
        .args(["import", &path.to_string_lossy()])
        .output()
        .map_err(|e| format!("cannot run reg: {e}"))?;
    let _ = std::fs::remove_file(&path);
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    // Ask Explorer to refresh its icon cache.
    let _ = std::process::Command::new("ie4uinit.exe").arg("-show").output();
    Ok(format!(
        "registered .lykaon (icon: {icon}, open with: {opener} run \"%1\")"
    ))
}

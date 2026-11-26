//! M5f item 10: optional agent mode.
//!
//! `lykaon audit --agent` sends the audit findings plus the source to the
//! user's own model (API key from OPENROUTER_API_KEY) and prints its
//! suggestions. It is optional: nothing depends on it, and it never edits
//! files. The model's proposals are shown as-is; the evidence rule still
//! applies to what `lykaon audit` itself reports.
//!
//! The HTTP call uses `curl` (present on Windows 10+ and most CI images) to
// avoid adding a TLS/HTTP dependency to the compiler.

use std::process::Command;

pub fn review(file: &str, source: &str, findings_json: &str, model: &str) -> Result<String, String> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| "set OPENROUTER_API_KEY to use --agent".to_string())?;
    let prompt = format!(
        "You are reviewing a Lykaon program for bugs and security issues.\n\
File: {file}\n\nSource:\n{source}\n\nStatic audit findings (JSON):\n{findings_json}\n\n\
Suggest concrete fixes. Be brief and specific. Do not invent issues that are not shown.",
        file = file,
        source = source,
        findings_json = findings_json
    );
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0
    })
    .to_string();
    let out = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "https://openrouter.ai/api/v1/chat/completions",
            "-H",
            &format!("Authorization: Bearer {api_key}"),
            "-H",
            "Content-Type: application/json",
            "-d",
            &body,
        ])
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("curl failed: {}", out.status));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    Ok(v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no content)")
        .to_string())
}
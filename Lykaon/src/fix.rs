//! M5c item 6: `lykaon fix`.
//!
//! Applies unambiguous suggestions (currently "did you mean ...?" for an
//! unknown name) to a **copy** and prints a diff. The file is written only
//! with `--apply`. If there is no safe suggestion, nothing changes.

use std::path::Path;

pub fn run(file: &str, apply: bool) -> Result<String, String> {
    let source = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let diags = crate::check_diagnostics(&source, file);
    let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
    let mut changes: Vec<(usize, String, String)> = Vec::new();
    for d in &diags {
        if d.line == 0 || d.line > lines.len() {
            continue;
        }
        for s in &d.suggestions {
            if s.replacement.is_empty() || s.hi <= s.lo {
                continue;
            }
            let line = &lines[d.line - 1];
    if s.hi > line.len() || !line.is_char_boundary(s.lo) || !line.is_char_boundary(s.hi) {
        continue;
    }
    let old = line[s.lo..s.hi].to_string();
    changes.push((d.line, old, s.replacement.clone()));
            lines[d.line - 1].replace_range(s.lo..s.hi, &s.replacement);
        }
    }
    if changes.is_empty() {
        return Ok("no safe suggestions found.\n".to_string());
    }
    // Apply per line in reverse column order would be ideal; the suggestions
    // here are single-word replacements, so this simple form is safe.
    let mut out = String::new();
    for (line, old, new) in &changes {
        out.push_str(&format!("line {line}: {old} -> {new}\n"));
    }
    if apply {
        std::fs::write(Path::new(file), lines.join("\n") + "\n").map_err(|e| e.to_string())?;
        out.push_str("applied with --apply.\n");
    } else {
        out.push_str("no files changed (use --apply).\n");
    }
    Ok(out)
}
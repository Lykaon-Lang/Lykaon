//! M5c item 2: JSON diagnostics for `lykaon check --json`.
//!
//! The text diagnostics stay the source of truth; this is a pure formatter.

use serde_json::json;

fn line_of(block: &str) -> Option<usize> {
    block
        .lines()
        .find_map(|l| l.strip_prefix("Where:"))
        .and_then(|l| l.split(", line ").nth(1))
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.trim().parse().ok())
}

fn section<'a>(block: &'a str, prefix: &str) -> Option<&'a str> {
    block
        .lines()
        .find_map(|l| l.strip_prefix(prefix))
        .map(|s| s.trim())
}

fn code_of(message: &str) -> String {
    let start = message.rfind('(').unwrap_or(0);
    let end = message.rfind(')').unwrap_or(0);
    if start != 0 && end > start {
        message[start + 1..end].trim().to_string()
    } else {
        String::new()
    }
}

/// JSON form of structured diagnostics (M5c).
pub fn to_json(diags: &[crate::diagnostics::Diagnostic]) -> String {
    let items: Vec<_> = diags.iter().map(|d| d.to_json()).collect();
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Diagnostic, Severity};

    #[test]
    fn json_has_code_line_and_fix() {
        let d = Diagnostic::error(
            "T0205",
            "x is not defined.".to_string(),
            "t.lykaon".to_string(),
            3,
            "Set x to 1.".to_string(),
            4,
            5,
            "x is used where it is not defined.".to_string(),
            "Define it first.".to_string(),
        );
        let json = to_json(&[d]);
        assert!(json.contains("\"code\": \"T0205\""));
        assert!(json.contains("\"line\": 3"));
        assert!(json.contains("Define it first."));
        assert!(json.contains("\"severity\": \"error\""));
    }
}

//! Formatter: print the canonical layout of a source file.
//! One sentence per line, 4 spaces per indent level, LF endings, no BOM,
//! no trailing spaces, at most one blank line in a row.
//!
//! The formatter only lays out a *valid* file. If the splitter reports a
//! structural error (odd indentation, an unexpected indent, an opener with
//! no block, ...), formatting is refused and the diagnostics are returned.
//! That way `lykaon fmt` can never silently flatten an invalid block into a
//! different (but valid-looking) program.
//!
//! Raw text keeps its capitals and its internal spaces. For the text-taking
//! words (`Say`, `Note`, `About`, `Test`, `Example`, `Fail with`, `Log`) the
//! formatter sets exactly one space between the word and the text and leaves
//! the text itself untouched. Lines under `Say lines.` are raw text too.

use crate::diagnostics::{Diagnostic, Severity};
use crate::splitter::{Sentence, split_source};

/// Format `source` to the canonical layout, or return the layout errors that
/// make the file unsafe to format.
pub fn try_format(source: &str, file_name: &str) -> Result<String, Vec<Diagnostic>> {
    let text = strip_bom(source);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let split = split_source(&normalized, file_name)?;

    // Warnings such as T0020-T0024 are exactly what the formatter fixes, so
    // they do not stop it. Errors mean the indentation tree itself is broken;
    // formatting such a file would change its meaning.
    let errors: Vec<Diagnostic> = split
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .cloned()
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut out = String::new();
    let mut last_line: Option<usize> = None;
    emit_block(&split.all, &split.roots, 0, &mut last_line, &mut out);
    // exactly one trailing newline
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn strip_bom(source: &str) -> &str {
    if source.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
        &source[3..]
    } else {
        source
    }
}

fn emit_block(
    all: &[Sentence],
    indices: &[usize],
    level: usize,
    last_line: &mut Option<usize>,
    out: &mut String,
) {
    let indent = " ".repeat(level * 4);
    for &idx in indices {
        let s = &all[idx];

        // Keep at most one blank line from the original file between two
        // sentences. Leading and trailing blank lines disappear.
        if let Some(prev) = *last_line {
            if s.line > prev + 1 && !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
        }

        out.push_str(&indent);
        out.push_str(&canonical_line(s));
        out.push('\n');
        *last_line = Some(s.line);

        if !s.children.is_empty() {
            emit_block(all, &s.children, level + 1, last_line, out);
        }
    }
}

fn canonical_line(s: &Sentence) -> String {
    // Lines under `Say lines.` are literal output. Keep their capitals and
    // their internal spaces; only the indentation is canonical.
    if s.is_raw_text {
        return s.text.clone();
    }

    if let Some((word, rest)) = text_taking_split(&s.text) {
        let rest = rest.trim_start_matches(' ');
        if rest.is_empty() {
            return word;
        }
        return format!("{word} {rest}");
    }

    let mut out = s.text.split_whitespace().collect::<Vec<_>>().join(" ");
    capitalize_first(&mut out);
    out
}

/// Split a sentence that starts with a text-taking word into the canonical
/// word (or two-word phrase) and the raw text after it.
fn text_taking_split(text: &str) -> Option<(String, &str)> {
    let first_end = text.find(' ').unwrap_or(text.len());
    let first = &text[..first_end];
    let first_lower = first.trim_end_matches('.').to_ascii_lowercase();

    if first_lower == "fail" {
        let after_first = &text[first_end..];
        let maybe = after_first.trim_start_matches(' ');
        let second_end = maybe.find(' ').unwrap_or(maybe.len());
        let second = &maybe[..second_end];
        if second.trim_end_matches('.').eq_ignore_ascii_case("with") {
            // `Fail with.` with no text keeps its period; `Fail with text.`
            // gives the text the period.
            if second.ends_with('.') {
                return Some(("Fail with.".to_string(), ""));
            }
            return Some(("Fail with".to_string(), &maybe[second_end..]));
        }
        return None;
    }

    let canonical = match first_lower.as_str() {
        "say" => "Say",
        "note" => "Note",
        "about" => "About",
        "test" => "Test",
        "example" => "Example",
        "log" => "Log",
        _ => return None,
    };

    // `Test.` with no text keeps its period; a real text owns the period.
    if first.ends_with('.') {
        return Some((format!("{canonical}."), ""));
    }
    Some((canonical.to_string(), &text[first_end..]))
}

fn capitalize_first(s: &mut String) {
    if let Some(c) = s.chars().next() {
        if c.is_ascii_lowercase() {
            let upper = c.to_ascii_uppercase();
            s.replace_range(..c.len_utf8(), &upper.to_string());
        }
    }
}

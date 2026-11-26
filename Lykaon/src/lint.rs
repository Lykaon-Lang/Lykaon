//! Linter: legal but suspicious code (Ch.40).
//!
//! `lykaon check` prints the layout warnings T0020-T0024 plus T0141/T0142.
//! `lykaon lint` prints those and T0143-T0147. Every lint is a warning, never
//! an error.

use crate::diagnostics::Diagnostic;
use crate::splitter::{Sentence, split_source};
use std::collections::HashSet;

const SOFT_WORDS: &[&str] = &[
    "open", "close", "table", "row", "column", "value", "name", "text", "number", "list", "map",
    "set", "pair", "model", "data", "file", "folder", "path", "host", "port", "key", "secret",
    "limit", "time", "memory", "process", "task", "channel", "message", "event", "screen",
    "button", "field", "style", "theme", "color", "size", "shape", "space",
];

pub fn lint_source(source: &str, file_name: &str) -> Vec<Diagnostic> {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let split = match split_source(&normalized, file_name) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();

    // Defined names: the word before `is` for Changeable/Constant, the
    // function name after To, and record names after A/An.
    let mut defined: Vec<(String, usize, String)> = Vec::new();
    for &idx in &split.roots {
        let s = &split.all[idx];
        let words: Vec<&str> = s.text.split_whitespace().collect();
        let name = match words.first().map(|w| w.to_ascii_lowercase()) {
            Some(fw) if fw == "changeable" || fw == "constant" => words.get(1).cloned(),
            Some(fw) if fw == "to" => words.get(1).cloned(),
            Some(fw) if fw == "a" || fw == "an" => words.get(1).cloned(),
            Some(_) if words.get(1).is_some_and(|w| w.eq_ignore_ascii_case("is")) => words.first().cloned(),
            _ => None,
        };
        if let Some(n) = name {
            let n = n.trim_end_matches('.').to_string();
            if !n.is_empty() {
                defined.push((n, s.line, s.text.clone()));
            }
        }
    }

    // T0142: a name has the spelling of a soft/library word.
    for (n, line, text) in &defined {
        if SOFT_WORDS.contains(&n.to_ascii_lowercase().as_str()) {
            out.push(warn(
                "T0142",
                "Soft word shadowed.",
                file_name,
                *line,
                text.clone(),
                0,
                n.len(),
                "A name has the spelling of a library word and hides it.",
                "Rename the name, or use the library word where you meant it.",
            ));
        }
    }

    // T0141: a defined name is printed as text in one place.
    for (n, _, _) in &defined {
        let mut found: Option<(usize, String)> = None;
        for &idx in &split.roots {
            let s = &split.all[idx];
            if !s.text.to_ascii_lowercase().starts_with("say ") {
                continue;
            }
            let text = &s.text[4..];
            if contains_word(text, n) {
                found = Some((s.line, s.text.clone()));
                break;
            }
        }
        if let Some((line, text)) = found {
            out.push(warn(
                "T0141",
                "Name in text by accident.",
                file_name,
                line,
                text,
                0,
                n.len(),
                "The same word is printed as text in one place and used as a defined name in another.",
                "Quote it as text with `as text`, or rename the name.",
            ));
        }
    }

    // T0143: a sentence of more than 20 words.
    for &idx in &split.roots {
        let s = &split.all[idx];
        if !s.is_raw_text && s.text.split_whitespace().count() > 20 {
            out.push(warn(
                "T0143",
                "Long sentence.",
                file_name,
                s.line,
                s.text.clone(),
                0,
                s.text.len(),
                "A sentence has more than 20 words.",
                "Split it into shorter sentences.",
            ));
        }
    }

    // T0144: a defined name that is never used.
    for (n, line, text) in &defined {
        let mut used = false;
        for &idx in &split.roots {
            let s = &split.all[idx];
            if s.line == *line {
                continue;
            }
            if contains_word(&s.text, n) {
                used = true;
                break;
            }
        }
        if !used {
            out.push(warn(
                "T0144",
                "Unused name.",
                file_name,
                *line,
                text.clone(),
                0,
                n.len(),
                "A name is defined and never used.",
                "Use it, or remove the definition.",
            ));
        }
    }

    // T0145: a block that holds only Note/About.
    for &idx in &split.roots {
        let s = &split.all[idx];
        if s.children.is_empty() {
            continue;
        }
        let only_notes = s.children.iter().all(|&c| {
            let t = &split.all[c];
            let fw = t.text.split_whitespace().next().unwrap_or("");
            t.is_raw_text || fw.eq_ignore_ascii_case("note") || fw.eq_ignore_ascii_case("about")
        });
        if only_notes {
            out.push(warn(
                "T0145",
                "Empty block.",
                file_name,
                s.line,
                s.text.clone(),
                0,
                s.text.len(),
                "A block holds no sentence that does anything.",
                "Remove it or fill it with real sentences.",
            ));
        }
    }

    // T0146: both branches of a choice do the same thing.
    for (i, &idx) in split.roots.iter().enumerate() {
        let s = &split.all[idx];
        if !s.text.to_ascii_lowercase().starts_with("if ") {
            continue;
        }
        let mut otherwise: Option<&Sentence> = None;
        if let Some(&next) = split.roots.get(i + 1) {
            let t = &split.all[next];
            if t.text.to_ascii_lowercase().starts_with("otherwise") {
                otherwise = Some(t);
            }
        }
        if let Some(o) = otherwise {
            let a: Vec<String> = s
                .children
                .iter()
                .map(|&c| split.all[c].text.to_ascii_lowercase())
                .collect();
            let b: Vec<String> = o
                .children
                .iter()
                .map(|&c| split.all[c].text.to_ascii_lowercase())
                .collect();
            if !a.is_empty() && a == b {
                out.push(warn(
                    "T0146",
                    "Identical branches.",
                    file_name,
                    s.line,
                    s.text.clone(),
                    0,
                    s.text.len(),
                    "Both branches of a choice do the same thing.",
                    "Merge the branches, or make them different.",
                ));
            }
        }
    }

    // T0147: the same number appears many times without a name.
    let mut counts: std::collections::HashMap<String, (usize, String)> = Default::default();
    for &idx in &split.roots {
        let s = &split.all[idx];
        if s.is_raw_text {
            continue;
        }
        let fw = s.text.split_whitespace().next().unwrap_or("");
        if fw.eq_ignore_ascii_case("say")
            || fw.eq_ignore_ascii_case("note")
            || fw.eq_ignore_ascii_case("about")
        {
            continue;
        }
        for w in s.text.split_whitespace() {
            let t = w.trim_end_matches('.');
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
                let e = counts.entry(t.to_string()).or_insert((s.line, s.text.clone()));
                e.0 = s.line;
            }
        }
    }
    for (n, (line, text)) in counts {
        if !n.is_empty() {
            // count occurrences again for the threshold
            let mut c = 0;
            for &idx in &split.roots {
                let s = &split.all[idx];
                if s.is_raw_text {
                    continue;
                }
                for w in s.text.split_whitespace() {
                    if w.trim_end_matches('.') == n {
                        c += 1;
                    }
                }
            }
            if c >= 3 {
                out.push(warn(
                    "T0147",
                    "Magic number.",
                    file_name,
                    line,
                    text,
                    0,
                    n.len(),
                    "The same number appears many times without a name.",
                    "Give the number a name with `Constant <name> is <number>.`",
                ));
            }
        }
    }

    out
}

fn contains_word(text: &str, word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|p| p == w)
}

fn warn(
    code: &'static str,
    what: &str,
    file: &str,
    line: usize,
    sentence: String,
    lo: usize,
    hi: usize,
    why: &str,
    fix: &str,
) -> Diagnostic {
    Diagnostic::warning(
        code,
        what.to_string(),
        file.to_string(),
        line,
        sentence,
        lo,
        hi,
        why.to_string(),
        fix.to_string(),
    )
}
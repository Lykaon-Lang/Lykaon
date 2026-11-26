//! Lykaon diagnostics (M1a + M5c upgrade).
//!
//! Every diagnostic has:
//!   - a stable code (one rule),
//!   - a severity (Error, Warning, Note, Help),
//!   - a source span (file, line, column, byte offsets),
//!   - a primary label and optional secondary labels,
//!   - optional machine-applicable suggestions (span + replacement),
//!   - a plain-English Why and Fix.
//!
//! The ASCII rendering is unchanged for diagnostics without labels or
//! suggestions, so every existing golden test keeps its exact text. Colours
//! are added only when the caller asks for them (a real terminal).

use std::fmt;
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl Severity {
    pub fn word(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Note => "Note",
            Severity::Help => "Help",
        }
    }

    fn color(self) -> &'static str {
        match self {
            Severity::Error => "\x1b[31m",
            Severity::Warning => "\x1b[33m",
            Severity::Note => "\x1b[36m",
            Severity::Help => "\x1b[32m",
        }
    }
}

/// One labelled span. `primary` is the main `^^^^`; the others are `----`.
#[derive(Clone, Debug)]
pub struct Label {
    pub text: String,
    pub lo: usize,
    pub hi: usize,
    pub primary: bool,
}

/// A machine-applicable suggestion: replace `lo..hi` with `replacement`.
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub text: String,
    pub lo: usize,
    pub hi: usize,
    pub replacement: String,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub what: String,
    pub file: String,
    pub line: usize,
    /// 1-based column of `mark_lo` in `sentence`.
    pub column: usize,
    pub sentence: String,
    pub mark_lo: usize,
    pub mark_hi: usize,
    pub why: String,
    pub fix: String,
    /// Secondary labels (empty for ordinary diagnostics).
    pub labels: Vec<Label>,
    /// Machine-applicable suggestions (empty for ordinary diagnostics).
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    fn new(
        severity: Severity,
        code: &'static str,
        what: String,
        file: String,
        line: usize,
        sentence: String,
        mark_lo: usize,
        mark_hi: usize,
        why: String,
        fix: String,
    ) -> Self {
        let column = sentence
            .chars()
            .take(mark_lo.min(sentence.len()))
            .count()
            + 1;
        Self {
            severity,
            code,
            what,
            file,
            line,
            column,
            sentence,
            mark_lo,
            mark_hi,
            why,
            fix,
            labels: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn error(
        code: &'static str,
        what: String,
        file: String,
        line: usize,
        sentence: String,
        mark_lo: usize,
        mark_hi: usize,
        why: String,
        fix: String,
    ) -> Self {
        Self::new(Severity::Error, code, what, file, line, sentence, mark_lo, mark_hi, why, fix)
    }

    pub fn warning(
        code: &'static str,
        what: String,
        file: String,
        line: usize,
        sentence: String,
        mark_lo: usize,
        mark_hi: usize,
        why: String,
        fix: String,
    ) -> Self {
        Self::new(Severity::Warning, code, what, file, line, sentence, mark_lo, mark_hi, why, fix)
    }

    /// A Note: information that does not stop compilation.
    pub fn note(
        code: &'static str,
        what: String,
        file: String,
        line: usize,
        sentence: String,
        mark_lo: usize,
        mark_hi: usize,
        why: String,
        fix: String,
    ) -> Self {
        Self::new(Severity::Note, code, what, file, line, sentence, mark_lo, mark_hi, why, fix)
    }

    /// Help: a short hint attached to a diagnostic.
    pub fn help(
        code: &'static str,
        what: String,
        file: String,
        line: usize,
        sentence: String,
        mark_lo: usize,
        mark_hi: usize,
        why: String,
        fix: String,
    ) -> Self {
        Self::new(Severity::Help, code, what, file, line, sentence, mark_lo, mark_hi, why, fix)
    }

    pub fn with_label(mut self, text: impl Into<String>, lo: usize, hi: usize, primary: bool) -> Self {
        self.labels.push(Label { text: text.into(), lo, hi, primary });
        self
    }

    pub fn with_suggestion(
        mut self,
        text: impl Into<String>,
        lo: usize,
        hi: usize,
        replacement: impl Into<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            text: text.into(),
            lo,
            hi,
            replacement: replacement.into(),
        });
        self
    }

    /// The ASCII rendering. With `color`, severity and carets use ANSI colours.
    /// Suggestions are hidden (see `render_ex`).
    pub fn render(&self, color: bool) -> String {
        self.render_ex(color, false)
    }

    /// The ASCII rendering. With `color`, severity and carets use ANSI colours.
    /// `show_suggestions` adds the machine-applicable suggestions (text mode
    /// `--suggest`); ordinary output keeps them out so golden tests are stable.
    pub fn render_ex(&self, color: bool, show_suggestions: bool) -> String {
        let mut out = String::new();
        let paint = |s: &str, c: &str| -> String {
            if color { format!("{c}{s}\x1b[0m") } else { s.to_string() }
        };
        let colour = self.severity.color();
        writeln!(
            out,
            "{}: {} ({})",
            paint(self.severity.word(), colour),
            self.what,
            self.code
        )
        .unwrap();
        let where_prefix = format!("Where: {}, line {}: ", self.file, self.line);
        writeln!(out, "{}{}", where_prefix, self.sentence).unwrap();
        let width = (self.mark_hi.saturating_sub(self.mark_lo)).max(1);
        let mark = " ".repeat(where_prefix.len() + self.mark_lo) + &"^".repeat(width);
        writeln!(out, "{}", paint(&mark, colour)).unwrap();
        writeln!(out, "Why: {}", self.why).unwrap();
        writeln!(out, "Fix: {}", self.fix).unwrap();
        // Secondary labels (`----`) are additive; ordinary diagnostics have
        // none, and they show only in verbose (`--suggest`) or JSON.
        for label in self
            .labels
            .iter()
            .take(if show_suggestions { usize::MAX } else { 0 })
        {
            if label.primary {
                continue;
            }
            let lo = label.lo.min(self.sentence.len());
            let hi = label.hi.max(lo).min(self.sentence.len());
            let width = (hi.saturating_sub(lo)).max(1);
            let mark = " ".repeat(where_prefix.len() + lo) + &"-".repeat(width);
            writeln!(out, "{}", paint(&mark, "\x1b[36m")).unwrap();
            writeln!(out, "  {}", label.text).unwrap();
        }
        // Machine-applicable suggestions are shown only when asked (or in JSON).
        for s in self.suggestions.iter().take(if show_suggestions { usize::MAX } else { 0 }) {
            writeln!(out, "Suggestion: {}", s.text).unwrap();
            if !s.replacement.is_empty() {
                writeln!(out, "  replace with: {}", s.replacement).unwrap();
            }
        }
        out
    }

    /// JSON form used by `lykaon check --json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code,
            "severity": self.severity.word().to_ascii_lowercase(),
            "file": self.file,
            "line": self.line,
            "column": self.column,
            "message": self.what,
            "sentence": self.sentence,
            "mark_lo": self.mark_lo,
            "mark_hi": self.mark_hi,
            "why": self.why,
            "fix": self.fix,
            "labels": self.labels.iter().map(|l| serde_json::json!({
                "text": l.text, "lo": l.lo, "hi": l.hi, "primary": l.primary
            })).collect::<Vec<_>>(),
            "suggestions": self.suggestions.iter().map(|s| serde_json::json!({
                "text": s.text, "lo": s.lo, "hi": s.hi, "replacement": s.replacement
            })).collect::<Vec<_>>()
        })
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Diagnostic {
        Diagnostic::error(
            "T0205",
            "totaal is not defined.".to_string(),
            "t.lykaon".to_string(),
            2,
            "Set totaal to 6.".to_string(),
            4,
            10,
            "totaal is used where it is not defined.".to_string(),
            "Define it first, or check the spelling.".to_string(),
        )
    }

    #[test]
    fn code_and_column() {
        let d = sample();
        assert_eq!(d.code, "T0205");
        assert_eq!(d.column, 5); // byte 4 -> 1-based column 5
        assert_eq!(d.severity.word(), "Error");
    }

    #[test]
    fn primary_label_is_carets() {
        let text = sample().render(false);
        assert!(text.contains("^^^^^^"), "{text}");
    }

    #[test]
    fn secondary_label_is_dashes() {
        let d = sample().with_label("earlier here", 0, 3, false);
        let text = d.render_ex(false, true);
        assert!(text.contains("---"), "{text}");
        assert!(text.contains("earlier here"), "{text}");
    }

    #[test]
    fn suggestion_hidden_unless_verbose() {
        let d = sample().with_suggestion("Did you mean total?", 4, 10, "total");
        let plain = d.render(false);
        assert!(!plain.contains("Suggestion:"), "{plain}");
        let verbose = d.render_ex(false, true);
        assert!(verbose.contains("Suggestion: Did you mean total?"), "{verbose}");
        assert!(verbose.contains("replace with: total"), "{verbose}");
    }

    #[test]
    fn severity_words() {
        assert_eq!(Severity::Error.word(), "Error");
        assert_eq!(Severity::Warning.word(), "Warning");
        assert_eq!(Severity::Note.word(), "Note");
        assert_eq!(Severity::Help.word(), "Help");
    }

    #[test]
    fn color_adds_ansi() {
        let plain = sample().render(false);
        let colored = sample().render(true);
        assert!(!plain.contains('\x1b'));
        assert!(colored.contains("\x1b[31m"), "{colored}");
    }

    #[test]
    fn json_has_every_field() {
        let d = sample()
            .with_label("earlier", 0, 3, false)
            .with_suggestion("Did you mean total?", 4, 10, "total");
        let j = d.to_json();
        assert_eq!(j["code"], "T0205");
        assert_eq!(j["severity"], "error");
        assert_eq!(j["column"], 5);
        assert_eq!(j["labels"][0]["text"], "earlier");
        assert_eq!(j["suggestions"][0]["replacement"], "total");
    }

    #[test]
    fn note_and_help_render() {
        let n = Diagnostic::note(
            "T9000",
            "a note".to_string(),
            "f".to_string(),
            1,
            "Say x.".to_string(),
            0,
            1,
            "why".to_string(),
            "fix".to_string(),
        );
        assert!(n.render(false).starts_with("Note: a note (T9000)"));
        let h = Diagnostic::help(
            "T9001",
            "a help".to_string(),
            "f".to_string(),
            1,
            "Say x.".to_string(),
            0,
            1,
            "why".to_string(),
            "fix".to_string(),
        );
        assert!(h.render(false).starts_with("Help: a help (T9001)"));
    }
}
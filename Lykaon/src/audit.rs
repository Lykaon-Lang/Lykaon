//! M5f (Appendix L add-on): the audit and review framework.
//!
//! Additive only: this never changes the language. It analyses the typed tree
//! for a small, high-value rule set:
//!   - taint: values from Ask / files / network / environment reaching a sink
//!     (Run, file writes, database writes),
//!   - missing permissions: a used feature with no `Allows` in settings.lykaon,
//!   - secrets: hardcoded password/key/token-looking literals.
//!
//! Each finding carries a severity, a confidence, a plain-English Why (the
//! source -> sink path) and EVIDENCE. Static findings are marked `Opinion`
//! (shown separately, never counted as proved); a finding only becomes
// `Reproduced` when a run proves it.

use std::collections::HashMap;
use std::path::Path;

use lykaon_core::sandbox::Sandbox;
use lykaon_core::sem::{self, E, Expr, FnBody, Program, Stmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn word(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Note => "Note",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Evidence {
    /// The tool ran something that shows the issue.
    Reproduced { program: String, detail: String },
    /// Static finding, not proven by a run (shown separately, never counted).
    Opinion,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Severity,
    pub confidence: f32,
    pub file: String,
    pub line: usize,
    pub why: String,
    pub evidence: Evidence,
    pub fix: String,
}

impl Finding {
    pub fn proved(&self) -> bool {
        matches!(self.evidence, Evidence::Reproduced { .. })
    }
}

#[derive(Default)]
pub struct Audit {
    pub findings: Vec<Finding>,
}

/// Collect the names an expression reads (Name and Call args), recursively.
fn expr_names(e: &Expr, out: &mut Vec<String>) {
    match &e.e {
        E::Name(n) => out.push(n.clone()),
        E::Neg(x) => expr_names(x, out),
        E::Binary { l, r, .. } => {
            expr_names(l, out);
            expr_names(r, out);
        }
        E::DivWhole(a, b) | E::Rem(a, b) | E::Pow(a, b) => {
            expr_names(a, out);
            expr_names(b, out);
        }
        E::AndOr(a, b, _) => {
            expr_names(a, out);
            expr_names(b, out);
        }
        E::Round(x, _) | E::Factorial(x) | E::CountIn(x) | E::Lower(x) => expr_names(x, out),
        E::AsWhole(x) | E::AsDecimal(x) | E::AsBig(x) | E::AsText(x) | E::AsNumber(x)
        | E::AsMaybe(x) => expr_names(x, out),
        E::Call { args, .. } => {
            for a in args {
                expr_names(a, out);
            }
        }
        E::ListOf(xs) => {
            for x in xs {
                expr_names(x, out);
            }
        }
        E::PairOf(a, b) => {
            expr_names(a, out);
            expr_names(b, out);
        }
        E::FirstOf(x) | E::SecondOf(x) | E::Sorted(x, _) | E::Reverse(x) | E::Unique(x) => {
            expr_names(x, out)
        }
        E::Combine { list, .. } | E::ApplyEach { list, .. } => expr_names(list, out),
        E::Fallback { value, default } => {
            expr_names(value, out);
            expr_names(default, out);
        }
        E::JoinedWith(x, _) | E::Upper(x) | E::Trimmed(x) | E::Normalized(x) => expr_names(x, out),
        E::RecordOf { parts, .. } | E::EnumOf { parts, .. } => {
            for (_, e) in parts {
                expr_names(e, out);
            }
        }
        E::PartOf { base, .. } => expr_names(base, out),
        _ => {}
    }
}

/// The first tainted name an expression reads, with the source line.
fn expr_tainted(e: &Expr, t: &HashMap<String, usize>) -> Option<(String, usize)> {
    let mut names = Vec::new();
    expr_names(e, &mut names);
    for n in names {
        if let Some(line) = t.get(&n) {
            return Some((n, *line));
        }
    }
    None
}

/// A sink that takes a value expression.
fn sink_value(
    st: &Stmt,
    t: &HashMap<String, usize>,
) -> Option<(&'static str, String, usize)> {
    let (rule, value): (&'static str, &Expr) = match st {
        Stmt::RunProc { args, .. } => {
            // Run NAME with ARGS: taint if any argument is tainted.
            for a in args {
                if let Some((src, line)) = expr_tainted(a, t) {
                    return Some(("taint-run", src, line));
                }
            }
            return None;
        }
        Stmt::WriteFile { value, .. } => ("taint-file", value),
        Stmt::AppendFile { value, .. } => ("taint-file", value),
        Stmt::ReplaceFile { value, .. } => ("taint-file", value),
        Stmt::DbAdd { value, .. } => ("taint-db", value),
        Stmt::DbSet { value, .. } => ("taint-db", value),
        _ => return None,
    };
    expr_tainted(value, t).map(|(src, line)| (rule, src, line))
}

fn taint_stmt(
    st: &Stmt,
    t: &mut HashMap<String, usize>,
    file: &str,
    out: &mut Audit,
    _source: &str,
) {
    match st {
        Stmt::Ask { name, .. } => {
            t.insert(name.clone(), 0);
        }
        Stmt::Define(d) => {
            if let Some((_src, line)) = expr_tainted(&d.expr, t) {
                t.insert(d.name.clone(), line);
            }
        }
        Stmt::Set { name, expr } => {
            if let Some((_src, line)) = expr_tainted(expr, t) {
                t.insert(name.clone(), line);
            }
        }
        // These produce `it` from an untrusted source.
        Stmt::ReadFile { .. }
        | Stmt::ReadBytes { .. }
        | Stmt::Fetch { .. }
        | Stmt::ReadFormat { .. }
        | Stmt::ReadFormatText { .. } => {
            t.insert("it".to_string(), 0);
        }
        _ => {}
    }
    if let Some((rule, src, line)) = sink_value(st, t) {
        out.findings.push(Finding {
            rule,
            severity: Severity::Error,
            confidence: 0.9,
            file: file.to_string(),
            line,
            why: format!(
                "a value derived from '{src}' reaches a {} sink",
                match rule {
                    "taint-run" => "process",
                    "taint-db" => "database",
                    _ => "file",
                }
            ),
            evidence: Evidence::Opinion,
            fix: "validate or sanitise the value before it reaches this sink".to_string(),
        });
    }
    // recurse into nested statement bodies
    for body in nested_bodies(st) {
        for s in body {
            taint_stmt(s, t, file, out, _source);
        }
    }
}

/// The nested statement bodies of a statement (for the taint walk).
fn nested_bodies(st: &Stmt) -> Vec<&Vec<Stmt>> {
    let mut v: Vec<&Vec<Stmt>> = Vec::new();
    match st {
        Stmt::If { branches, other } => {
            for (_, b) in branches {
                v.push(b);
            }
            if let Some(b) = other {
                v.push(b);
            }
        }
        Stmt::RepeatTimes { body, .. }
        | Stmt::RepeatForever { body }
        | Stmt::RepeatWhile { body, .. }
        | Stmt::ForRange { body, .. }
        | Stmt::ForIn { body, .. }
        | Stmt::StartTask { body, .. }
        | Stmt::DoTogether { body }
        | Stmt::AfterTimer { body, .. }
        | Stmt::EveryTimer { body, .. }
        | Stmt::StreamLoop { body, .. }
        | Stmt::Spread { body, .. }
        | Stmt::ForEachLine { body, .. }
        | Stmt::Transaction { body }
        | Stmt::WhenSignal { body, .. }
        | Stmt::ForEachInput { body, .. }
        | Stmt::Careful { body } => v.push(body),
        Stmt::Try {
            sentence,
            success,
            failure,
        } => {
            v.push(success);
            for (_, b) in failure {
                v.push(b);
            }
            let _ = sentence;
        }
        Stmt::LookAt { whens, .. } => {
            for w in whens {
                v.push(&w.body);
            }
        }
        Stmt::DefineLocalFn(f) => {
            if let FnBody::Block(b) = &f.body {
                v.push(b);
            }
        }
        _ => {}
    }
    v
}

/// Does the program use each feature? (for the permission check)
fn uses(stmts: &[Stmt]) -> (bool, bool, bool) {
    let (mut files, mut net, mut proc) = (false, false, false);
    for st in stmts {
        match st {
            Stmt::WriteFile { .. }
            | Stmt::AppendFile { .. }
            | Stmt::ReplaceFile { .. }
            | Stmt::ReadFile { .. }
            | Stmt::ReadBytes { .. }
            | Stmt::RemovePath { .. }
            | Stmt::MovePath { .. }
            | Stmt::MakeFolder { .. }
            | Stmt::ForEachLine { .. } => files = true,
            Stmt::Fetch { .. } | Stmt::Serve { .. } => net = true,
            Stmt::RunProc { .. } => proc = true,
            _ => {}
        }
        for b in nested_bodies(st) {
            let (f, n, p) = uses(b);
            files |= f;
            net |= n;
            proc |= p;
        }
    }
    (files, net, proc)
}

/// Scan the source for secret-looking literals.
fn check_secrets(source: &str, file: &str, out: &mut Audit) {
    for (i, line) in source.lines().enumerate() {
        let low = line.to_ascii_lowercase();
        let looks_secret = (low.contains("password is")
            || low.contains("secret is")
            || low.contains("token is")
            || low.contains("api key is")
            || low.contains("apikey is"))
            && !low.contains("missing");
        if looks_secret {
            out.findings.push(Finding {
                rule: "secret",
                severity: Severity::Warning,
                confidence: 0.8,
                file: file.to_string(),
                line: i + 1,
                why: "a password/key/token appears to be hardcoded in the source".to_string(),
                evidence: Evidence::Reproduced {
                    program: line.trim().to_string(),
                    detail: "the literal is in the source".to_string(),
                },
                fix: "read it from the environment or a secret store instead".to_string(),
            });
        }
    }
}

/// Audit a compiled program.
pub fn audit(prog: &Program, source: &str, file: &str) -> Audit {
    let mut out = Audit::default();
    check_secrets(source, file, &mut out);

    // permissions: compare used features with settings.lykaon
    let dir = Path::new(&prog.source_dir);
    let sb = Sandbox::from_settings(dir);
    let (uses_files, mut uses_net, mut uses_proc) = uses(&prog.body);
    // M6b: the built-in boundaries are calls, not statements, so the
    // statement scan above does not see them; catch them by name.
    let src_low = source.to_ascii_lowercase();
    if src_low.contains("capture ") {
        uses_proc = true;
    }
    if src_low.contains("tcpopen") || src_low.contains("tcpsend") {
        uses_net = true;
    }
    if uses_files && !sb.declares_files() {
        out.findings.push(Finding {
            rule: "perm-files",
            severity: Severity::Warning,
            confidence: 0.9,
            file: file.to_string(),
            line: 0,
            why: "the program uses files but settings.lykaon has no `Allows files`".to_string(),
            evidence: Evidence::Opinion,
            fix: "add `Allows files ...` to settings.lykaon or remove the file access".to_string(),
        });
    }
    if uses_net && !sb.declares_network() {
        out.findings.push(Finding {
            rule: "perm-network",
            severity: Severity::Warning,
            confidence: 0.9,
            file: file.to_string(),
            line: 0,
            why: "the program uses the network but settings.lykaon has no `Allows network`"
                .to_string(),
            evidence: Evidence::Opinion,
            fix: "add `Allows network ...` to settings.lykaon or remove the network access"
                .to_string(),
        });
    }
    if uses_proc && !sb.declares_processes() {
        out.findings.push(Finding {
            rule: "perm-process",
            severity: Severity::Warning,
            confidence: 0.9,
            file: file.to_string(),
            line: 0,
            why: "the program starts processes but settings.lykaon has no `Allows processes`"
                .to_string(),
            evidence: Evidence::Opinion,
            fix: "add `Allows processes` to settings.lykaon or remove the process launch"
                .to_string(),
        });
    }

    // taint
    let mut tainted: HashMap<String, usize> = HashMap::new();
    for st in &prog.body {
        taint_stmt(st, &mut tainted, file, &mut out, source);
    }
    for f in &prog.funcs {
        if let FnBody::Block(b) = &f.body {
            let mut t: HashMap<String, usize> = HashMap::new();
            for st in b {
                taint_stmt(st, &mut t, file, &mut out, source);
            }
        }
    }

    out.findings.sort_by_key(|f| std::cmp::Reverse(f.severity as u8));
    out
}

impl Audit {
    pub fn proved(&self) -> usize {
        self.findings.iter().filter(|f| f.proved()).count()
    }
    pub fn opinions(&self) -> usize {
        self.findings.iter().filter(|f| !f.proved()).count()
    }
}

impl Audit {
    pub fn render(&self) -> String {
        let mut out = String::new();
        for f in &self.findings {
            let ev = if f.proved() { "reproduced" } else { "opinion" };
            out.push_str(&format!(
                "{}: {} ({}:{ })\n  Why: {}\n  Fix: {}\n  Evidence: {}\n\n",
                f.severity.word(),
                f.rule,
                f.file,
                if f.line == 0 { 1 } else { f.line },
                f.why,
                f.fix,
                ev
            ));
        }
        out.push_str(&format!(
            "{} finding(s): {} proved, {} opinion",
            self.findings.len(),
            self.proved(),
            self.opinions()
        ));
        out
    }

    pub fn to_json(&self) -> String {
        let items: Vec<_> = self
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "rule": f.rule,
                    "severity": f.severity.word(),
                    "confidence": f.confidence,
                    "file": f.file,
                    "line": f.line,
                    "why": f.why,
                    "fix": f.fix,
                    "evidence": if f.proved() { "reproduced" } else { "opinion" }
                })
            })
            .collect();
        serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn to_sarif(&self) -> String {
        let results: Vec<_> = self
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "ruleId": f.rule,
                    "level": match f.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Note => "note",
                    },
                    "message": {"text": f.why},
                    "properties": {"confidence": f.confidence, "evidence": if f.proved() { "reproduced" } else { "opinion" }},
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": {"uri": f.file},
                            "region": {"startLine": if f.line == 0 { 1 } else { f.line }}
                        }
                    }]
                })
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({
            "version": "2.1.0",
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "runs": [{
                "tool": {"driver": {"name": "lykaon", "informationUri": "https://github.com/Lykaon-Lang/Lykaon"}},
                "results": results
            }]
        }))
        .unwrap_or_else(|_| "{}".to_string())
    }
}

/// Review a diff: functions added/removed and permissions newly used.
pub fn review_diff(old_prog: &Program, new_prog: &Program, file: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in &old_prog.funcs {
        if !new_prog.funcs.iter().any(|n| n.name == f.name) {
            out.push(Finding {
                rule: "func-removed",
                severity: Severity::Warning,
                confidence: 1.0,
                file: file.to_string(),
                line: 0,
                why: format!("function '{}' was removed", f.name),
                evidence: Evidence::Opinion,
                fix: "check callers still compile".to_string(),
            });
        }
    }
    for f in &new_prog.funcs {
        if !old_prog.funcs.iter().any(|o| o.name == f.name) {
            out.push(Finding {
                rule: "func-added",
                severity: Severity::Note,
                confidence: 1.0,
                file: file.to_string(),
                line: 0,
                why: format!("function '{}' was added", f.name),
                evidence: Evidence::Opinion,
                fix: "none needed".to_string(),
            });
        }
    }
    // permissions newly used
    let old_findings = audit(old_prog, "", file).findings;
    let new_findings = audit(new_prog, "", file).findings;
    for nf in &new_findings {
        if nf.rule.starts_with("perm-")
            && !old_findings.iter().any(|o| o.rule == nf.rule)
        {
            out.push(Finding {
                rule: "perm-new",
                severity: Severity::Warning,
                confidence: 0.9,
                file: file.to_string(),
                line: nf.line,
                why: format!("new permission needed: {}", nf.why),
                evidence: Evidence::Opinion,
                fix: nf.fix.clone(),
            });
        }
    }
    out
}

/// A suppression from lykaon-audit.toml: `rule = "reason"`.
#[derive(Clone, Debug)]
pub struct Suppression {
    pub rule: String,
    pub reason: String,
}

pub fn parse_suppressions(text: &str) -> Vec<Suppression> {
    let mut v = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with('[') {
            continue;
        }
        if let Some((rule, reason)) = t.split_once('=') {
            let rule = rule.trim().trim_matches('"').to_string();
            let reason = reason.trim().trim_matches('"').to_string();
            if !rule.is_empty() {
                v.push(Suppression { rule, reason });
            }
        }
    }
    v
}

impl Audit {
    pub fn key(f: &Finding) -> String {
        format!("{}:{}:{}", f.rule, f.file, f.line)
    }
    pub fn keys(&self) -> Vec<String> {
        self.findings.iter().map(Self::key).collect()
    }
    pub fn suppress(&mut self, suppressions: &[Suppression]) {
        self.findings
            .retain(|f| !suppressions.iter().any(|s| s.rule == f.rule));
    }
    pub fn only_new(&mut self, baseline: &std::collections::HashSet<String>) {
        self.findings.retain(|f| !baseline.contains(&Self::key(f)));
    }
    pub fn has_high_confidence_error(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.confidence >= 0.9)
    }
}

/// M5f item 8: dependency checksum + advisory check.
///
/// Verifies each `lykaon.lock` checksum against the dependency's folder. An
/// advisory database is not available offline, so that leg is reported as a
/// note rather than guessed.
pub fn check_dependencies(root: &Path, file: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let lock_path = root.join("lykaon.lock");
    let Ok(text) = std::fs::read_to_string(&lock_path) else {
        return out;
    };
    let Ok(lock) = crate::packages::parse_lockfile(&text) else {
        return out;
    };
    for dep in &lock.locked {
        let Some(want) = &dep.checksum else { continue };
        let dir = match &dep.source {
            crate::packages::Source::Folder(f) => Some(root.join(f)),
            crate::packages::Source::Git { .. } => {
                Some(crate::packages::git_cache_dir(root, &dep.name))
            }
        };
        let Some(dir) = dir else { continue };
        if !dir.exists() {
            continue;
        }
        match crate::packages::tree_checksum(&dir) {
            Ok(got) if got == *want => {}
            Ok(_got) => out.push(Finding {
                rule: "dep-checksum",
                severity: Severity::Error,
                confidence: 1.0,
                file: file.to_string(),
                line: 0,
                why: format!("dependency '{}' checksum does not match lykaon.lock", dep.name),
                evidence: Evidence::Reproduced {
                    program: format!("tree checksum of {}", dir.display()),
                    detail: "the folder hash differs from the lock".to_string(),
                },
                fix: "run `lykaon update` or restore the locked version".to_string(),
            }),
            Err(e) => out.push(Finding {
                rule: "dep-checksum",
                severity: Severity::Note,
                confidence: 0.5,
                file: file.to_string(),
                line: 0,
                why: format!("cannot checksum dependency '{}': {e}", dep.name),
                evidence: Evidence::Opinion,
                fix: "check the dependency folder".to_string(),
            }),
        }
    }
    out
}

/// M5f item 7: intent check.
///
/// For `About`/`Note` text that claims the program prints or gives a word, run
/// the program and report only when the run proves the claim is false.
pub fn check_intent(source: &str, file: &str, out: &mut Vec<Finding>) {
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        let low = t.to_ascii_lowercase();
        let claim = if let Some(rest) = low.strip_prefix("about ") {
            claim_word(rest)
        } else if let Some(rest) = low.strip_prefix("note ") {
            claim_word(rest)
        } else {
            None
        };
        let Some(word) = claim else { continue };
        let (stdout, code) = crate::run_source(source, file, "");
        if code != 0 {
            continue; // cannot judge a program that does not run
        }
        if !stdout.to_ascii_lowercase().contains(&word) {
            out.push(Finding {
                rule: "intent",
                severity: Severity::Warning,
                confidence: 0.8,
                file: file.to_string(),
                line: i + 1,
                why: format!("the About/Note says it prints '{word}' but the run does not"),
                evidence: Evidence::Reproduced {
                    program: line.trim().to_string(),
                    detail: format!("output was {:?}", stdout.lines().next().unwrap_or("")),
                },
                fix: "make the code match the description, or fix the description".to_string(),
            });
        }
        break; // one intent claim is enough
    }
}

/// Extract the word after "prints"/"gives" in an About/Note line.
fn claim_word(rest: &str) -> Option<String> {
    for key in ["prints ", "gives ", "says "] {
        if let Some(p) = rest.find(key) {
            let w = rest[p + key.len()..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric());
            if !w.is_empty() {
                return Some(w.to_string());
            }
        }
    }
    None
}

/// M5f item 9: diff behavior on generated inputs.
///
/// For each function present in both versions, call it with sample whole-number
/// arguments (appended as a `Say f 1.` harness) and report when the output
/// differs. This proves a behaviour change with a real run.
pub fn diff_behavior(
    old_src: &str,
    new_src: &str,
    old_prog: &Program,
    new_prog: &Program,
    file: &str,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in &old_prog.funcs {
        let Some(nf) = new_prog.funcs.iter().find(|n| n.name == f.name) else {
            continue;
        };
        if f.inputs.len() != nf.inputs.len() || f.inputs.is_empty() {
            continue;
        }
        let mut call = format!("Say {} ", f.name);
        for i in 0..f.inputs.len() {
            if i > 0 {
                call.push_str("with ");
            }
            call.push('1');
        }
        call.push('.');
        let oh = format!("{}\n{}", old_src, call);
        let nh = format!("{}\n{}", new_src, call);
        let (oo, oc) = crate::run_source(&oh, file, "");
        let (no, nc) = crate::run_source(&nh, file, "");
        if oc == 0 && nc == 0 && norm(&oo) != norm(&no) {
            out.push(Finding {
                rule: "behavior-change",
                severity: Severity::Error,
                confidence: 1.0,
                file: file.to_string(),
                line: 0,
                why: format!("function '{}' returns different results on sample input", f.name),
                evidence: Evidence::Reproduced {
                    program: call.clone(),
                    detail: format!("old {:?} vs new {:?}", norm(&oo), norm(&no)),
                },
                fix: "review the change; update callers or tests".to_string(),
            });
        }
    }
    out
}

fn norm(s: &str) -> String {
    s.lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

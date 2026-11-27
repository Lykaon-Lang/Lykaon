//! M6b: `lykaon bindgen` — a small C header → `.stub.shelf` generator.
//!
//! Supported prototypes: `int add(int a, int b);`, `double cos(double x);`,
//! `const char *name(void);`. No macros, no structs.

use crate::cbridge::CKind;

pub fn bindgen(source: &str) -> String {
    let mut out = String::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let line = line.strip_suffix(';').unwrap_or(line).trim();
        let line = line.strip_prefix("extern ").unwrap_or(line).trim();
        let Some(open) = line.find('(') else { continue };
        let Some(close) = line.rfind(')') else { continue };
        let head = line[..open].trim();
        let args = &line[open + 1..close];
        let mut it = head.rsplitn(2, ' ');
        let mut name = it.next().unwrap_or("").trim().to_string();
        let mut ret = it.next().unwrap_or("").trim().to_string();
        if name.starts_with('*') {
            ret.push(' ');
            ret.push('*');
            name.remove(0);
        }
        let name = name.trim();
        let ret = ret.trim();
        if name.is_empty() || ret.is_empty() {
            continue;
        }
        let Some(result) = ckind(ret) else { continue };
        if ret == "void" {
            continue;
        }
        let mut inputs = Vec::new();
        for arg in args.split(',') {
            let arg = arg.trim();
            if arg.is_empty() || arg == "void" {
                continue;
            }
            let mut ait = arg.rsplitn(2, ' ');
            let _aname = ait.next();
            let atype = ait.next().unwrap_or(arg).trim();
            if let Some(k) = ckind(atype) {
                inputs.push(k);
            }
        }
        let takes = if inputs.is_empty() {
            "nothing".to_string()
        } else {
            inputs
                .iter()
                .map(ckind_word)
                .collect::<Vec<_>>()
                .join(" and ")
        };
        out.push_str(&format!(
            "C function {name} as {name} takes {takes} gives {}.\n",
            ckind_word(&result)
        ));
    }
    out
}

fn ckind(t: &str) -> Option<CKind> {
    let t = t.trim();
    if t == "void" {
        return None;
    }
    if t.ends_with('*') {
        Some(CKind::Text)
    } else if t.contains("double") || t.contains("float") {
        Some(CKind::Decimal)
    } else if t.contains("int") || t.contains("long") || t.contains("size_t") {
        Some(CKind::Whole)
    } else {
        None
    }
}

fn ckind_word(k: &CKind) -> &'static str {
    match k {
        CKind::Whole => "whole number",
        CKind::Decimal => "decimal",
        CKind::YesNo => "yes or no",
        CKind::Letter => "letter",
        CKind::Text => "text",
        CKind::Bytes => "bytes",
        CKind::Record(_) => "record",
    }
}
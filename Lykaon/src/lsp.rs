//! M5 (Appendix L 9.1): `lykaon lsp`.
//!
//! A Language Server Protocol 3.17 server over standard input/output.
//! This increment provides:
//!   - `initialize` / `shutdown` / `exit`
//!   - full-text document sync (didOpen/didChange/didClose)
//!   - diagnostics (pushed after each change) with the Where line
//!   - hover (the word at the cursor)
//!   - completion (the language's word list)
//!   - go to definition (a `To NAME` or `NAME is` line)
//!   - document formatting (the `lykaon fmt` rules)
//!
//! The server is additive: it changes no language rule.

use std::collections::HashMap;
use std::io::{Read, Write};

use serde_json::{json, Value};

fn read_message<R: Read>(r: &mut R) -> Option<Vec<u8>> {
    // LSP headers are ASCII, terminated by a blank line (\r\n\r\n).
    let mut headers = Vec::new();
    let mut b = [0u8; 1];
    loop {
        if r.read(&mut b).ok()? == 0 {
            return None;
        }
        headers.push(b[0]);
        if headers.ends_with(b"\r\n\r\n") {
            break;
        }
        if headers.len() > 64 * 1024 {
            return None;
        }
    }
    let text = String::from_utf8_lossy(&headers);
    let len = text
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())?;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    Some(body)
}

fn write_message(body: &Value) {
    let text = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n", text.len());
    let _ = out.write_all(&text);
    let _ = out.flush();
}

fn respond(id: Option<Value>, result: Value) {
    let Some(id) = id else { return };
    write_message(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
}

fn notify(method: &str, params: Value) {
    write_message(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
}

fn parse_diagnostics(text: &str) -> Vec<Value> {
    let mut blocks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    let mut out = Vec::new();
    for b in blocks {
        let message = b.lines().next().unwrap_or("").trim().to_string();
        let line_no = b
            .lines()
            .find_map(|l| l.strip_prefix("Where:"))
            .and_then(|l| l.split(", line ").nth(1))
            .and_then(|s| s.split(':').next())
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(1);
            let line = line_no.saturating_sub(1);
        out.push(json!({
            "range": {
                "start": {"line": line, "character": 0},
                "end": {"line": line, "character": 1}
            },
            "severity": 1,
            "message": message
        }));
    }
    out
}

fn publish(uri: &str, text: &str) {
    let (diags, _ok) = crate::check_source(text, uri);
    let diagnostics = parse_diagnostics(&diags);
    notify(
        "textDocument/publishDiagnostics",
        json!({"uri": uri, "diagnostics": diagnostics}),
    );
}

fn word_at(text: &str, line: usize, character: usize) -> Option<String> {
    let line = text.lines().nth(line)?;
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let mut start = character.min(chars.len() - 1);
    let mut end = start;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    if start == end {
        None
    } else {
        Some(chars[start..end].iter().collect())
    }
}

/// Document symbols: `To NAME` (functions) and `NAME is` (values).
fn doc_symbols(text: &str) -> Value {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        let low = t.to_ascii_lowercase();
        let words: Vec<&str> = t.split_whitespace().collect();
        let mut name = String::new();
        let mut kind = 0;
        if let Some(rest) = low.strip_prefix("to ") {
            if let Some(w) = rest.split_whitespace().next() {
                name = w.to_string();
                kind = 12; // Function
            }
        } else if low.starts_with("changeable ") || low.starts_with("constant ") {
            // "Changeable NAME is ..." -> the name is the word after the keyword
            if words.len() >= 2 {
                name = words[1].trim_matches('.').to_string();
                kind = 13; // Variable
            }
        } else if let Some(pos) = low.find(" is ") {
            // "NAME is ..." -> the word(s) before " is "
            let before = t[..pos].trim();
            if let Some(w) = before.split_whitespace().last() {
                name = w.trim_matches('.').to_string();
                kind = 13; // Variable
            }
        }
        if !name.is_empty() {
            out.push(json!({
                "name": name,
                "kind": kind,
                "location": {"uri": "", "range": {
                    "start": {"line": i, "character": 0},
                    "end": {"line": i, "character": 1}
                }}
            }));
        }
    }
    json!(out)
}

/// All whole-word occurrences of `word` as LSP ranges.
fn find_all(text: &str, word: &str) -> Value {
    let needle = word.to_ascii_lowercase();
    let mut out = Vec::new();
    if needle.is_empty() {
        return json!(out);
    }
    for (i, line) in text.lines().enumerate() {
        let low = line.to_ascii_lowercase();
        let bytes = line.as_bytes();
        let mut start = 0usize;
        while let Some(pos) = low[start..].find(&needle) {
            let abs = start + pos;
            let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
            let after = abs + needle.len();
            let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
            if before_ok && after_ok {
                out.push(json!({"range": {
                    "start": {"line": i, "character": abs},
                    "end": {"line": i, "character": after}
                }}));
            }
            start = abs + needle.len();
        }
    }
    json!(out)
}

const KEYWORDS: &[&str] = &[
    "say", "ask", "set", "if", "otherwise", "otherwise if", "repeat", "for", "each", "in",
    "stop", "skip", "to", "give", "give back", "lend", "copy of", "share", "weak share", "constant",
    "changeable", "note", "about", "example", "test", "check", "pretend", "try", "fail with",
    "pass failure up", "look at", "when", "use", "close", "start task", "wait for", "send",
    "receive", "do these together", "do these all or none", "sleep for", "after", "every",
    "requires", "ensures", "always", "is", "are", "not", "and", "or", "plus", "minus", "times",
    "divided by", "more than", "less than", "with", "from", "of", "as", "yes", "no", "missing",
];

/// Run the LSP loop.
pub fn run() {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut docs: HashMap<String, String> = HashMap::new();
    loop {
        let Some(body) = read_message(&mut reader) else {
            break;
        };
        let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
            continue;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                respond(
                    id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 1,
                            "hoverProvider": true,
                            "completionProvider": {"resolveProvider": false},
                            "definitionProvider": true,
                            "documentSymbolProvider": true,
                            "referencesProvider": true,
                            "renameProvider": true,
                            "documentFormattingProvider": true
                        },
                        "serverInfo": {"name": "lykaon-lsp", "version": "0.1.0"}
                    }),
                );
            }
            "shutdown" => respond(id, json!(null)),
            "exit" => break,
            "textDocument/didOpen" | "textDocument/didChange" => {
                let uri = params
                    .pointer("/textDocument/uri")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();
                let text = if method == "textDocument/didOpen" {
                    params
                        .pointer("/textDocument/text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    params
                        .pointer("/contentChanges/0/text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                docs.insert(uri.clone(), text.clone());
                publish(&uri, &text);
            }
            "textDocument/didClose" => {
                let uri = params
                    .pointer("/textDocument/uri")
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                docs.remove(uri);
                notify(
                    "textDocument/publishDiagnostics",
                    json!({"uri": uri, "diagnostics": []}),
                );
            }
            "textDocument/hover" => {
                let uri = params.pointer("/textDocument/uri").and_then(|u| u.as_str()).unwrap_or("");
                let line = params.pointer("/position/line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let character = params.pointer("/position/character").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let text = docs.get(uri).cloned().unwrap_or_default();
                let word = word_at(&text, line, character).unwrap_or_default();
                respond(id, json!({"contents": {"kind": "markdown", "value": format!("**{}**", word)}}));
            }
            "textDocument/completion" => {
                let items: Vec<Value> = KEYWORDS
                    .iter()
                    .map(|k| json!({"label": k, "kind": 14}))
                    .collect();
                respond(id, json!(items));
            }
            "textDocument/definition" => {
                let uri = params.pointer("/textDocument/uri").and_then(|u| u.as_str()).unwrap_or("");
                let line = params.pointer("/position/line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let character = params.pointer("/position/character").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let text = docs.get(uri).cloned().unwrap_or_default();
                let word = word_at(&text, line, character).unwrap_or_default();
                let needle = word.to_ascii_lowercase();
                let found = text.lines().enumerate().find(|(i, l)| {
                    let l = l.trim().to_ascii_lowercase();
                    (l.starts_with("to ") && l.contains(&needle))
                        || (l.contains(" is ") && l.starts_with(&needle))
                });
                match found {
                    Some((i, _)) => respond(id, json!([{
                        "uri": uri,
                        "range": {"start": {"line": i, "character": 0}, "end": {"line": i, "character": 1}}
                    }])),
                    None => respond(id, json!([])),
                }
            }
            "textDocument/formatting" => {
                let uri = params.pointer("/textDocument/uri").and_then(|u| u.as_str()).unwrap_or("");
                let text = docs.get(uri).cloned().unwrap_or_default();
                // Refuse to format a file whose indentation tree is broken:
                // returning no edits is safer than flattening it.
                match crate::try_format_source(&text, uri) {
                    Ok(formatted) => {
                        let lines = formatted.lines().count().max(1);
                        respond(id, json!([{
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end": {"line": lines, "character": 0}
                            },
                            "newText": formatted
                        }]));
                    }
                    Err(_) => respond(id, json!([])),
                }
            }
            "textDocument/documentSymbol" => {
                let uri = params.pointer("/textDocument/uri").and_then(|u| u.as_str()).unwrap_or("");
                let text = docs.get(uri).cloned().unwrap_or_default();
                let syms = doc_symbols(&text);
                respond(id, syms);
            }
            "textDocument/references" => {
                let uri = params.pointer("/textDocument/uri").and_then(|u| u.as_str()).unwrap_or("");
                let line = params.pointer("/position/line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let character = params.pointer("/position/character").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let text = docs.get(uri).cloned().unwrap_or_default();
                let word = word_at(&text, line, character).unwrap_or_default();
                let mut locs = find_all(&text, &word);
                if let Some(arr) = locs.as_array_mut() {
                    for l in arr.iter_mut() {
                        l["uri"] = json!(uri);
                    }
                }
                respond(id, locs);
            }
            "textDocument/rename" => {
                let uri = params.pointer("/textDocument/uri").and_then(|u| u.as_str()).unwrap_or("");
                let line = params.pointer("/position/line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let character = params.pointer("/position/character").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let new_name = params.pointer("/newName").and_then(|v| v.as_str()).unwrap_or("");
                let text = docs.get(uri).cloned().unwrap_or_default();
                let word = word_at(&text, line, character).unwrap_or_default();
                let locs = find_all(&text, &word);
                let mut edits = Vec::new();
                if let Some(arr) = locs.as_array() {
                    for l in arr {
                        edits.push(json!({"range": l["range"], "newText": new_name}));
                    }
                }
                respond(id, json!({"changes": {uri: edits}}));
            }
            _ => {
                if let Some(id) = id {
                    respond(Some(id), json!(null));
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diagnostics_finds_the_line() {
        let text = "Error: x is not defined. (T0205)\nWhere: t.lykaon, line 3: Set x to 1.\n  ^^^\nWhy: x is used where it is not defined.\nFix: Define it first.\n";
        let d = parse_diagnostics(text);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0]["range"]["start"]["line"], 2);
        assert!(d[0]["message"].as_str().unwrap().contains("T0205"));
    }

    #[test]
    fn word_at_finds_the_word_under_the_cursor() {
        let text = "Say hello world.";
        assert_eq!(word_at(text, 0, 5).as_deref(), Some("hello"));
        assert_eq!(word_at(text, 0, 0).as_deref(), Some("Say"));
        assert_eq!(word_at(text, 0, 99).as_deref(), Some("world"));
    }
}

//! M5 (Appendix L 9.3): `lykaon doc`.
//!
//! Reads every `About` sentence and the `Example` blocks under it and writes
//! a folder of linked HTML pages: one page per function with its signature,
//! the About text and each Example's code. The command is additive; it never
//! changes the language.
//!
//! This increment writes the signature, the About text and the example code.
//! Running each example and embedding its output (the book's "obtained by
//! running it") is the next increment; until then the page says the example
//! is part of `lykaon test`.

use std::path::{Path, PathBuf};

use lykaon_core::sem::{FuncDef, Program};

#[derive(Clone, Debug, Default)]
struct Example {
    heading: String,
    code: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct About {
    text: String,
    examples: Vec<Example>,
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Collect the About/Example blocks in source order. An About starts at a
/// top-level `About ...` line; indented `Example ...` lines under it are its
/// examples. The next top-level `To ...` line owns the About.
fn scan_about(lines: &[&str]) -> Vec<(usize, About)> {
    let mut out: Vec<(usize, About)> = Vec::new();
    let mut current: Option<(usize, About)> = None;
    let mut example: Option<Example> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let indent = indent_of(line);
        if indent == 0 && lower.starts_with("about ") {
            if let Some(prev) = current.take() {
                out.push(prev);
            }
            let text = trimmed["About ".len()..].trim().to_string();
            current = Some((
                i,
                About {
                    text,
                    examples: Vec::new(),
                },
            ));
            example = None;
            continue;
        }
        if indent == 0 && lower.starts_with("to ") {
            if let Some(mut prev) = current.take() {
                if let Some(ex) = example.take() {
                    prev.1.examples.push(ex);
                }
                out.push(prev);
            }
            example = None;
            continue;
        }
        if indent > 0 && lower.starts_with("example ") {
            if let Some((_, about)) = current.as_mut() {
                if let Some(ex) = example.take() {
                    about.examples.push(ex);
                }
                example = Some(Example {
                    heading: trimmed["Example ".len()..].trim().to_string(),
                    code: Vec::new(),
                });
            }
            continue;
        }
        if let Some(ex) = example.as_mut() {
            if indent > 0 {
                ex.code.push(line.to_string());
            }
        }
    }
    if let Some(mut prev) = current.take() {
        if let Some(ex) = example.take() {
            prev.1.examples.push(ex);
        }
        out.push(prev);
    }
    out
}

fn signature(f: &FuncDef) -> String {
    let mut s = format!("To {}", f.name);
    for (name, _) in &f.inputs {
        s.push(' ');
        s.push_str(name);
    }
    if f.can_fail {
        s.push_str(" can fail");
    }
    s.push('.');
    s
}

fn page_for(func: &FuncDef, about: &About) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;max-width:52rem;margin:2rem auto;padding:0 1rem}}\
         pre{{background:#f5f5f5;padding:1rem;overflow:auto}}code{{font-family:monospace}}\
         .sig{{font-family:monospace;font-size:1.1rem}}</style></head><body>",
        escape(&func.name)
    ));
    body.push_str(&format!(
        "<p class=\"sig\">{}</p>",
        escape(&signature(func))
    ));
    if !about.text.is_empty() {
        body.push_str(&format!("<p>{}</p>", escape(&about.text)));
    }
    for ex in &about.examples {
        body.push_str("<h3>Example</h3>");
        if !ex.heading.is_empty() {
            body.push_str(&format!("<p>{}</p>", escape(&ex.heading)));
        }
        let code: Vec<String> = ex
            .code
            .iter()
            .map(|l| l.trim_start_matches("    "))
            .map(|l| l.to_string())
            .collect();
        body.push_str(&format!("<pre><code>{}</code></pre>", escape(&code.join("\n"))));
    }
    body.push_str("<p><a href=\"index.html\">Index</a></p></body></html>");
    body
}

/// Generate the documentation tree. Returns the number of pages written.
pub fn generate(root: &Path, out_dir: &Path) -> Result<usize, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect(root, &mut files);
    files.sort();
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mut index = String::new();
    index.push_str("<!doctype html><html><head><meta charset=\"utf-8\"><title>Lykaon documentation</title>\
        <style>body{font-family:system-ui,sans-serif;max-width:52rem;margin:2rem auto;padding:0 1rem}\
        code{font-family:monospace}</style></head><body><h1>Lykaon documentation</h1>");
    let mut pages = 0usize;
    for path in &files {
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let prog: Program = crate::compile_tree(&source, &path.to_string_lossy())
            .map_err(|diags| diags.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))?;
        let lines: Vec<&str> = source.lines().collect();
        let abouts = scan_about(&lines);
        let module = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");
        // the top-level `To ...` lines, mapped to the functions in order
        let to_lines: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim().to_ascii_lowercase().starts_with("to "))
            .map(|(i, _)| i)
            .collect();
        index.push_str(&format!("<h2>{}</h2><ul>", escape(&path.to_string_lossy())));
        for (about_line, about) in &abouts {
            let Some(&to_line) = to_lines.iter().find(|&&l| l > *about_line) else {
                continue;
            };
            let func_index = to_lines.iter().position(|&l| l == to_line).unwrap_or(0);
            let Some(func) = prog.funcs.get(func_index) else {
                continue;
            };
            let file_name = format!("{module}-{}.html", slug(&func.name));
            let html = page_for(func, about);
            std::fs::write(out_dir.join(&file_name), html).map_err(|e| e.to_string())?;
            index.push_str(&format!(
                "<li><a href=\"{file_name}\">{}</a> <code>{}</code></li>",
                escape(&func.name),
                escape(&signature(func))
            ));
            pages += 1;
        }
        index.push_str("</ul>");
    }
    index.push_str("</body></html>");
    std::fs::write(out_dir.join("index.html"), index).map_err(|e| e.to_string())?;
    Ok(pages)
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == ".git" || name == "build" || name == "output" {
                continue;
            }
            collect(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("lykaon") {
            files.push(path);
        }
    }
}
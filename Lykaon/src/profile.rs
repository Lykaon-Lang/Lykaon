//! M5 (Appendix L 9.5): `lykaon profile`.
//!
//! Runs the program with sentence counters and prints the hottest sentences.
//! The book asks for time shares and file/line; this increment counts
//! executions and uses the typed-tree debug form as the row label (logged).
//! `lykaon profile memory` also prints the peak live bytes.

pub fn run(file: &str, memory: bool) -> Result<String, String> {
    let source = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    lykaon_core::interp::enable_profile();
    let (out, _ranges, _code) = crate::run_source_tool_streams(&source, file, "", &[], true);
    lykaon_core::interp::disable_profile();
    let mut rows = lykaon_core::interp::take_profile();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(20);
    let mut text = out;
    if memory {
        text.push_str(&format!("\npeak memory: {} bytes\n", lykaon_core::alloc::peak()));
    }
    text.push_str("\nsentences by count:\n");
    for (sentence, count) in rows {
        text.push_str(&format!("{count:>8}  {sentence}\n"));
    }
    Ok(text)
}
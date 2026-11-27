use lykaon::{NativeFail, try_format_source, native_run_tool, packages};
use std::io::IsTerminal;
use std::{env, path::{Path, PathBuf}, process::ExitCode};

// M5-S45: count allocations in the compiler front too, so lykaon profile
// memory can report a peak.
#[global_allocator]
static GLOBAL_ALLOC: lykaon::alloc::CountingAllocator = lykaon::alloc::CountingAllocator;


fn run_big(source: String, file: String, stdin_text: String) -> (String, i32) {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || lykaon::run_source(&source, &file, &stdin_text))
        .map_err(|e| e.to_string())
        .and_then(|h| h.join().map_err(|_| "worker panicked".to_string()))
        .unwrap_or((String::new(), 1))
}

fn run_big_metered(
    source: String,
    file: String,
    stdin_text: String,
    fuel: u64,
) -> lykaon::RunOutcome {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || lykaon::run_source_budgeted_metered(&source, &file, &stdin_text, fuel))
        .map_err(|e| e.to_string())
        .and_then(|h| h.join().map_err(|_| "worker panicked".to_string()))
        .unwrap_or_else(|e| lykaon::RunOutcome {
            output: format!("Error: {e}\n"),
            exit: 1,
            reason: lykaon::interp::ExitReason::Failure,
            fuel_used: 0,
            peak_alloc_bytes: 0,
            permissions_used: Vec::new(),
        })
}

/// M5g item d4: run N top-level sentences on a large-stack worker.
fn run_big_until(
    source: String,
    file: String,
    stdin_text: String,
    mode: lykaon::TraceMode,
    stop_after: usize,
) -> (String, i32, Option<String>) {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || lykaon::snapshot::until(&source, &file, &stdin_text, mode, stop_after))
        .map_err(|e| e.to_string())
        .and_then(|h| h.join().map_err(|_| "worker panicked".to_string()))
        .unwrap_or_else(|e| (String::new(), 1, Some(format!("Error: {e}"))))
}

/// M5g item c: run on a large-stack worker and bring the trace back.
fn run_big_traced(
    source: String,
    file: String,
    stdin_text: String,
    mode: lykaon::TraceMode,
    deterministic: bool,
    seed: u64,
    fuel: u64,
) -> lykaon::TracedRun {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || {
            lykaon::run_source_traced(&source, &file, &stdin_text, fuel, mode, deterministic, seed)
        })
        .map_err(|e| e.to_string())
        .and_then(|h| h.join().map_err(|_| "worker panicked".to_string()))
        .unwrap_or_else(|e| lykaon::TracedRun {
            outcome: lykaon::RunOutcome {
                output: format!("Error: {e}\n"),
                exit: 1,
                reason: lykaon::interp::ExitReason::Failure,
                fuel_used: 0,
                peak_alloc_bytes: 0,
                permissions_used: Vec::new(),
            },
            events: Vec::new(),
            replay_left: 0,
        })
}

fn main() -> ExitCode {
    // a double click on the exe opens a console that would close before
    // the usage text can be read: when stdout is a real console window,
    // wait for Enter before closing (piped and automated runs are
    // unaffected - they are not terminals)
    let interactive_console = std::io::stdout().is_terminal();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("{info}");
        if std::io::stderr().is_terminal() {
            eprintln!("\nPress Enter to close...");
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
        }
    }));
    let mut args: Vec<String> = env::args().collect();
    // M5-S43: lykaon run safely [file] (or --safely) enables the sandbox.
    let mut safely = false;
    if args.get(2).map(|s| s == "safely" || s == "--safely").unwrap_or(false) {
        safely = true;
        args.remove(2);
    }
    if safely {
        unsafe { std::env::set_var("LYKAON_SAFE", "1"); }
    }
    if args.len() < 2 {
        eprintln!("usage: lykaonc-cli <command> [args]");
        eprintln!("  run file.lykaon | check file.lykaon | fmt file.lykaon | test [file]");
        eprintln!("  run-many file.lykaon [--runs N] [--warmup N] [--threads N]");
        eprintln!("  build file.lykaon | repl | bench | report");
        eprintln!("  lsp | debug file.lykaon | doc | outline | explain | fix");
        eprintln!("  migrate [status] | setup | serve | sbom");
        eprintln!("  checkpoint file.lykaon | resume file.lykaon");
        eprintln!("  add <folder-or-url> | remove <name> | update");
        if interactive_console {
            // a double click opens a console: write and test code right
            // here instead of closing
            return lykaon::cli::repl_session();
        }
        return ExitCode::from(2);
    }
    match args[1].as_str() {
        "check" => {
            if args.len() < 3 {
                eprintln!("usage: lykaon check <file>");
                return ExitCode::from(2);
            }
            let file = &args[2];
            // read bytes, not text: a file that is not UTF-8 is T0018
            let bytes = match std::fs::read(file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            let (out, ok) = lykaon::check_bytes(&bytes, file);
            let color = args.iter().any(|a| a == "--color");
            let suggest = args.iter().any(|a| a == "--suggest");
            if args.iter().any(|a| a == "--json") {
                let source = String::from_utf8_lossy(&bytes).to_string();
                let diags = lykaon::check_diagnostics(&source, file);
                print!("{}", lykaon::diag_json::to_json(&diags));
            } else if suggest || color {
                // M5c: verbose text mode shows suggestions, secondary labels
                // and Note/Help; --color adds ANSI colours.
                let source = String::from_utf8_lossy(&bytes).to_string();
                let diags = lykaon::check_diagnostics(&source, file);
                for d in &diags {
                    print!("{}", d.render_ex(color, suggest));
                    if !d.what.is_empty() {
                        println!();
                    }
                }
            } else {
                print!("{}", out);
            }
            if ok {
                ExitCode::from(0)
            } else {
                ExitCode::from(1)
            }
        }
        "doc" => {
            // M5 (Appendix L 9.3): lykaon doc [outdir].
            let out = args.get(2).cloned().unwrap_or_else(|| "doc".to_string());
            match lykaon::docgen::generate(std::path::Path::new("."), std::path::Path::new(&out)) {
                Ok(n) => {
                    println!("{n} documentation pages in {out}/");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "lsp" => {
            // M5 (Appendix L 9.1): the language server over stdio.
            lykaon::lsp::run();
            ExitCode::from(0)
        }
        "outline" => {
            // M5c item 3: lykaon outline [file] [--json].
            let json = args.iter().any(|a| a == "--json");
            let file = args.get(2).cloned().unwrap_or_else(|| "main.lykaon".to_string());
            match lykaon::outline::run(&file, json) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "docs" => {
            // M5c item 7: lykaon docs --compact (the cheat sheet).
            print!("{}", lykaon::m5c::cheat_sheet());
            ExitCode::from(0)
        }
        "grammar" => {
            // M5c item 4: lykaon grammar [--json|--export]
            if args.iter().any(|a| a == "--json") {
                print!("{}", lykaon::grammar::export_json());
            } else {
                print!("{}", lykaon::grammar::export_ebnf());
            }
            ExitCode::from(0)
        }
        "explain" => {
            // M5c item 5: lykaon explain <code>.
            let code = args.get(2).cloned().unwrap_or_default();
            println!("{}", lykaon::m5c::explain(&code));
            ExitCode::from(0)
        }
        "profile" => {
            // M5 (Appendix L 9.5): lykaon profile [memory] [file]
            let memory = args.iter().any(|a| a == "memory");
            let file = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with("--") && a.as_str() != "memory")
                .cloned()
                .unwrap_or_else(|| "main.lykaon".to_string());
            match lykaon::profile::run(&file, memory) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "fix" => {
            // M5c item 6: apply unambiguous suggestions (did-you-mean).
            let apply = args.iter().any(|a| a == "--apply");
            let file = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with("--"))
                .cloned()
                .unwrap_or_else(|| "main.lykaon".to_string());
            match lykaon::fix::run(&file, apply) {
                Ok(text) => { print!("{text}"); ExitCode::from(0) }
                Err(e) => { eprintln!("Error: {e}"); ExitCode::from(1) }
            }
        }
        "debug" => {
            // Step 9: lykaon debug [--dap] <file>
            let file = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with("--"))
                .cloned()
                .unwrap_or_else(|| "main.lykaon".to_string());
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            // DAP over stdio (the editor drives it).
            lykaon::dap::serve(source, file);
            ExitCode::from(0)
        }
        "layout" => {
            // M5b: lykaon layout [file.csv] - choose a data layout and time it.
            let file = args.iter().skip(2).find(|a| !a.starts_with("--")).cloned();
            let table = if let Some(f) = file {
                match std::fs::read_to_string(&f) {
                    Ok(text) => match lykaon_core::data::load_csv_text(&text) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("Error: cannot load {f}: {}", e.message());
                            return ExitCode::from(1);
                        }
                    },
                    Err(e) => {
                        eprintln!("Error: cannot read {f}: {e}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                lykaon_core::layout::synthetic(1_000_000)
            };
            let report = lykaon_core::layout::choose(&table);
            println!("{}", report.describe());
            let col = 0usize;
            let (row_ms, soa_ms) = lykaon_core::layout::benchmark(&table, col);
            let speedup = if soa_ms > 0.0 { row_ms / soa_ms } else { 0.0 };
            println!(
                "sum column 0: row-major {:.2} ms, SoA+SIMD {:.2} ms ({:.2}x)",
                row_ms, soa_ms, speedup
            );
            let a = lykaon_core::layout::sum_column_rowmajor(&table, col);
            let b = lykaon_core::layout::sum_column_soa(&table, col);
            println!(
                "checksum equal: {} ({:.3})",
                (a - b).abs() < 1e-6,
                a
            );
            ExitCode::from(0)
        }
        "audit" => {
            // M5f: lykaon audit [file] [--json|--sarif|--emit-tests]
            let file = args.iter().skip(2).find(|a| !a.starts_with("--")).cloned()
                .unwrap_or_else(|| "main.lykaon".to_string());
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => { eprintln!("Error: cannot read {file}: {e}"); return ExitCode::from(1); }
            };
            let prog = match lykaon::compile_tree(&source, &file) {
                Ok(p) => p,
                Err(diags) => {
                    eprintln!("{}", diags.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"));
                    return ExitCode::from(1);
                }
            };
            let mut audit = lykaon::audit::audit(&prog, &source, &file);
            // M5f item 7 (intent) and item 8 (dependencies).
            let cwd = std::env::current_dir().unwrap_or_default();
            audit.findings.extend(lykaon::audit::check_dependencies(&cwd, &file));
            lykaon::audit::check_intent(&source, &file, &mut audit.findings);
            // M5f: suppressions (lykaon-audit.toml), baseline, strict.
            if let Ok(text) = std::fs::read_to_string("lykaon-audit.toml") {
                audit.suppress(&lykaon::audit::parse_suppressions(&text));
            }
            if let Some(base) = args.iter().position(|a| a == "--baseline").and_then(|i| args.get(i + 1)) {
                let keys: std::collections::HashSet<String> = std::fs::read_to_string(base)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                if keys.is_empty() && !std::path::Path::new(base).exists() {
                    let _ = std::fs::write(base, serde_json::to_string_pretty(&audit.keys()).unwrap_or_else(|_| "[]".into()));
                    println!("wrote baseline {base}");
                } else {
                    audit.only_new(&keys);
                }
            }
            // M5f: agent mode (optional; API key from the environment).
            if args.iter().any(|a| a == "--agent") {
                let model = std::env::var("LYKAON_AGENT_MODEL")
                    .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());
                return match lykaon::agent::review(&file, &source, &audit.to_json(), &model) {
                    Ok(text) => { println!("{text}"); ExitCode::from(0) }
                    Err(e) => { eprintln!("agent: {e}"); ExitCode::from(1) }
                };
            }
            // M5f: level (quick default, normal, deep). deep adds mutation.
            let deep = args.iter().any(|a| a == "--mutate")
                || args
                    .iter()
                    .position(|a| a == "--level")
                    .and_then(|i| args.get(i + 1))
                    .map(|v| v == "deep")
                    .unwrap_or(false);
            // M5f: mutation testing (deep).
            if deep {
                let m = lykaon::mutation::run(&source, &file);
                println!("mutation: {} mutants, {} caught, {} survived", m.total, m.caught, m.survived.len());
                if !m.has_tests { println!("no Test blocks: mutation testing needs tests"); }
                for s in &m.survived { println!("  survived: {s}"); }
                return ExitCode::from(0);
            }
            if args.iter().any(|a| a == "--json") {
                println!("{}", audit.to_json());
            } else if args.iter().any(|a| a == "--sarif") {
                println!("{}", audit.to_sarif());
            } else {
                print!("{}", audit.render());
                println!();
            }
            if args.iter().any(|a| a == "--emit-tests") {
                let dir = std::path::Path::new("audit_regression");
                let _ = std::fs::create_dir_all(dir);
                for (i, f) in audit.findings.iter().enumerate() {
                    let p = dir.join(format!("{:02}-{}-{}.lykaon", i, f.rule, f.line));
                    let _ = std::fs::write(&p, &source);
                }
                println!("wrote {} regression program(s) to audit_regression/", audit.findings.len());
            }
            if args.iter().any(|a| a == "--strict") && audit.has_high_confidence_error() {
                eprintln!("strict: high-confidence Error finding(s) present");
                return ExitCode::from(1);
            }
            ExitCode::from(0)
        }
        "review" => {
            // M5f: lykaon review --diff <old> <new>
            let mut old = None;
            let mut new = None;
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--diff" {
                    old = args.get(i + 1).cloned();
                    new = args.get(i + 2).cloned();
                    break;
                }
                i += 1;
            }
            let (Some(old), Some(new)) = (old, new) else {
                eprintln!("usage: lykaon review --diff <old.lykaon> <new.lykaon>");
                return ExitCode::from(2);
            };
            let read = |f: &str| -> Result<String, ExitCode> {
                std::fs::read_to_string(f).map_err(|e| { eprintln!("Error: cannot read {f}: {e}"); ExitCode::from(1) })
            };
            let (os, ns) = match (read(&old), read(&new)) { (Ok(a), Ok(b)) => (a, b), _ => return ExitCode::from(1) };
            let op = lykaon::compile_tree(&os, &old).ok();
            let np = lykaon::compile_tree(&ns, &new).ok();
            match (op, np) {
                (Some(op), Some(np)) => {
                    let mut findings = lykaon::audit::review_diff(&op, &np, &new);
                    findings.extend(lykaon::audit::diff_behavior(&os, &ns, &op, &np, &new));
                    for f in &findings {
                        println!("{}: {} ({})", f.severity.word(), f.rule, f.why);
                    }
                    if findings.is_empty() { println!("no interface, permission or behaviour changes"); }
                }
                _ => { eprintln!("Error: one of the files does not compile"); return ExitCode::from(1); }
            }
            ExitCode::from(0)
        }
        "report" => {
            // M5g item f: lykaon report <file> [--fuel N] [--output <file>]
            // Resource report v1 as JSON. The report command itself exits 0
            // when it can produce the document; `exit` in the document is the
            // program's result.
            let mut file: Option<String> = None;
            let mut fuel = 0u64;
            let mut output: Option<String> = None;
            let mut input = String::new();
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--input" => {
                        input = args.get(i + 1).cloned().unwrap_or_default();
                        i += 2;
                    }
                    "--fuel" => {
                        if let Some(v) = args.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                            fuel = v;
                        }
                        i += 2;
                    }
                    "--output" => {
                        output = args.get(i + 1).cloned();
                        i += 2;
                    }
                    other if other.starts_with("--") => i += 1,
                    other => {
                        file = Some(other.to_string());
                        i += 1;
                    }
                }
            }
            let file = file.unwrap_or_else(|| "main.lykaon".to_string());
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => { eprintln!("Error: cannot read {file}: {e}"); return ExitCode::from(1); }
            };
            let start = std::time::Instant::now();
            let outcome = run_big_metered(source, file.clone(), input.clone(), fuel);
            let ms = start.elapsed().as_millis();
            let doc = serde_json::json!({
                "schema": "lykaon-report-v1",
                "file": file,
                "exit": outcome.exit,
                "exit_reason": outcome.reason.as_str(),
                "fuel": fuel,
                "fuel_used": outcome.fuel_used,
                "wall_ms": ms,
                "output_bytes": outcome.output.len(),
                "peak_alloc_bytes": outcome.peak_alloc_bytes,
                "input_bytes": input.len(),
                "permissions_used": outcome.permissions_used,
            });
            let text = serde_json::to_string_pretty(&doc)
                .unwrap_or_else(|_| "{\"schema\":\"lykaon-report-v1\"}".to_string());
            match output {
                Some(path) => match std::fs::write(&path, text + "\n") {
                    Ok(()) => {
                        eprintln!("Wrote {path}");
                        ExitCode::from(0)
                    }
                    Err(e) => {
                        eprintln!("Error: cannot write {path}: {e}");
                        ExitCode::from(1)
                    }
                },
                None => {
                    println!("{text}");
                    ExitCode::from(0)
                }
            }
        }
        "setup" => {
            // M5g/IDE: register the .lykaon file type + icon (Windows Explorer).
            let icon = args
                .iter()
                .position(|a| a == "--icon")
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_else(|| {
                    let mut p = std::env::current_exe().unwrap_or_default();
                    p.pop();
                    p.set_file_name("lykaon.ico");
                    p.to_string_lossy().to_string()
                });
            let mut opener = std::env::current_exe().unwrap_or_default();
            opener.pop();
            opener.set_file_name(if cfg!(windows) { "lykaon-cli.exe" } else { "lykaon-cli" });
            let opener = if opener.exists() {
                opener.to_string_lossy().to_string()
            } else {
                std::env::current_exe()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            };
            match lykaon::setup::register(&icon, &opener) {
                Ok(msg) => { println!("{msg}"); ExitCode::from(0) }
                Err(e) => { eprintln!("Error: {e}"); ExitCode::from(1) }
            }
        }
        "serve" => {
            // M5g item g: lykaon serve (JSON-RPC over stdio, one object per line).
            // Sandbox knobs are additive and default to unlimited.
            let mut fuel = 0u64;
            let mut max_request_bytes = 1024 * 1024usize;
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--fuel" => {
                        if let Some(v) = args.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                            fuel = v;
                        }
                        i += 2;
                    }
                    "--max-request-bytes" => {
                        if let Some(v) = args.get(i + 1).and_then(|v| v.parse::<usize>().ok()) {
                            max_request_bytes = v;
                        }
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            lykaon::serve::serve(lykaon::serve::Options { fuel, max_request_bytes });
            ExitCode::from(0)
        }
        "sbom" => {
            // M5g item h: lykaon sbom (CycloneDX JSON).
            let doc = lykaon::sbom::sbom();
            if let Some(p) = args.iter().position(|a| a == "--output") {
                let Some(path) = args.get(p + 1) else {
                    eprintln!("usage: lykaon sbom [--output <file>]");
                    return ExitCode::from(2);
                };
                match std::fs::write(path, &doc) {
                    Ok(()) => {
                        eprintln!("Wrote {path}");
                        ExitCode::from(0)
                    }
                    Err(e) => {
                        eprintln!("Error: cannot write {path}: {e}");
                        ExitCode::from(1)
                    }
                }
            } else {
                println!("{doc}");
                ExitCode::from(0)
            }
        }
        "migrate" => {
            // M5-S38: lykaon migrate [status] - run (or list) the SQL files
            // in migrations/ against the database named by settings.lykaon.
            let status = args.get(2).map(|s| s == "status").unwrap_or(false);
            match lykaon::migrate::run(std::path::Path::new("."), status) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "fmt" => {
            if args.len() < 3 {
                eprintln!("usage: lykaon fmt <file>");
                return ExitCode::from(2);
            }
            let file = &args[2];
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            match try_format_source(&source, file) {
                Ok(formatted) => {
                    print!("{}", formatted);
                    ExitCode::from(0)
                }
                Err(diags) => {
                    for d in &diags {
                        eprintln!("{d}");
                    }
                    ExitCode::from(1)
                }
            }
        }
        "lint" => {
            if args.len() < 3 {
                eprintln!("usage: lykaon lint <file>");
                return ExitCode::from(2);
            }
            let file = &args[2];
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            for d in lykaon::lint::lint_source(&source, file) {
                println!("{d}");
            }
            ExitCode::from(0)
        }
        "bindgen" => {
            if args.len() < 3 {
                eprintln!("usage: lykaon bindgen <header.h> [out.stub.shelf]");
                return ExitCode::from(2);
            }
            let input = &args[2];
            let source = match std::fs::read_to_string(input) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {input}: {e}");
                    return ExitCode::from(1);
                }
            };
            let text = lykaon::bindgen::bindgen(&source);
            match args.get(3) {
                Some(out) => {
                    if let Err(e) = std::fs::write(out, text) {
                        eprintln!("Error: cannot write {out}: {e}");
                        return ExitCode::from(1);
                    }
                }
                None => print!("{text}"),
            }
            ExitCode::from(0)
        }
        "registry" => {
            // lykaon registry status|publish <folder>|add <name> [dest] [--registry <dir>]
            let mut reg = PathBuf::from("registry");
            let mut rest: Vec<String> = Vec::new();
            let mut i = 2usize;
            while i < args.len() {
                if args[i] == "--registry" {
                    if let Some(p) = args.get(i + 1) {
                        reg = PathBuf::from(p);
                        i += 2;
                        continue;
                    }
                }
                rest.push(args[i].clone());
                i += 1;
            }
            let Some(sub) = rest.first().map(|s| s.as_str()) else {
                eprintln!("usage: lykaon registry status | publish <folder> | add <name> [dest]");
                return ExitCode::from(2);
            };
            match sub {
                "status" => {
                    print!("{}", lykaon::registry::status(&reg));
                    ExitCode::from(0)
                }
                "publish" => {
                    let Some(folder) = rest.get(1) else {
                        eprintln!("usage: lykaon registry publish <folder>");
                        return ExitCode::from(2);
                    };
                    match lykaon::registry::publish(Path::new(folder), &reg) {
                        Ok((name, version)) => {
                            println!("Published {name} {version} to {}.", reg.display());
                            ExitCode::from(0)
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            ExitCode::from(1)
                        }
                    }
                }
                "add" => {
                    let Some(name) = rest.get(1) else {
                        eprintln!("usage: lykaon registry add <name> [dest]");
                        return ExitCode::from(2);
                    };
                    let dest = rest.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
                    match lykaon::registry::add(name, &reg, &dest) {
                        Ok(p) => {
                            println!("Added {name} to {}.", p.display());
                            ExitCode::from(0)
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            ExitCode::from(1)
                        }
                    }
                }
                other => {
                    eprintln!("Error: unknown registry command {other}.");
                    ExitCode::from(2)
                }
            }
        }
        "checkpoint" => {
            // M5g item d: lykaon checkpoint <file> [--after N] [--out <file>]
            let mut file: Option<String> = None;
            let mut after = 0usize;
            let mut out_path: Option<String> = None;
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--after" => {
                        after = args.get(i + 1).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                        i += 2;
                    }
                    "--out" => {
                        out_path = args.get(i + 1).cloned();
                        i += 2;
                    }
                    a if a.starts_with("--") => i += 1,
                    a => {
                        file = Some(a.to_string());
                        i += 1;
                    }
                }
            }
            let file = file.unwrap_or_else(|| "main.lykaon".to_string());
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            let mut buf = String::new();
            use std::io::Read;
            let _ = std::io::stdin().read_to_string(&mut buf);
            let outcome = lykaon::snapshot::checkpoint(&source, &file, &buf, after);
            if let Some(e) = &outcome.error {
                eprintln!("Error: {e}");
            }
            print!("{}", outcome.output);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            if let Some(snap) = outcome.snapshot {
                let path = out_path.unwrap_or_else(|| format!("{file}.snap.json"));
                match lykaon::snapshot::write(std::path::Path::new(&path), &source, &snap) {
                    Ok(()) => eprintln!("Wrote {path}"),
                    Err(e) => {
                        eprintln!("Error: cannot write {path}: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            ExitCode::from(outcome.exit as u8)
        }
        "resume" => {
            // M5g item d: lykaon resume <file> --snapshot <snap> [--input <text>]
            let mut file: Option<String> = None;
            let mut snap_path: Option<String> = None;
            let mut input: Option<String> = None;
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--snapshot" => {
                        snap_path = args.get(i + 1).cloned();
                        i += 2;
                    }
                    "--input" => {
                        input = args.get(i + 1).cloned();
                        i += 2;
                    }
                    a if a.starts_with("--") => i += 1,
                    a => {
                        file = Some(a.to_string());
                        i += 1;
                    }
                }
            }
            let file = file.unwrap_or_else(|| "main.lykaon".to_string());
            let snap_path = match snap_path {
                Some(p) => p,
                None => {
                    eprintln!("usage: lykaon resume <file> --snapshot <file> [--input <text>]");
                    return ExitCode::from(2);
                }
            };
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            let snap = match lykaon::snapshot::read(std::path::Path::new(&snap_path), &source) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: {e}");
                    return ExitCode::from(1);
                }
            };
            let (out, code, err) = lykaon::snapshot::resume(&source, &file, &snap, input.as_deref());
            print!("{out}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            if let Some(e) = err {
                eprintln!("Error: {e}");
            }
            ExitCode::from(code as u8)
        }
        "run" => {
            // M4-D17 (Appendix K 7.11): from M4 the run tool runs native by
            // default; the interpreter is the explicit --interpret choice.
            // The lykaon flags belong before the file name; everything after
            // the file is the program's own command line (M4-S82).
            let mut interpret = false;
            let mut native_mode: Option<&'static str> = None;
            let mut trace_json = false;
            // M5g item b: --deterministic (virtual clock, fixed seed) and
            // --seed N (the seed that mode uses; default 42).
            let mut deterministic = false;
            let mut seed = 42u64;
            // M5g item a: --fuel N (a step budget; stops cleanly when exceeded).
            let mut fuel: Option<u64> = None;
            // M5g item d4: --stop-after N stops after N top-level sentences.
            let mut stop_after = 0usize;
            // M5g item e: --isolate runs the program in a child under OS
            // limits (Job Object on Windows, rlimits on Linux). The child
            // is always the interpreter, so the run path stays observable.
            let mut isolate = false;
            let mut limits = lykaon::isolate::Limits::default();
            let mut child_args: Vec<String> = Vec::new();
            if let Some(p) = args.iter().position(|a| a == "--stop-after") {
                stop_after = args.get(p + 1).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            }
            if let Some(p) = args.iter().position(|a| a == "--fuel") {
                fuel = args.get(p + 1).and_then(|v| v.parse::<u64>().ok());
            }
            // M5g item c: --record <file> / --replay <file> (stdin).
            let record: Option<String> = args.iter().position(|a| a == "--record").and_then(|p| args.get(p + 1).cloned());
            let replay: Option<String> = args.iter().position(|a| a == "--replay").and_then(|p| args.get(p + 1).cloned());
            let mut file: Option<String> = None;
            let mut prog_args: Vec<String> = Vec::new();
            let mut i = 2;
            while i < args.len() {
                let a = args[i].as_str();
                if file.is_none() {
                    match a {
                        "--interpret" => interpret = true,
                        "--native" => {}
                        "--fast" => native_mode = Some("fast"),
                        "--small" => native_mode = Some("small"),
                        // M5c: --trace-json runs on the interpreter (the native
                        // path has no sentence trace) and writes the trace to
                        // stderr as JSON; stdout stays the program's output.
                        "--trace-json" => {
                            trace_json = true;
                            interpret = true;
                        }
                        "--fuel" => {
                            if let Some(v) = args.get(i + 1).cloned() {
                                child_args.push("--fuel".to_string());
                                child_args.push(v);
                            }
                            i += 1; // skip the budget value
                        }
                        "--deterministic" => deterministic = true,
                        "--seed" => {
                            seed = args
                                .get(i + 1)
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(42);
                            if let Some(v) = args.get(i + 1).cloned() {
                                child_args.push("--seed".to_string());
                                child_args.push(v);
                            }
                            i += 1; // skip the seed value
                        }
                        "--record" | "--replay" => {
                            if let Some(v) = args.get(i + 1).cloned() {
                                child_args.push(a.to_string());
                                child_args.push(v);
                            }
                            i += 1; // skip the file value
                        }
                        "--stop-after" => {
                            if let Some(v) = args.get(i + 1).cloned() {
                                child_args.push("--stop-after".to_string());
                                child_args.push(v);
                            }
                            i += 1; // skip the count
                        }
                        // M5g item e: isolation flags belong to the parent.
                        "--isolate" => isolate = true,
                        "--memory-mb" => {
                            limits.memory_mb = args
                                .get(i + 1)
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(limits.memory_mb);
                            i += 1;
                        }
                        "--cpu-seconds" => {
                            limits.cpu_seconds = args
                                .get(i + 1)
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(limits.cpu_seconds);
                            i += 1;
                        }
                        "--max-output-bytes" => {
                            limits.max_output_bytes = args
                                .get(i + 1)
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(limits.max_output_bytes);
                            i += 1;
                        }
                        "--max-processes" => {
                            limits.max_processes = args
                                .get(i + 1)
                                .and_then(|v| v.parse::<u32>().ok())
                                .unwrap_or(limits.max_processes);
                            i += 1;
                        }
                        _ => file = Some(args[i].clone()),
                    }
                } else {
                    prog_args.push(args[i].clone());
                }
                i += 1;
            }
            // M5g item b: tell the interpreter (and a native child, which
            // inherits the environment) to use the virtual clock and seed.
            if deterministic {
                unsafe {
                    std::env::set_var("LYKAON_DETERMINISTIC", "1");
                    std::env::set_var("LYKAON_SEED", seed.to_string());
                }
            }
            // A lone "--" after the file is the conventional end-of-options
            // marker (`lykaon run app.lykaon -- arg`); drop it so it is not
            // passed to the program as an argument.
            if prog_args.first().map(|s| s == "--").unwrap_or(false) {
                prog_args.remove(0);
            }
            let Some(file) = file else {
                eprintln!(
                    "usage: lykaon run [--interpret | [--fast | --small]] [--deterministic] [--seed N] [--fuel N] [--stop-after N] [--isolate [--memory-mb N] [--cpu-seconds N] [--max-output-bytes N] [--max-processes N]] <file> [arguments]"
                );
                return ExitCode::from(2);
            };
            if isolate {
                // M5g item e: the child is this same executable on the
                // interpreter path. Isolation flags never reach it (they
                // belong to the parent), and the output cap is handed over
                // through LYKAON_OUTPUT_LIMIT.
                let mut argv = vec!["run".to_string(), "--interpret".to_string()];
                argv.extend(child_args);
                argv.push(file);
                argv.extend(prog_args);
                return ExitCode::from(lykaon::isolate::run(&argv, &limits).code as u8);
            }
            if trace_json {
                let source = match std::fs::read_to_string(&file) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: cannot read {file}: {e}");
                        return ExitCode::from(1);
                    }
                };
                let (out, trace, code) = lykaon::run_source_trace(&source, &file, "");
                print!("{out}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&trace).unwrap_or_else(|_| "[]".to_string())
                );
                return ExitCode::from(code as u8);
            }
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            if record.is_some() && stop_after > 0 {
                eprintln!("Error: --stop-after cannot be used with --record");
                return ExitCode::from(2);
            }
            if let Some(rec) = record {
                // M5g item c: record the outside-world reads to a v1 trace,
                // then run normally.
                if replay.is_some() {
                    eprintln!("Error: --record and --replay cannot be used together");
                    return ExitCode::from(2);
                }
                let mut buf = String::new();
                use std::io::Read;
                let _ = std::io::stdin().read_to_string(&mut buf);
                let run = run_big_traced(
                    source.clone(),
                    file.clone(),
                    buf,
                    lykaon::TraceMode::Record(Vec::new()),
                    deterministic,
                    seed,
                    fuel.unwrap_or(0),
                );
                if let Err(e) = lykaon::trace::write(
                    std::path::Path::new(&rec),
                    &source,
                    &prog_args,
                    &run.events,
                    run.outcome.exit,
                    &run.outcome.output,
                ) {
                    eprintln!("Error: cannot write {rec}: {e}");
                    return ExitCode::from(1);
                }
                print!("{}", run.outcome.output);
                let _ = std::io::Write::flush(&mut std::io::stdout());
                return ExitCode::from(run.outcome.exit as u8);
            }
            if let Some(rep) = replay {
                // M5g item c: a v1 trace replays; a plain text file is still
                // accepted as raw standard input (the old --record shape).
                let raw = match std::fs::read_to_string(&rep) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: cannot read {rep}: {e}");
                        return ExitCode::from(1);
                    }
                };
                let (stdin, mode, expected) = if raw.trim_start().starts_with('{') {
                    match lykaon::trace::parse(&raw) {
                        Ok(t) => {
                            let source_hash = lykaon::trace::sha256_hex(source.as_bytes());
                            if !t.program_sha256.is_empty() && t.program_sha256 != source_hash {
                                eprintln!(
                                    "Error: the trace was recorded for a different program (sha256 mismatch)."
                                );
                                return ExitCode::from(1);
                            }
                            let stdin = lykaon::trace::stdin_of(&t.events);
                            let events = lykaon::trace::replay_events(t.events);
                            (
                                stdin,
                                lykaon::TraceMode::Replay { events, pos: 0 },
                                Some((t.exit, t.output_sha256)),
                            )
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            return ExitCode::from(1);
                        }
                    }
                } else {
                    (raw, lykaon::TraceMode::Off, None)
                };
                if stop_after > 0 {
                    let (out, code, err) = run_big_until(
                        source.clone(),
                        file.clone(),
                        stdin,
                        mode,
                        stop_after,
                    );
                    print!("{out}");
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    if let Some(e) = err {
                        eprintln!("Error: {e}");
                    }
                    return ExitCode::from(code as u8);
                }
                let run = run_big_traced(
                    source.clone(),
                    file.clone(),
                    stdin,
                    mode,
                    deterministic,
                    seed,
                    fuel.unwrap_or(0),
                );
                print!("{}", run.outcome.output);
                let _ = std::io::Write::flush(&mut std::io::stdout());
                if let Some((exit, hash)) = expected {
                    let already_diverged = run.outcome.output.contains("Trace replay diverged");
                    if !already_diverged {
                        if lykaon::trace::sha256_hex(run.outcome.output.as_bytes()) != hash {
                            eprintln!(
                                "Error: Trace replay diverged: the output hash differs from the recording."
                            );
                            return ExitCode::from(1);
                        }
                        if run.replay_left > 0 {
                            eprintln!(
                                "Error: Trace replay diverged: {} recorded read(s) were not used.",
                                run.replay_left
                            );
                            return ExitCode::from(1);
                        }
                        if run.outcome.exit != exit {
                            eprintln!(
                                "Error: Trace replay diverged: exit {} but the recording had {}.",
                                run.outcome.exit, exit
                            );
                            return ExitCode::from(1);
                        }
                    }
                }
                return ExitCode::from(run.outcome.exit as u8);
            }
            if let Some(f) = fuel.filter(|_| interpret) {
                // M5g item a: the interpreter budget path (an explicit
                // --interpret). A native run takes the budget through
                // native_run_tool and the codegen tick.
                // run with a fuel budget on a large-stack worker.
                let src = source.clone();
                let f2 = file.clone();
                let h = std::thread::Builder::new()
                    .stack_size(256 * 1024 * 1024)
                    .spawn(move || lykaon::run_source_budgeted(&src, &f2, "", f))
                    .expect("spawn fuel worker");
                let (out, code) = h.join().unwrap_or((String::new(), 1));
                print!("{out}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                return ExitCode::from(code as u8);
            }
            if stop_after > 0 {
                let (out, code, err) = run_big_until(
                    source.clone(),
                    file.clone(),
                    String::new(),
                    lykaon::TraceMode::Off,
                    stop_after,
                );
                print!("{out}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                if let Some(e) = err {
                    eprintln!("Error: {e}");
                }
                return ExitCode::from(code as u8);
            }
            if !interpret {
                // Native mode (M4-D17 default): compile, link with
                // liblykaon_rt, and run the built program with the program's
                // arguments (M4-S82). A missing native toolchain falls
                // back to the interpreter with a note (the differential
                // guarantee makes the two byte-identical).
                match native_run_tool(
                    &source,
                    &file,
                    native_mode.unwrap_or("debug"),
                    &prog_args,
                    fuel.unwrap_or(0),
                ) {
                    Ok((code, out, err)) => {
                        print!("{out}");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        if !err.is_empty() {
                            eprint!("{err}");
                        }
                        ExitCode::from(code as u8)
                    }
                    Err(NativeFail::Diags(text)) => {
                        // diagnostics print to the console exactly like the
                        // interpreter's compile does (M3 differential rule)
                        print!("{text}");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        ExitCode::from(1)
                    }
                    Err(NativeFail::Tool(e)) => {
                        eprintln!("Note: {e}");
                        eprintln!("Note: running with the interpreter instead.");
                        lykaon::cli::run_interpreter(source, file.clone(), prog_args.clone())
                    }
                }
            } else {
                lykaon::cli::run_interpreter(source, file, prog_args)
            }
        }
        "test" => {
            // M5-D15: lykaon test coverage [--min N]
            if args.get(2).map(|s| s == "coverage").unwrap_or(false) {
                let mut min = None;
                let mut i = 3usize;
                while i < args.len() {
                    if args[i] == "--min" {
                        if let Some(v) = args.get(i + 1).and_then(|v| v.parse::<f64>().ok()) {
                            min = Some(v);
                        }
                        i += 2;
                        continue;
                    }
                    i += 1;
                }
                return match lykaon::test_coverage::run(std::path::Path::new("."), min) {
                    Ok(text) => { print!("{text}\n"); ExitCode::from(0) }
                    Err(text) => { eprintln!("{text}"); ExitCode::from(1) }
                };
            }
            if args.len() < 3 {
                eprintln!("usage: lykaon test <file>");
                return ExitCode::from(2);
            }
            let file = &args[2];
            // M5c: lykaon test <file> --json (additive; default text is
            // unchanged).
            if args.iter().any(|a| a == "--json") {
                return match lykaon::m5c::test_json(file) {
                    Ok(json) => {
                        println!("{json}");
                        ExitCode::from(0)
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::from(1)
                    }
                };
            }
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            let (out, ok) = lykaon::test_source(&source, file, "");
            print!("{out}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            ExitCode::from(if ok { 0 } else { 1 })
        }
        "repl" => lykaon::cli::repl_session(),
        "ui" | "--ui" => {
            #[cfg(feature = "gui")]
            {
                lykaon::ui::compiler::run();
                ExitCode::from(0)
            }
            #[cfg(not(feature = "gui"))]
            {
                eprintln!(
                    "the compiler UI is built with: cargo build --features gui --bin Lykaonc"
                );
                ExitCode::from(2)
            }
        }
        "add" => {
            if args.len() < 3 {
                eprintln!("usage: lykaon add <folder>");
                return ExitCode::from(2);
            }
            let folder = &args[2];
            let toml = PathBuf::from("Lykaon.toml");
            let text = match std::fs::read_to_string(&toml) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("Error: no Lykaon.toml here. Create one first.");
                    return ExitCode::from(1);
                }
            };
            let mut proj = match packages::parse_project(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            };
            if proj.needs.iter().any(|d| d.name == *folder) {
                eprintln!("Error: {folder} is already a dependency.");
                return ExitCode::from(1);
            }
            if packages::looks_like_git_url(folder) {
                let name = packages::name_from_git_url(folder);
                if proj.needs.iter().any(|d| d.name == name) {
                    eprintln!("Error: {name} is already a dependency.");
                    return ExitCode::from(1);
                }
                proj.needs.push(packages::Dependency {
                    name,
                    source: packages::Source::Git {
                        url: folder.clone(),
                        version: None,
                    },
                });
            } else {
                proj.needs.push(packages::Dependency {
                    name: folder.clone(),
                    source: packages::Source::Folder(PathBuf::from(folder)),
                });
            }
            let mut out = String::new();
            if let Some(n) = &proj.name {
                out.push_str(&format!("Name is {n}.\n"));
            }
            if let Some((ma, mi, pa)) = proj.version {
                out.push_str(&format!("Version is {ma} point {mi} point {pa}.\n"));
            }
            if let Some(e) = &proj.entry {
                out.push_str(&format!("Entry is {e}.\n"));
            }
            for dep in &proj.needs {
                match &dep.source {
                    packages::Source::Folder(f) => {
                        out.push_str(&format!(
                            "Needs {} from folder {}.\n",
                            dep.name,
                            f.display()
                        ));
                    }
                    packages::Source::Git { url, version } => match version {
                        Some((ma, mi, pa)) => out.push_str(&format!(
                            "Needs {} from git {} at version {ma} point {mi} point {pa}.\n",
                            dep.name, url
                        )),
                        None => out.push_str(&format!("Needs {} from git {}.\n", dep.name, url)),
                    },
                }
            }
            match std::fs::write(&toml, &out) {
                Ok(()) => {
                    println!("Added dependency {folder}. Run lykaon update to write lykaon.lock.");
                    // M5 Step 6: show the permissions the new dependency asks
                    // for, so a person can review them before running it.
                    if !packages::looks_like_git_url(folder) {
                        if let Some(summary) =
                            lykaon::sandbox::Sandbox::summarize_dir(std::path::Path::new(folder))
                        {
                            println!("{folder} requests permission for:\n{summary}");
                        } else {
                            println!("{folder} requests no special permissions.");
                        }
                    } else {
                        println!(
                            "Permissions for {folder} will be shown after lykaon update fetches it."
                        );
                    }
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: cannot write Lykaon.toml: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "remove" => {
            if args.len() < 3 {
                eprintln!("usage: lykaon remove <name>");
                return ExitCode::from(2);
            }
            let name = &args[2];
            let toml = PathBuf::from("Lykaon.toml");
            let text = match std::fs::read_to_string(&toml) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("Error: no Lykaon.toml here. Create one first.");
                    return ExitCode::from(1);
                }
            };
            let mut proj = match packages::parse_project(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            };
            let before = proj.needs.len();
            proj.needs.retain(|d| d.name != *name);
            if proj.needs.len() == before {
                eprintln!("Error: {name} is not a dependency.");
                return ExitCode::from(1);
            }
            let mut out = String::new();
            if let Some(n) = &proj.name {
                out.push_str(&format!("Name is {n}.\n"));
            }
            if let Some((ma, mi, pa)) = proj.version {
                out.push_str(&format!("Version is {ma} point {mi} point {pa}.\n"));
            }
            if let Some(e) = &proj.entry {
                out.push_str(&format!("Entry is {e}.\n"));
            }
            for dep in &proj.needs {
                match &dep.source {
                    packages::Source::Folder(f) => {
                        out.push_str(&format!(
                            "Needs {} from folder {}.\n",
                            dep.name,
                            f.display()
                        ));
                    }
                    packages::Source::Git { url, version } => match version {
                        Some((ma, mi, pa)) => out.push_str(&format!(
                            "Needs {} from git {} at version {ma} point {mi} point {pa}.\n",
                            dep.name, url
                        )),
                        None => out.push_str(&format!("Needs {} from git {}.\n", dep.name, url)),
                    },
                }
            }
            match std::fs::write(&toml, &out) {
                Ok(()) => {
                    println!("Removed dependency {name}.");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: cannot write Lykaon.toml: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "update" => {
            let toml = PathBuf::from("Lykaon.toml");
            let text = match std::fs::read_to_string(&toml) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("Error: no Lykaon.toml here. Create one first.");
                    return ExitCode::from(1);
                }
            };
            let proj = match packages::parse_project(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            };
            // the previous lock lets `lykaon update` catch a rewritten tag
            // (T0520's "its checksum differs"); a damaged lock is T0516
            let old_lock = match std::fs::read_to_string("lykaon.lock") {
                Ok(t) => match packages::parse_lockfile(&t) {
                    Ok(l) => Some(l),
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::from(1);
                    }
                },
                Err(_) => None,
            };
            // git dependencies are fetched into the cache first (M4-11);
            // a failure is T0520 with the book's fix wording
            let fetched = match packages::sync_git_deps(
                &proj,
                &std::env::current_dir().unwrap_or_default(),
                old_lock.as_ref(),
            ) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            };
            for (name, _) in &fetched {
                println!("Fetched {name} from git.");
            }
            if let Err(e) =
                packages::validate_deps(&proj, &std::env::current_dir().unwrap_or_default())
            {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
            let lock = packages::write_lockfile(&proj, &fetched);
            match std::fs::write("lykaon.lock", &lock) {
                Ok(()) => {
                    println!("Wrote lykaon.lock.");
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: cannot write lykaon.lock: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "build" => {
            // M6-G32: `lykaon build for library <input> [outdir]`.
            if args.len() >= 4 && args[2] == "for" && args[3] == "library" {
                let input = args[4].clone();
                let outdir = args
                    .get(5)
                    .cloned()
                    .unwrap_or_else(|| ".".to_string());
                let source = match std::fs::read_to_string(&input) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: cannot read {input}: {e}");
                        return ExitCode::from(1);
                    }
                };
                let prog = match lykaon::compile_tree(&source, &input) {
                    Ok(p) => p,
                    Err(diags) => {
                        for d in &diags {
                            eprintln!("{d}");
                        }
                        return ExitCode::from(1);
                    }
                };
                let stem = std::path::Path::new(&input)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("library")
                    .to_string();
                let dir = std::path::Path::new(&outdir);
                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("Error: cannot create {outdir}: {e}");
                    return ExitCode::from(1);
                }
                let header = lykaon::c_header(&prog);
                let header_path = dir.join(format!("{stem}.h"));
                if let Err(e) = std::fs::write(&header_path, header) {
                    eprintln!("Error: cannot write {}: {e}", header_path.display());
                    return ExitCode::from(1);
                }
                let exports: Vec<String> =
                    prog.c_exports.iter().map(|f| f.name.clone()).collect();
                let nest = serde_json::json!({
                    "name": stem,
                    "exports": exports,
                });
                let nest_path = dir.join(format!("{stem}.nest"));
                if let Err(e) = std::fs::write(&nest_path, nest.to_string()) {
                    eprintln!("Error: cannot write {}: {e}", nest_path.display());
                    return ExitCode::from(1);
                }
                println!("built {stem} for library");
                return ExitCode::from(0);
            }
            // M6-G36: `lykaon build for python <input> [outdir]`.
            if args.len() >= 4 && args[2] == "for" && args[3] == "python" {
                let input = args[4].clone();
                let outdir = args
                    .get(5)
                    .cloned()
                    .unwrap_or_else(|| ".".to_string());
                let source = match std::fs::read_to_string(&input) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: cannot read {input}: {e}");
                        return ExitCode::from(1);
                    }
                };
                if let Err(diags) = lykaon::compile_tree(&source, &input) {
                    for d in &diags {
                        eprintln!("{d}");
                    }
                    return ExitCode::from(1);
                }
                let stem = std::path::Path::new(&input)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("program")
                    .to_string();
                let dir = std::path::Path::new(&outdir);
                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("Error: cannot create {outdir}: {e}");
                    return ExitCode::from(1);
                }
                let pyi = format!(
                    "# Lykaon module {stem}\ndef run() -> int:\n    \"\"\"Run the embedded Lykaon program.\"\"\"\n    ...\n"
                );
                if let Err(e) = std::fs::write(dir.join(format!("{stem}.pyi")), pyi) {
                    eprintln!("Error: cannot write the .pyi: {e}");
                    return ExitCode::from(1);
                }
                let c = format!(
                    "#include <Python.h>\n\
                     extern int lykaon_rt_main(const char *src, long long len, int mode);\n\
                     static PyObject *lykaon_run(PyObject *self, PyObject *args) {{\n\
                     \x20   const char *src; Py_ssize_t len;\n\
                     \x20   if (!PyArg_ParseTuple(args, \"s#\", &src, &len)) return NULL;\n\
                     \x20   return PyLong_FromLong(lykaon_rt_main(src, (long long)len, 0));\n\
                     }}\n\
                     static PyMethodDef methods[] = {{\n\
                     \x20   {{\"run\", lykaon_run, METH_VARARGS, \"Run the embedded Lykaon program.\"}},\n\
                     \x20   {{NULL, NULL, 0, NULL}}\n\
                     }};\n\
                     static struct PyModuleDef module = {{\n\
                     \x20   PyModuleDef_HEAD_INIT, \"{stem}\", NULL, -1, methods\n\
                     }};\n\
                     PyMODINIT_FUNC PyInit_{stem}(void) {{ return PyModule_Create(&module); }}\n"
                );
                if let Err(e) = std::fs::write(dir.join(format!("{stem}.c")), c) {
                    eprintln!("Error: cannot write the extension source: {e}");
                    return ExitCode::from(1);
                }
                // Try to build the importable extension. Windows uses the
                // detected MSVC cl/link; other platforms need a C toolchain.
                let py = std::process::Command::new("python")
                    .args([
                        "-c",
                        "import sysconfig; print(sysconfig.get_paths()['include']); print(sysconfig.get_config_var('LIBDIR')); print(sysconfig.get_config_var('LDLIBRARY'))",
                    ])
                    .output();
                if let Ok(py) = py {
                    if py.status.success() {
                        let text = String::from_utf8_lossy(&py.stdout);
                        let mut it = text.lines();
                        let include = it.next().unwrap_or("").to_string();
                        let libdir = it.next().unwrap_or("").to_string();
                        let dll = it.next().unwrap_or("python314.dll").to_string();
                        let pylib = dll.replace(".dll", ".lib");
                        if let Ok(tc) = lykaon::linker::detect() {
                            unsafe {
                                std::env::set_var("HOST", "x86_64-pc-windows-msvc");
                                std::env::set_var("TARGET", "x86_64-pc-windows-msvc");
                                std::env::set_var("OPT_LEVEL", "2");
                            }
                            let obj = dir.join("obj");
                                let cfile = dir.join(format!("{stem}.c"));
                                let _ = std::fs::create_dir_all(&obj);
                                unsafe {
                                    std::env::set_var("OUT_DIR", &obj);
                                }
                                cc::Build::new()
                                    .file(&cfile)
                                    .include(&include)
                                    .warnings(false)
                                    .compile("lykaon_py");
                                let pyobj = obj.join("lykaon_py.lib");
                                if pyobj.is_file() {
                                    let rt = std::path::Path::new("target/release/lykaon_rt.lib");
                                    let pyd = dir.join(format!("{stem}.pyd"));
                                    let prog_obj = dir.join(format!("{stem}.obj"));
                                    let obj_bytes = match lykaon::native_object(&source, &input, "fast", false) {
                                        Ok((o, _, _)) => o,
                                        Err(e) => {
                                            eprintln!("python object failed");
                                            Vec::new()
                                        }
                                    };
                                    if obj_bytes.is_empty() {
                                        println!("built {stem} for python (source + .pyi; link with the Python C API)");
                                        return ExitCode::from(0);
                                    }
                                    if std::fs::write(&prog_obj, &obj_bytes).is_err() {
                                        eprintln!("python object write failed");
                                        return ExitCode::from(1);
                                    }
                                    let mut cmd = std::process::Command::new(&tc.link);
                                    cmd.arg("/nologo").arg("/DLL").arg("/NODEFAULTLIB:LIBCMT");
                                    for d in &tc.lib_dirs {
                                        cmd.arg(format!("/LIBPATH:{}", d.display()));
                                    }
                                    cmd.arg(format!("/LIBPATH:{libdir}"));
                                    cmd.arg(format!("/WHOLEARCHIVE:{}", pyobj.to_string_lossy()));
                                    cmd.arg(&prog_obj);
                                    cmd.arg(rt);
                                    cmd.arg(&pylib);
                                    for lib in [
                                        "ws2_32.lib",
                                        "advapi32.lib",
                                        "bcrypt.lib",
                                        "ntdll.lib",
                                        "userenv.lib",
                                        "crypt32.lib",
                                        "secur32.lib",
                                        "iphlpapi.lib",
                                    ] {
                                        cmd.arg(lib);
                                    }
                                    cmd.arg(format!("/OUT:{}", pyd.to_string_lossy()));
                                    let link = cmd.output();
                                    if link.as_ref().map(|o| o.status.success()).unwrap_or(false) {
                                        println!("built {stem} for python ({})", pyd.display());
                                        return ExitCode::from(0);
                                    } else if let Ok(o) = link {
                                        eprintln!(
                                            "python link failed: {}{}",
                                            String::from_utf8_lossy(&o.stdout),
                                            String::from_utf8_lossy(&o.stderr)
                                        );
                                    }
                                }
                            }
                        }
                    }
                println!("built {stem} for python (source + .pyi; link with the Python C API)");
                return ExitCode::from(0);
            }
            // M6-G36: `lykaon build for js <input> [outdir]`.
            if args.len() >= 4 && args[2] == "for" && args[3] == "js" {
                let input = args[4].clone();
                let outdir = args.get(5).cloned().unwrap_or_else(|| ".".to_string());
                let source = match std::fs::read_to_string(&input) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error: cannot read {input}: {e}");
                        return ExitCode::from(1);
                    }
                };
                if let Err(diags) = lykaon::compile_tree(&source, &input) {
                    for d in &diags {
                        eprintln!("{d}");
                    }
                    return ExitCode::from(1);
                }
                let stem = std::path::Path::new(&input)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("program")
                    .to_string();
                let dir = std::path::Path::new(&outdir);
                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("Error: cannot create {outdir}: {e}");
                    return ExitCode::from(1);
                }
                let wasm_src = std::path::Path::new("playground/lykaon_playground.wasm");
                if !wasm_src.is_file() {
                    eprintln!("Error: playground/lykaon_playground.wasm is missing; build the playground first");
                    return ExitCode::from(1);
                }
                if let Err(e) = std::fs::copy(wasm_src, dir.join(format!("{stem}.wasm"))) {
                    eprintln!("Error: cannot copy the wasm runtime: {e}");
                    return ExitCode::from(1);
                }
                let src_json = serde_json::to_string(&source).unwrap_or_else(|_| "\"\"".to_string());
                let js = format!(
                    "const SOURCE = {src_json};\n\
                     async function load() {{\n\
                     \x20   const bytes = await (await fetch(new URL('./{stem}.wasm', import.meta.url))).arrayBuffer();\n\
                     \x20   return WebAssembly.instantiate(bytes);\n\
                     }}\n\
                     export async function run() {{\n\
                     \x20   const {{ instance }} = await load();\n\
                     \x20   const memory = instance.exports.memory;\n\
                     \x20   const ptr = instance.exports.lykaon_alloc(SOURCE.length);\n\
                     \x20   const data = new TextEncoder().encode(SOURCE);\n\
                     \x20   new Uint8Array(memory.buffer, ptr, data.length).set(data);\n\
                     \x20   const packed = instance.exports.lykaon_run(ptr, data.length);\n\
                     \x20   const outPtr = Number(packed >> 32n);\n\
                     \x20   const outLen = Number(packed & 0xffffffffn);\n\
                     \x20   return new TextDecoder().decode(new Uint8Array(memory.buffer, outPtr, outLen));\n\
                     }}\n"
                );
                if let Err(e) = std::fs::write(dir.join(format!("{stem}.js")), js) {
                    eprintln!("Error: cannot write the JS loader: {e}");
                    return ExitCode::from(1);
                }
                let dts = format!(
                    "# Lykaon module {stem}\nexport function run(): Promise<string>;\n"
                );
                if let Err(e) = std::fs::write(dir.join(format!("{stem}.d.ts")), dts) {
                    eprintln!("Error: cannot write the .d.ts: {e}");
                    return ExitCode::from(1);
                }
                println!("built {stem} for js (wasm + loader + .d.ts)");
                return ExitCode::from(0);
            }
            // M3-S44: `lykaon build <debug|fast|small> [input.lykaon] [output]`,
            // the default output being build/<stem>.obj; the summary line is
            // `built <stem> in <mode>`, with the lowering reported honestly.
            let mode = if args.len() >= 3 {
                args[2].as_str()
            } else {
                "debug"
            };
            let input_file = if args.len() >= 4 {
                args[3].as_str()
            } else {
                "main.lykaon"
            };
            if !matches!(mode, "debug" | "fast" | "small") {
                eprintln!("usage: lykaon build <debug|fast|small> [input.lykaon] [output]");
                return ExitCode::from(2);
            }
            let output_file = if args.len() >= 5 {
                args[4].clone()
            } else {
                let stem = std::path::Path::new(input_file)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("program");
                // `lykaon build` produces a runnable program, not an object:
                // the .lykaon source becomes a native executable (the M4
                // requirement that the language makes command-line tools).
                format!("build/{stem}{}", lykaon::linker::exe_suffix())
            };
            // An output named .obj/.o explicitly asks for the object only, the
            // M3 behaviour the bench and differential harnesses rely on.
            let object_only = matches!(
                std::path::Path::new(&output_file)
                    .extension()
                    .and_then(|e| e.to_str()),
                Some("obj") | Some("o")
            );
            let source = match std::fs::read_to_string(input_file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {input_file}: {e}");
                    return ExitCode::from(1);
                }
            };
            let _ = std::fs::create_dir_all(
                std::path::Path::new(&output_file)
                    .parent()
                    .unwrap_or(std::path::Path::new(".")),
            );
            match lykaon::native_object(&source, input_file, mode, false) {
                Ok((obj_bytes, kind, reports)) => {
                    let stem = std::path::Path::new(input_file)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("program");
                    // Write the object beside the final program, then link.
                    // An object-only request writes straight to output_file.
                    let obj_path = if object_only {
                        std::path::PathBuf::from(&output_file)
                    } else {
                        std::path::PathBuf::from(format!(
                            "{}.lykaon-obj-{}{}",
                            output_file,
                            std::process::id(),
                            if cfg!(windows) { ".obj" } else { ".o" }
                        ))
                    };
                    if let Err(e) = std::fs::write(&obj_path, &obj_bytes) {
                        eprintln!("Error: cannot write {}: {e}", obj_path.display());
                        return ExitCode::from(1);
                    }
                    if !object_only {
                        let linked = lykaon::linker::find_rt_lib()
                            .map_err(|e| format!("the runtime library is missing: {e}"))
                            .and_then(|rt| {
                                lykaon::linker::link_exe(
                                    &obj_path,
                                    &rt,
                                    std::path::Path::new(&output_file),
                                )
                            });
                        let _ = std::fs::remove_file(&obj_path);
                        if let Err(e) = linked {
                            eprintln!("Error: {e}");
                            return ExitCode::from(1);
                        }
                    }
                    println!("built {stem} in {mode}");
                    match kind {
                        lykaon::codegen::LowerKind::Full => {
                            println!("{output_file} â€” machine code (full scalar lowering)")
                        }
                        lykaon::codegen::LowerKind::Embedded { reason } => {
                            println!("{output_file} â€” embedded evaluator: {reason}")
                        }
                    }
                    for r in &reports {
                        println!(
                            "  optimizer: {} â€” {} change{}",
                            r.name,
                            r.changes,
                            if r.changes == 1 { "" } else { "s" }
                        );
                    }
                    ExitCode::from(0)
                }
                Err(f) => {
                    // diagnostics print to the console like the interpreter's
                    print!("{}", f.message());
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    ExitCode::from(1)
                }
            }
        }
        "run-many" => {
            // M5g item g: compile once, run N times, report throughput.
            //   lykaon run-many <file> [--runs N] [--warmup N] [--threads N]
            //   [--fuel N] [--input TEXT] [--json]
            let mut file: Option<String> = None;
            let mut opts = lykaon::run_many::Options::default();
            let mut json = false;
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--runs" => {
                        opts.runs = args
                            .get(i + 1)
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(opts.runs);
                        i += 2;
                    }
                    "--warmup" => {
                        opts.warmup = args
                            .get(i + 1)
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(opts.warmup);
                        i += 2;
                    }
                    "--threads" => {
                        opts.threads = args
                            .get(i + 1)
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(opts.threads);
                        i += 2;
                    }
                    "--fuel" => {
                        opts.fuel = args
                            .get(i + 1)
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(opts.fuel);
                        i += 2;
                    }
                    "--input" => {
                        opts.input = args.get(i + 1).cloned().unwrap_or_default();
                        i += 2;
                    }
                    "--json" => {
                        json = true;
                        i += 1;
                    }
                    other if other.starts_with("--") => i += 1,
                    other => {
                        file = Some(other.to_string());
                        i += 1;
                    }
                }
            }
            let file = file.unwrap_or_else(|| "main.lykaon".to_string());
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            match lykaon::run_many::run(&source, &file, &opts) {
                Ok(summary) => {
                    if json {
                        println!("{}", lykaon::run_many::json(&summary));
                    } else {
                        print!("{}", lykaon::run_many::report(&summary));
                    }
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "bench" => {
            // The benchmark programs live in benchmarks/ in a release export
            // and in tests/bench/ in the development tree.
            let bench_dir = ["benchmarks", "tests/bench"]
                .map(PathBuf::from)
                .into_iter()
                .find(|d| d.is_dir());
            let Some(bench_dir) = bench_dir else {
                eprintln!("Error: benchmarks/ not found.");
                return ExitCode::from(1);
            };
            let rust_ref_path = bench_dir.join("rust_reference.json");
            let rust_times: Vec<(String, f64)> = match std::fs::read_to_string(&rust_ref_path) {
                Ok(text) => serde_json::from_str::<Vec<(String, f64)>>(&text).unwrap_or_else(|e| {
                    eprintln!("Error: cannot parse {}: {e}", rust_ref_path.display());
                    std::process::exit(1);
                }),
                Err(_) => {
                    eprintln!(
                        "Error: cannot read {}. Build the Rust reference benchmarks first.",
                        rust_ref_path.display()
                    );
                    return ExitCode::from(1);
                }
            };
            let rust_ref_dir = bench_dir.join("reference").join("bin");
            let bench_exe_dir = bench_dir.join("bin");
            let _ = std::fs::create_dir_all(&bench_exe_dir);

            // Stored reference times are a staleness cross-check only; the
            // P-061 ratios are computed from Rust medians measured in this
            // run (see below), so both sides are same-method, same-run.
            let mut rat_d: Vec<f64> = Vec::new();
            let mut rat_f: Vec<f64> = Vec::new();
            // M3b-6: contract checks the prover proves skippable, summed
            // over every benchmark program (Ensures contracts + Always sites).
            let mut checks_total: usize = 0;
            for (name, rust_ms) in &rust_times {
                let lykaon_path = bench_dir.join(format!("{name}.lykaon"));
                let source = match std::fs::read_to_string(&lykaon_path) {
                    Ok(s) => s,
                    Err(_) => {
                        eprintln!(
                            "Error: cannot read {lykaon_path:?}. Write the benchmark program first."
                        );
                        return ExitCode::from(1);
                    }
                };
                let rust_exe = rust_ref_dir.join(format!("{name}.exe"));
                if !rust_exe.is_file() {
                    eprintln!("Error: Rust reference exe not found: {rust_exe:?}. Build it first.");
                    return ExitCode::from(1);
                }

                // M3b-6: count how many contract checks the prover removes
                // for this benchmark program (the interpreter/net check
                // count is mode-independent, so one pass is enough).
                let checks = match lykaon::compile_tree(&source, lykaon_path.to_str().unwrap()) {
                    Ok(mut p) => lykaon::prover::checks_removed(&mut p),
                    Err(ds) => {
                        eprintln!(
                            "Error: {name} does not compile for prover counting: {} diagnostic(s)",
                            ds.len()
                        );
                        return ExitCode::from(1);
                    }
                };
                checks_total += checks;

                // Lykaon default (debug): compile once (timed), then run 10x.
                let (lykaon_exe_d, compile_ms_d) = match lykaon::native_compile_to_exe(
                    &source,
                    lykaon_path.to_str().unwrap(),
                    "debug",
                    &bench_exe_dir,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "Error: {name} lykaon compile (debug) failed: {}",
                            e.message()
                        );
                        return ExitCode::from(1);
                    }
                };
                let _ = lykaon::run_exe(&lykaon_exe_d); // warm up
                let mut times_d: Vec<f64> = Vec::new();
                let mut out_d = String::new();
                for _ in 0..10 {
                    match lykaon::run_exe(&lykaon_exe_d) {
                        Ok((code, out, ms)) => {
                            if code != 0 {
                                eprintln!(
                                    "Error: {name} lykaon run (debug) failed with exit code {code}"
                                );
                                return ExitCode::from(1);
                            }
                            times_d.push(ms);
                            out_d = out;
                        }
                        Err(e) => {
                            eprintln!("Error: {name} lykaon run (debug) failed: {}", e.message());
                            return ExitCode::from(1);
                        }
                    }
                }
                let _ = std::fs::remove_file(&lykaon_exe_d);
                times_d.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let median_d = times_d[times_d.len() / 2];

                // Lykaon fast (like-for-like): compile once (timed), then run 10x.
                let (lykaon_exe_f, compile_ms_f) = match lykaon::native_compile_to_exe(
                    &source,
                    lykaon_path.to_str().unwrap(),
                    "fast",
                    &bench_exe_dir,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "Error: {name} lykaon compile (fast) failed: {}",
                            e.message()
                        );
                        return ExitCode::from(1);
                    }
                };
                let _ = lykaon::run_exe(&lykaon_exe_f); // warm up
                let mut times_f: Vec<f64> = Vec::new();
                let mut out_f = String::new();
                for _ in 0..10 {
                    match lykaon::run_exe(&lykaon_exe_f) {
                        Ok((code, out, ms)) => {
                            if code != 0 {
                                eprintln!(
                                    "Error: {name} lykaon run (fast) failed with exit code {code}"
                                );
                                return ExitCode::from(1);
                            }
                            times_f.push(ms);
                            out_f = out;
                        }
                        Err(e) => {
                            eprintln!("Error: {name} lykaon run (fast) failed: {}", e.message());
                            return ExitCode::from(1);
                        }
                    }
                }
                let _ = std::fs::remove_file(&lykaon_exe_f);
                times_f.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let median_f = times_f[times_f.len() / 2];

                // Rust: run 10x (timed) and capture output checksum.
                let _ = lykaon::run_exe(&rust_exe); // warm up
                let mut rust_times_vec: Vec<f64> = Vec::new();
                let mut rust_out = String::new();
                for _ in 0..10 {
                    match lykaon::run_exe(&rust_exe) {
                        Ok((code, out, ms)) => {
                            if code != 0 {
                                eprintln!("Error: {name} rust run failed with exit code {code}");
                                return ExitCode::from(1);
                            }
                            rust_times_vec.push(ms);
                            rust_out = out.trim().to_string();
                        }
                        Err(e) => {
                            eprintln!("Error: {name} rust run failed: {}", e.message());
                            return ExitCode::from(1);
                        }
                    }
                }
                rust_times_vec.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let rust_median = rust_times_vec[rust_times_vec.len() / 2];

                // Verify Lykaon and Rust outputs match (checksum comparison).
                if out_f.trim() != rust_out {
                    eprintln!(
                        "Error: {name} checksum mismatch: Lykaon={out_f:?} Rust={rust_out:?}"
                    );
                    return ExitCode::from(1);
                }

                // The honest ratio divides by the Rust median measured in
                // THIS run (same exe-vs-exe method, same machine state),
                // not by the stored reference time. The stored reference is
                // only a cross-check: if the stored and measured Rust times
                // disagree by more than 2x, the stored file is stale.
                let ratio_d = median_d / rust_median;
                let ratio_f = median_f / rust_median;
                rat_d.push(ratio_d);
                rat_f.push(ratio_f);
                if rust_median < *rust_ms * 0.5 || rust_median > *rust_ms * 2.0 {
                    eprintln!(
                        "warning: {name} measured Rust time {rust_median:.1}ms differs from stored {} ms; update {name}.json entry",
                        rust_ms
                    );
                }

                println!(
                    "{}: lykaon_debug={:.1}ms (ratio {:.3}x) lykaon_fast={:.1}ms (ratio {:.3}x) rust={:.1}ms compile_d={:.1}ms compile_f={:.1}ms runs=10 checksum={} checks_removed={}",
                    name,
                    median_d,
                    ratio_d,
                    median_f,
                    ratio_f,
                    rust_median,
                    compile_ms_d,
                    compile_ms_f,
                    out_f.trim(),
                    checks,
                );
            }
            let gm_d: f64 = rat_d.iter().product::<f64>().powf(1.0 / rat_d.len() as f64);
            let gm_f: f64 = rat_f.iter().product::<f64>().powf(1.0 / rat_f.len() as f64);
            println!("contract prover checks removed across benchmarks: {checks_total}");
            println!("geometric mean ratio (default/debug): {gm_d:.3}x");
            println!("geometric mean ratio (like-for-like/fast): {gm_f:.3}x");
            ExitCode::from(0)
        }
        other => {
            eprintln!("unknown command: {other}");
            ExitCode::from(2)
        }
    }
}

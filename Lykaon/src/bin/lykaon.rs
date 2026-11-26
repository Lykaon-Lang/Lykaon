//! The lykaon front: the interpreter/REPL.
//!
//! The compiler front is the lykaonc exe (src/main.rs). This front runs
//! the interpreter only: `lykaon run <file> [args]` (the M4-D17
//! --interpret path, the default of this front) and `lykaon repl`
//! (the REPL). The compiler commands (the native run, build, check, ...)
//! belong to lykaonc.

use std::io::IsTerminal;
use std::process::ExitCode;

#[global_allocator]
static GLOBAL_ALLOC: lykaon::alloc::CountingAllocator = lykaon::alloc::CountingAllocator;

fn main() -> ExitCode {
    // A double click on the exe opens a console that would close before the
    // usage text can be read. When stdout is a real console window, start the
    // REPL instead of exiting, so the interpreter stays open. Piped and
    // automated runs are not terminals and keep the old usage-and-exit path.
    let interactive_console = std::io::stdout().is_terminal();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("{info}");
        if std::io::stderr().is_terminal() {
            eprintln!("\nPress Enter to close...");
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
        }
    }));
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("run") => {
            // the lykaon flags belong before the file; everything after
            // the file is the program's own command line (M4-S82).
            // --interpret is this front's default (the flag is accepted);
            // the compiler flags belong to lykaonc
            // M5-S43: `lykaon run safely [file]` (or --safely) enables
            // the permission sandbox before the program runs.
            let mut trace = false;
            let mut safely = false;
            let mut file: Option<String> = None;
            let mut prog_args: Vec<String> = Vec::new();
            let mut i = 2;
            while i < args.len() {
                let a = args[i].as_str();
                if file.is_none() {
                    match a {
                        "--interpret" => {} // this front's default
                        "--safely" | "safely" => safely = true,
                        "--trace-json" => trace = true,
                        "--native" | "--fast" | "--small" => {
                            eprintln!("the compiler front is lykaonc-cli ({a} belongs there)");
                            return ExitCode::from(2);
                        }
                        _ => file = Some(args[i].clone()),
                    }
                } else {
                    prog_args.push(args[i].clone());
                }
                i += 1;
            }
            let Some(file) = file else {
                eprintln!("usage: lykaon-cli run <file> [arguments]");
                eprintln!("the compiler front (the native run) is lykaonc-cli run");
                return ExitCode::from(2);
            };
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: cannot read {file}: {e}");
                    return ExitCode::from(1);
                }
            };
            if safely {
                unsafe { std::env::set_var("LYKAON_SAFE", "1") };
            }
            if trace {
                lykaon_core::interp::enable_trace();
            }
            let code = lykaon::cli::run_interpreter(source, file, prog_args);
            if trace {
                let steps = lykaon_core::interp::take_trace();
                println!(
                    "{{\"steps\":[{}]}}",
                    steps
                        .iter()
                        .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
            code
        }
        Some("repl") => lykaon::cli::repl_session(),
        Some("ui") | Some("--ui") => {
            #[cfg(feature = "gui")]
            {
                lykaon::ui::interpreter::run();
                ExitCode::from(0)
            }
            #[cfg(not(feature = "gui"))]
            {
                eprintln!(
                    "the interpreter UI is built with: cargo build --features gui --bin Lykaon"
                );
                ExitCode::from(2)
            }
        }
        Some(other) => {
            eprintln!(
                "the lykaon front runs the interpreter: lykaon run <file> | lykaon repl | lykaon ui"
            );
            eprintln!("the compiler front is lykaonc-cli ({other} belongs there)");
            ExitCode::from(2)
        }
        None => {
            eprintln!("usage: lykaon-cli run <file> [arguments] | lykaon repl | lykaon ui");
            eprintln!("the compiler front is lykaonc-cli (build, check, native run, ui)");
            if interactive_console {
                // a double click opens a console: write and run code right
                // here instead of closing the window
                return lykaon::cli::repl_session();
            }
            ExitCode::from(2)
        }
    }
}

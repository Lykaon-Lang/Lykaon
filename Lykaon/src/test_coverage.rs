//! M5 (Appendix L 9.4): `lykaon test coverage`.
//!
//! Counts, per source file, how many sentences the tests ran. Test blocks are
//! the drivers, not the code under test, so they are not counted. A sentence
//! is identified by the debug form of its typed-tree node, which is stable
//! within one compiled program; the same hash is recorded at run time.
//!
//! The evaluator needs a large stack (`Value` is a large enum), so each file's
//! compile + test run happens on a worker thread with a 256 MiB stack. The
//! program is built inside that thread and never crosses it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use lykaon_core::interp;
use lykaon_core::sem::{FnBody, Program, Stmt};

fn add_stmt(st: &Stmt, all: &mut HashSet<u64>) {
    // Test and Check are drivers, not code under test.
    if matches!(st, Stmt::Test { .. } | Stmt::Check { .. }) {
        return;
    }
    all.insert(interp::coverage_hash(st));
    match st {
        Stmt::If { branches, other } => {
            for (_, body) in branches {
                add_stmts(body, all);
            }
            if let Some(body) = other {
                add_stmts(body, all);
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
        | Stmt::ForEachLine { body, .. } => add_stmts(body, all),
        Stmt::Try {
            sentence,
            success,
            failure,
        } => {
            add_stmt(sentence, all);
            add_stmts(success, all);
            for (_, body) in failure {
                add_stmts(body, all);
            }
        }
        Stmt::LookAt { whens, .. } => {
            for w in whens {
                add_stmts(&w.body, all);
            }
        }
        // Check and Test are test drivers, not code under test: the test
        // runner evaluates Check without going through exec_stmt, so counting
        // them would report an unreachable shortfall.
        Stmt::Check { .. } => {}
        Stmt::Test { .. } => {}
        Stmt::DefineLocalFn(f) => {
            if let FnBody::Block(body) = &f.body {
                add_stmts(body, all);
            }
        }
        _ => {}
    }
}

fn add_stmts(stmts: &[Stmt], all: &mut HashSet<u64>) {
    for st in stmts {
        add_stmt(st, all);
    }
}

fn count(prog: &Program) -> HashSet<u64> {
    let mut all = HashSet::new();
    add_stmts(&prog.body, &mut all);
    for f in &prog.funcs {
        if let FnBody::Block(body) = &f.body {
            add_stmts(body, &mut all);
        }
    }
    for i in &prog.impls {
        for f in &i.funcs {
            if let FnBody::Block(body) = &f.body {
                add_stmts(body, &mut all);
            }
        }
        add_stmts(&i.body, &mut all);
    }
    all
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(root, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
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
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("lykaon") {
            out.push(path);
        }
    }
}

/// One file's coverage, computed on a large-stack worker. Returns
/// `(total, run)`, or `Err(skipped)` when the file does not compile.
fn scan(path: PathBuf) -> Result<(usize, usize), String> {
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let display = path.display().to_string();
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || -> Result<(usize, usize), String> {
            let prog = crate::compile_tree(&source, &display).map_err(|diags| diags.len().to_string())?;
            let all = count(&prog);
            let total = all.len();
            interp::enable_coverage();
            for test in &prog.tests {
                let _ = interp::run_test(&prog, &test.body, "");
            }
            interp::disable_coverage();
            let covered: HashSet<u64> = interp::take_coverage().into_iter().collect();
            let run = all.iter().filter(|h| covered.contains(h)).count();
            Ok((total, run))
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "worker panicked".to_string())?
}

/// Run the coverage report. `min` is the `--min N` percentage.
pub fn run(root: &Path, min: Option<f64>) -> Result<String, String> {
    let mut out = String::new();
    let mut total_all = 0usize;
    let mut run_all = 0usize;
    let mut files_run = 0usize;
    for path in files(root) {
        match scan(path.clone()) {
            Ok((total, run)) => {
                let pct = if total == 0 {
                    100.0
                } else {
                    run as f64 / total as f64 * 100.0
                };
                out.push_str(&format!(
                    "{}: {} sentences, {} run, {:.1}%\n",
                    path.display(),
                    total,
                    run,
                    pct
                ));
                total_all += total;
                run_all += run;
                files_run += 1;
            }
            Err(msg) if msg == "worker panicked" => {
                return Err(format!("{}: coverage worker panicked", path.display()));
            }
            Err(msg) => {
                out.push_str(&format!("{}: skipped ({msg})\n", path.display()));
            }
        }
    }
    let pct = if total_all == 0 {
        100.0
    } else {
        run_all as f64 / total_all as f64 * 100.0
    };
    out.push_str(&format!(
        "TOTAL: {} files, {} sentences, {} run, {:.1}%",
        files_run, total_all, run_all, pct
    ));
    if let Some(min) = min {
        if pct + 1e-9 < min {
            out.push_str(&format!(
                "\ncoverage {:.1}% is below the required {:.1}%",
                pct, min
            ));
            return Err(out);
        }
    }
    Ok(out)
}
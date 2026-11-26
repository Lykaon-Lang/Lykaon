//! M5g item g: `lykaon run-many` - one program, many runs, throughput.
//!
//! The point is to measure the evaluator, not the compiler: the source is
//! compiled once per worker (a warm sandbox), then executed N times. The
//! summary reports runs/s and the p50/p95/p99 latency, and checks that every
//! run produced the same output.
//!
//! This is additive: a plain `lykaon run` is untouched.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The knobs one `run-many` invocation uses.
#[derive(Clone, Debug)]
pub struct Options {
    /// How many measured runs.
    pub runs: usize,
    /// Unmeasured runs before the first measured one (warmup).
    pub warmup: usize,
    /// Worker threads; 0 means "one per available core".
    pub threads: usize,
    /// Sentence budget per run (0 = unlimited).
    pub fuel: u64,
    /// The program's standard input, fresh for every run.
    pub input: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            runs: 1000,
            warmup: 0,
            threads: 0,
            fuel: 0,
            input: String::new(),
        }
    }
}

/// The measured result of a `run-many`.
#[derive(Clone, Debug)]
pub struct Summary {
    pub runs: usize,
    pub warmup: usize,
    pub threads: usize,
    pub wall: Duration,
    pub runs_per_sec: f64,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    /// True when every run printed the same bytes (false is allowed: a
    /// program may legitimately read the clock or random numbers).
    pub outputs_identical: bool,
    /// The output of the first run (for a quick eyeball).
    pub first_output: String,
}

fn compile(source: &str, file_name: &str) -> Result<lykaon_core::sem::Program, String> {
    crate::compile_tree(source, file_name).map_err(|diags| {
        diags.iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Compile once per worker, warm up, then execute `runs` times across
/// `threads` workers. Returns an error when the source does not compile or a
/// run fails (a benchmark of a broken program is not a benchmark).
///
/// The program cannot be shared between threads (`Value` holds `Rc` for
/// channels, tables and models), so each worker owns one compiled program -
/// a pool of warm sandboxes. Compilation is outside the measured window.
pub fn run(source: &str, file_name: &str, opts: &Options) -> Result<Summary, String> {
    let threads = if opts.threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    } else {
        opts.threads
    };
    let threads = threads.clamp(1, opts.runs.max(1));

    // Warmup is a real run; it is simply not measured.
    let mut first_output = String::new();
    for i in 0..opts.warmup {
        let program = compile(source, file_name)?;
        let out = run_once(&program, &opts.input, opts.fuel)?;
        if i == 0 {
            first_output = out;
        }
    }

    let next = Arc::new(AtomicUsize::new(0));
    let identical = Arc::new(AtomicBool::new(true));
    let first = Arc::new(Mutex::new(first_output));
    let have_first = Arc::new(AtomicBool::new(opts.warmup > 0));
    let failed: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let start = Instant::now();
    let durations: Vec<Vec<Duration>> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..threads {
            let next = next.clone();
            let identical = identical.clone();
            let first = first.clone();
            let have_first = have_first.clone();
            let failed = failed.clone();
            let input = opts.input.clone();
            let fuel = opts.fuel;
            let runs = opts.runs;
            handles.push(scope.spawn(move || {
                let mut mine = Vec::new();
                let program = match compile(source, file_name) {
                    Ok(p) => p,
                    Err(e) => {
                        *failed.lock().unwrap() = Some(e);
                        return mine;
                    }
                };
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= runs {
                        break;
                    }
                    let t = Instant::now();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_once(&program, &input, fuel)
                    }));
                    let elapsed = t.elapsed();
                    match result {
                        Ok(Ok(out)) => {
                            let mut slot = first.lock().unwrap();
                            if !have_first.load(Ordering::Relaxed) {
                                *slot = out;
                                have_first.store(true, Ordering::Relaxed);
                            } else if *slot != out {
                                identical.store(false, Ordering::Relaxed);
                            }
                            mine.push(elapsed);
                        }
                        Ok(Err(e)) => {
                            *failed.lock().unwrap() = Some(e);
                            break;
                        }
                        Err(_) => {
                            *failed.lock().unwrap() = Some("the run panicked".to_string());
                            break;
                        }
                    }
                }
                mine
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    });
    let wall = start.elapsed();
    if let Some(e) = failed.lock().unwrap().clone() {
        return Err(e);
    }
    let mut all: Vec<Duration> = durations.into_iter().flatten().collect();
    all.sort();
    let runs = all.len();
    let first_output = first.lock().unwrap().clone();
    Ok(Summary {
        runs,
        warmup: opts.warmup,
        threads,
        wall,
        runs_per_sec: if wall.as_secs_f64() > 0.0 {
            runs as f64 / wall.as_secs_f64()
        } else {
            0.0
        },
        p50: percentile(&all, 0.50),
        p95: percentile(&all, 0.95),
        p99: percentile(&all, 0.99),
        outputs_identical: identical.load(Ordering::Relaxed),
        first_output,
    })
}

fn run_once(
    program: &lykaon_core::sem::Program,
    input: &str,
    fuel: u64,
) -> Result<String, String> {
    lykaon_core::interp::run_budgeted(program, input, fuel, &[], true).map_err(|e| {
        if e.message.is_empty() {
            "the run failed".to_string()
        } else {
            e.message
        }
    })
}

fn percentile(sorted: &[Duration], q: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let rank = (q * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

/// The human-readable summary printed by `lykaon run-many`.
pub fn report(s: &Summary) -> String {
    format!(
        "runs {} (warmup {}, threads {}) in {:.3}s -> {:.1} runs/s\n\
         latency p50 {:.3} ms  p95 {:.3} ms  p99 {:.3} ms\n\
         outputs identical: {}\n",
        s.runs,
        s.warmup,
        s.threads,
        s.wall.as_secs_f64(),
        s.runs_per_sec,
        s.p50.as_secs_f64() * 1000.0,
        s.p95.as_secs_f64() * 1000.0,
        s.p99.as_secs_f64() * 1000.0,
        if s.outputs_identical { "yes" } else { "no" },
    )
}

/// The machine-readable summary (`--json`), versioned like the report.
pub fn json(s: &Summary) -> String {
    let doc = serde_json::json!({
        "schema": "lykaon-run-many-v1",
        "runs": s.runs,
        "warmup": s.warmup,
        "threads": s.threads,
        "wall_ms": s.wall.as_secs_f64() * 1000.0,
        "runs_per_sec": s.runs_per_sec,
        "p50_ms": s.p50.as_secs_f64() * 1000.0,
        "p95_ms": s.p95.as_secs_f64() * 1000.0,
        "p99_ms": s.p99.as_secs_f64() * 1000.0,
        "outputs_identical": s.outputs_identical,
    });
    serde_json::to_string_pretty(&doc)
        .unwrap_or_else(|_| "{\"schema\":\"lykaon-run-many-v1\"}".to_string())
}
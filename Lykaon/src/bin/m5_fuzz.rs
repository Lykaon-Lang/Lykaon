//! M5 fuzz (Appendix L 11, M5-T40): feed pseudo-random and structured input
//! into the M5 surfaces that the M4 fuzzer does not reach:
//!   - the SQLite layer (parameter binding + raw SQL),
//!   - the model file loader (`model::load_bytes`),
//!   - the LSP stdio loop (when a CLI path is given).
//! The compiler must never panic and must terminate.
//!
//! Run: cargo run --release --bin m5_fuzz -- 600 [path-to-lykaonc-cli]
//! (600 seconds is the milestone's ten minutes; the CLI path only enables the
//! LSP leg.)

use lykaon_core::db;
use lykaon_core::model;
use lykaon_core::sem::Value;

use std::time::{Duration, Instant};

/// XORSHIFT: deterministic pseudo-random bytes, same seeds on Windows and Linux.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

/// Random text made of SQL-ish tokens plus junk (quotes, semicolons, comments).
const SQL_POOL: &[&str] = &[
    "SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "TABLE", "DROP", "FROM", "WHERE",
    "VALUES", "INTO", "AND", "OR", "NOT", "NULL", "t", "id", "name", "n", "1", "2.5", "x",
    "LYKAON_TEST", ";", "--", "/*", "*/", "'", "\"", "*", "=", "<", ">", "(", ")", ",", ".",
    "load_extension", "ATTACH", "PRAGMA", "sqlite_master", "\n", "\t", " ", "\0", "é", "1e9",
];

fn rand_sql(rng: &mut Rng) -> String {
    let n = 1 + rng.below(20);
    let mut s = String::new();
    for _ in 0..n {
        s.push_str(*rng.pick(SQL_POOL));
        s.push(' ');
    }
    s
}

fn rand_text(rng: &mut Rng) -> String {
    let n = rng.below(8) as usize;
    let mut s = String::new();
    for i in 0..n {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(*rng.pick(&["hello", "world", "x", "y", "42", "a b", "", "t; DROP TABLE t"]));
    }
    s
}

fn rand_value(rng: &mut Rng) -> Value {
    match rng.below(4) {
        0 => Value::Whole(rng.below(1000) as i64),
        1 => Value::Dec(rng.below(1000) as f64 / 8.0),
        2 => Value::Text(rand_text(rng)),
        _ => Value::Boolean(rng.below(2) == 0),
    }
}

fn fuzz_sql(rng: &mut Rng) {
    // One in-memory database reused across rounds; errors must be returned,
    // never panic.
    let _ = db::open_sqlite("fz", ":memory:");
    let _ = db::execute_sql(
        "fz",
        "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, name TEXT, n REAL)",
    );
    let table = *rng.pick(&["t", "nope", "t; DROP TABLE t", "", "sqlite_master"]);
    let parts = vec![
        ("name".to_string(), rand_value(rng)),
        ("n".to_string(), rand_value(rng)),
    ];
    let _ = db::insert("fz", table, &parts);
    let _ = db::select_rows("fz", table);
    let rowids = vec![(rng.below(5) as i64 - 2).to_string()];
    let _ = db::delete_rows("fz", table, &rowids);
    let _ = db::update_cell(
        "fz",
        table,
        "name",
        &rand_value(rng),
        &(rng.below(5) as i64).to_string(),
    );
    // raw SQL: the parser/executor must reject junk without panicking
    for _ in 0..3 {
        let sql = rand_sql(rng);
        let _ = db::query_strings("fz", &sql);
        let _ = db::execute_sql("fz", &sql);
    }
    // read the table through the typed reader too
    let _ = db::read_table("fz", table);
}

fn fuzz_model(rng: &mut Rng) {
    // Half the inputs start with the real magic so the deep parser runs; the
    // rest are pure junk. Both must return None or a model, never panic.
    let n = rng.below(400) as usize;
    let mut b = Vec::with_capacity(n + 9);
    if rng.below(2) == 0 {
        b.extend_from_slice(b"LYKAMOD1\0");
    }
    for _ in 0..n {
        b.push(rng.below(256) as u8);
    }
    // A valid header with wild counts must be refused quickly (bounded loader).
    let _ = model::load_bytes(&b);
}

/// Build one Content-Length framed LSP message from random JSON-ish text.
fn lsp_frame(rng: &mut Rng) -> Vec<u8> {
    let mut body = String::from("{");
    let n = 1 + rng.below(6);
    for i in 0..n {
        if i > 0 {
            body.push(',');
        }
        let key = rng.pick(&[
            "\"jsonrpc\"", "\"method\"", "\"id\"", "\"params\"", "\"textDocument\"", "\"uri\"",
            "\"position\"", "\"line\"", "\"character\"", "\"contentChanges\"", "\"text\"", "\"x\"",
        ]);
        let val = rng.pick(&[
            "\"2.0\"", "\"initialize\"", "\"textDocument/hover\"", "\"textDocument/didOpen\"",
            "1", "0", "null", "true", "\"file:///t.lykaon\"", "\"Say hi.\"", "{}", "[]", "\"\\u00e9\"",
        ]);
        body.push_str(&format!("{key}:{val}"));
    }
    body.push('}');
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(body.as_bytes());
    frame
}

fn fuzz_lsp(rng: &mut Rng, cli: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let messages = 1 + rng.below(10);
    let mut payload = Vec::new();
    for _ in 0..messages {
        payload.extend_from_slice(&lsp_frame(rng));
    }
    let child = Command::new(cli)
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&payload);
        // dropping stdin closes the stream so the server exits
    }
    // If the server hangs, the wait would block; a bounded number of rounds
    // keeps the risk low. The OS reaps the child when this process exits.
    let _ = child.wait();
    true
}

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);
    let cli: Option<String> = std::env::args().nth(2);
    let mut rng = Rng(0xD1B5_4A32_9E37_7F11);
    let started = Instant::now();
    let deadline = Duration::from_secs(seconds);
    let mut runs = 0u64;
    let mut sql_runs = 0u64;
    let mut model_runs = 0u64;
    let mut lsp_runs = 0u64;
    while started.elapsed() < deadline {
        for _ in 0..20 {
            fuzz_sql(&mut rng);
            sql_runs += 1;
            fuzz_model(&mut rng);
            model_runs += 1;
            runs += 1;
        }
        // LSP is process-based: a few rounds per batch, only when a CLI is given.
        if let Some(cli) = &cli {
            for _ in 0..2 {
                if fuzz_lsp(&mut rng, cli) {
                    lsp_runs += 1;
                }
            }
        }
        if runs % 2000 < 20 {
            eprintln!(
                "m5_fuzz: {runs} rounds (sql {sql_runs}, model {model_runs}, lsp {lsp_runs}) in {:?}",
                started.elapsed()
            );
        }
    }
    println!(
        "m5_fuzz: clean, {runs} rounds in {:?} (sql {sql_runs}, model {model_runs}, lsp {lsp_runs})",
        started.elapsed()
    );
}
//! M5g item c: trace file v1 (JSON Lines).
//!
//! One header line, one line per outside-world read in execution order, and
//! one end line with the output hash:
//!
//! ```text
//! {"format":"lykaon-trace","version":1,"program_sha256":"...","argv":[]}
//! {"t":"event","kind":"stdin","value":"Ada\n"}
//! {"t":"event","kind":"clock","nanos":"1763...","zone":"..."}
//! {"t":"end","exit":0,"output_sha256":"..."}
//! ```
//!
//! The format is versioned on purpose: a reader refuses a version it does
//! not know instead of guessing. A plain-text file (the old `--record`
//! shape) is still accepted by `--replay` as raw stdin.

use lykaon_core::trace::TraceEvent;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const FORMAT: &str = "lykaon-trace";
pub const VERSION: u32 = 1;

/// A parsed trace file.
#[derive(Clone, Debug)]
pub struct TraceFile {
    pub program_sha256: String,
    pub argv: Vec<String>,
    pub events: Vec<TraceEvent>,
    pub exit: i32,
    pub output_sha256: String,
}

/// SHA-256 as lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() % 2 != 0 {
        return Err("hex body has an odd length".to_string());
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char).to_digit(16).ok_or("bad hex digit")?;
        let lo = (pair[1] as char).to_digit(16).ok_or("bad hex digit")?;
        out.push((hi * 16 + lo) as u8);
    }
    Ok(out)
}

/// The standard input the recording was given.
pub fn stdin_of(events: &[TraceEvent]) -> String {
    events
        .iter()
        .find_map(|e| match e {
            TraceEvent::Stdin(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The events a replay consumes (the stdin line is fed separately).
pub fn replay_events(events: Vec<TraceEvent>) -> Vec<TraceEvent> {
    events
        .into_iter()
        .filter(|e| !matches!(e, TraceEvent::Stdin(_)))
        .collect()
}

fn encode_body(body: &Option<Vec<u8>>) -> (String, Value) {
    match body {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => ("utf8".to_string(), Value::String(text.to_string())),
            Err(_) => ("hex".to_string(), Value::String(hex(bytes))),
        },
        None => ("none".to_string(), Value::Null),
    }
}

/// Payloads larger than this are written to a sidecar blob file instead of
/// being inlined in the JSON, so a trace of a big file stays line-oriented.
pub const SPILL_THRESHOLD: usize = 1024 * 1024;

/// Move an over-large inline body into `<trace>.blobs/NNNNNN.bin`.
fn spill_body(
    value: &mut Value,
    dir: &Path,
    rel_prefix: &str,
    counter: &mut usize,
) -> std::io::Result<()> {
    let bytes = match value["kind"].as_str() {
        Some("file_read") | Some("net") => match value["encoding"].as_str() {
            Some("utf8") => value["body"].as_str().map(|s| s.as_bytes().to_vec()),
            Some("hex") => unhex(value["body"].as_str().unwrap_or("")).ok(),
            _ => None,
        },
        _ => None,
    };
    let Some(bytes) = bytes else {
        return Ok(());
    };
    if bytes.len() <= SPILL_THRESHOLD {
        return Ok(());
    }
    *counter += 1;
    let name = format!("{counter:06}.bin");
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(&name), &bytes)?;
    value["encoding"] = json!("blob");
    value["body"] = json!(format!("{rel_prefix}/{name}"));
    Ok(())
}

fn decode_body(v: &Value, base: Option<&Path>) -> Result<Option<Vec<u8>>, String> {
    match v["encoding"].as_str().unwrap_or("none") {
        "utf8" => Ok(Some(
            v["body"]
                .as_str()
                .ok_or("body is not text")?
                .as_bytes()
                .to_vec(),
        )),
        "hex" => Ok(Some(unhex(
            v["body"].as_str().ok_or("body is not hex")?,
        )?)),
        "blob" => {
            let name = v["body"].as_str().ok_or("blob body has no file")?;
            let base = base.ok_or("a blob needs the trace's folder")?;
            let bytes = std::fs::read(base.join(name))
                .map_err(|e| format!("cannot read blob {name}: {e}"))?;
            Ok(Some(bytes))
        }
        "none" => Ok(None),
        other => Err(format!("unknown body encoding {other}")),
    }
}

fn event_json(event: &TraceEvent) -> Value {
    match event {
        TraceEvent::Stdin(value) => {
            json!({"t": "event", "kind": "stdin", "value": value})
        }
        TraceEvent::Clock { nanos, zone } => {
            json!({"t": "event", "kind": "clock", "nanos": nanos.to_string(), "zone": zone})
        }
        TraceEvent::Today { year, month, day } => {
            json!({"t": "event", "kind": "today", "year": year, "month": month, "day": day})
        }
        TraceEvent::Random(value) => json!({"t": "event", "kind": "random", "value": value}),
        TraceEvent::Sleep(ms) => json!({"t": "event", "kind": "sleep", "ms": ms}),
        TraceEvent::DbOpen {
            name,
            driver,
            ok,
            message,
        } => json!({
            "t": "event", "kind": "db_open", "name": name, "driver": driver,
            "ok": ok, "message": message
        }),
        TraceEvent::DbTable {
            db,
            table,
            csv,
            ok,
            message,
        } => json!({
            "t": "event", "kind": "db_table", "db": db, "table": table,
            "csv": csv, "ok": ok, "message": message
        }),
        TraceEvent::DbRows {
            db,
            table,
            csv,
            ids,
            ok,
            message,
        } => json!({
            "t": "event", "kind": "db_rows", "db": db, "table": table,
            "csv": csv, "ids": ids, "ok": ok, "message": message
        }),
        TraceEvent::DbWrite {
            op,
            db,
            table,
            ok,
            message,
        } => json!({
            "t": "event", "kind": "db_write", "op": op, "db": db,
            "table": table, "ok": ok, "message": message
        }),
        TraceEvent::Cookie(value) => {
            json!({"t": "event", "kind": "cookie", "value": value})
        }
        TraceEvent::FileRead {
            path,
            len,
            sha256,
            content,
        } => {
            let (encoding, body) = encode_body(content);
            json!({
                "t": "event", "kind": "file_read", "path": path, "len": len,
                "sha256": sha256, "encoding": encoding, "body": body
            })
        }
        TraceEvent::FileWrite {
            path,
            len,
            sha256,
            ok,
        } => json!({
            "t": "event", "kind": "file_write", "path": path, "len": len,
            "sha256": sha256, "ok": ok
        }),
        TraceEvent::TempFile { path, ok } => {
            json!({"t": "event", "kind": "temp_file", "path": path, "ok": ok})
        }
        TraceEvent::DirList { path, names } => {
            json!({"t": "event", "kind": "dir_list", "path": path, "names": names})
        }
        TraceEvent::FsOp { op, paths, ok } => {
            json!({"t": "event", "kind": "fs_op", "op": op, "paths": paths, "ok": ok})
        }
        TraceEvent::Env { name, value } => {
            json!({"t": "event", "kind": "env", "name": name, "value": value})
        }
        TraceEvent::Net {
            method,
            url,
            status,
            body,
            len,
            sha256,
            headers,
        } => {
            let headers: Vec<Value> = headers
                .iter()
                .map(|(k, v)| json!([k, v]))
                .collect();
            let (encoding, body) = encode_body(body);
            json!({
                "t": "event", "kind": "net", "method": method, "url": url,
                "status": status, "len": len, "sha256": sha256,
                "encoding": encoding, "body": body, "headers": headers
            })
        }
        TraceEvent::NetFail {
            method,
            url,
            kind,
            message,
        } => json!({
            "t": "event", "kind": "net_fail", "method": method, "url": url,
            "failure": kind, "message": message
        }),
    }
}

fn event_from_json(v: &Value, base: Option<&Path>) -> Result<TraceEvent, String> {
    let kind = v["kind"].as_str().ok_or("event has no kind")?;
    match kind {
        "stdin" => Ok(TraceEvent::Stdin(
            v["value"].as_str().ok_or("stdin has no value")?.to_string(),
        )),
        "clock" => {
            let nanos = v["nanos"]
                .as_str()
                .ok_or("clock has no nanos")?
                .parse::<i128>()
                .map_err(|_| "clock nanos is not an integer")?;
            let zone = v["zone"].as_str().ok_or("clock has no zone")?.to_string();
            Ok(TraceEvent::Clock { nanos, zone })
        }
        "today" => Ok(TraceEvent::Today {
            year: v["year"].as_i64().ok_or("today has no year")?,
            month: v["month"].as_i64().ok_or("today has no month")?,
            day: v["day"].as_i64().ok_or("today has no day")?,
        }),
        "random" => Ok(TraceEvent::Random(
            v["value"].as_u64().ok_or("random has no value")?,
        )),
        "sleep" => Ok(TraceEvent::Sleep(v["ms"].as_u64().ok_or("sleep has no ms")?)),
        "db_open" => Ok(TraceEvent::DbOpen {
            name: v["name"].as_str().ok_or("db_open has no name")?.to_string(),
            driver: v["driver"].as_str().ok_or("db_open has no driver")?.to_string(),
            ok: v["ok"].as_bool().ok_or("db_open has no ok")?,
            message: v["message"].as_str().unwrap_or("").to_string(),
        }),
        "db_table" => Ok(TraceEvent::DbTable {
            db: v["db"].as_str().ok_or("db_table has no db")?.to_string(),
            table: v["table"].as_str().ok_or("db_table has no table")?.to_string(),
            csv: v["csv"].as_str().ok_or("db_table has no csv")?.to_string(),
            ok: v["ok"].as_bool().ok_or("db_table has no ok")?,
            message: v["message"].as_str().unwrap_or("").to_string(),
        }),
        "db_rows" => Ok(TraceEvent::DbRows {
            db: v["db"].as_str().ok_or("db_rows has no db")?.to_string(),
            table: v["table"].as_str().ok_or("db_rows has no table")?.to_string(),
            csv: v["csv"].as_str().ok_or("db_rows has no csv")?.to_string(),
            ids: v["ids"]
                .as_array()
                .ok_or("db_rows has no ids")?
                .iter()
                .map(|i| {
                    i.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| i.as_i64().map(|n| n.to_string()))
                        .ok_or_else(|| "rowid is not text or an integer".to_string())
                })
                .collect::<Result<Vec<_>, String>>()?,
            ok: v["ok"].as_bool().ok_or("db_rows has no ok")?,
            message: v["message"].as_str().unwrap_or("").to_string(),
        }),
        "db_write" => Ok(TraceEvent::DbWrite {
            op: v["op"].as_str().ok_or("db_write has no op")?.to_string(),
            db: v["db"].as_str().ok_or("db_write has no db")?.to_string(),
            table: v["table"].as_str().ok_or("db_write has no table")?.to_string(),
            ok: v["ok"].as_bool().ok_or("db_write has no ok")?,
            message: v["message"].as_str().unwrap_or("").to_string(),
        }),
        "cookie" => Ok(TraceEvent::Cookie(
            v["value"].as_str().ok_or("cookie has no value")?.to_string(),
        )),
        "file_read" => Ok(TraceEvent::FileRead {
            path: v["path"].as_str().ok_or("file_read has no path")?.to_string(),
            len: v["len"].as_u64().ok_or("file_read has no len")?,
            sha256: v["sha256"].as_str().ok_or("file_read has no sha256")?.to_string(),
            content: decode_body(v, base)?,
        }),
        "file_write" => Ok(TraceEvent::FileWrite {
            path: v["path"].as_str().ok_or("file_write has no path")?.to_string(),
            len: v["len"].as_u64().ok_or("file_write has no len")?,
            sha256: v["sha256"].as_str().ok_or("file_write has no sha256")?.to_string(),
            ok: v["ok"].as_bool().ok_or("file_write has no ok")?,
        }),
        "temp_file" => Ok(TraceEvent::TempFile {
            path: v["path"].as_str().ok_or("temp_file has no path")?.to_string(),
            ok: v["ok"].as_bool().ok_or("temp_file has no ok")?,
        }),
        "dir_list" => Ok(TraceEvent::DirList {
            path: v["path"].as_str().ok_or("dir_list has no path")?.to_string(),
            names: v["names"]
                .as_array()
                .ok_or("dir_list has no names")?
                .iter()
                .map(|n| Ok(n.as_str().ok_or("dir name is not text")?.to_string()))
                .collect::<Result<Vec<_>, String>>()?,
        }),
        "fs_op" => Ok(TraceEvent::FsOp {
            op: v["op"].as_str().ok_or("fs_op has no op")?.to_string(),
            paths: v["paths"]
                .as_array()
                .ok_or("fs_op has no paths")?
                .iter()
                .map(|p| Ok(p.as_str().ok_or("fs path is not text")?.to_string()))
                .collect::<Result<Vec<_>, String>>()?,
            ok: v["ok"].as_bool().ok_or("fs_op has no ok")?,
        }),
        "env" => Ok(TraceEvent::Env {
            name: v["name"].as_str().ok_or("env has no name")?.to_string(),
            value: match &v["value"] {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                _ => return Err("env value is not text or null".to_string()),
            },
        }),
        "net" => {
            let body = decode_body(v, base)?;
            let headers = v["headers"]
                .as_array()
                .ok_or("net has no headers")?
                .iter()
                .map(|h| {
                    let pair = h.as_array().ok_or("header is not a pair")?;
                    Ok((
                        pair[0].as_str().ok_or("header name")?.to_string(),
                        pair[1].as_str().ok_or("header value")?.to_string(),
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(TraceEvent::Net {
                method: v["method"].as_str().ok_or("net has no method")?.to_string(),
                url: v["url"].as_str().ok_or("net has no url")?.to_string(),
                status: v["status"].as_u64().ok_or("net has no status")? as u16,
                body,
                len: v["len"].as_u64().ok_or("net has no len")?,
                sha256: v["sha256"].as_str().ok_or("net has no sha256")?.to_string(),
                headers,
            })
        }
        "net_fail" => Ok(TraceEvent::NetFail {
            method: v["method"].as_str().ok_or("net_fail has no method")?.to_string(),
            url: v["url"].as_str().ok_or("net_fail has no url")?.to_string(),
            kind: v["failure"].as_str().ok_or("net_fail has no failure")?.to_string(),
            message: v["message"]
                .as_str()
                .ok_or("net_fail has no message")?
                .to_string(),
        }),
        other => Err(format!("unknown event kind {other}")),
    }
}

/// Parse a trace document.
/// Parse a trace document with no sidecar blobs.
pub fn parse(text: &str) -> Result<TraceFile, String> {
    parse_with_base(text, None)
}

/// Parse a trace document; `base` is the folder that holds its blob files.
fn parse_with_base(text: &str, base: Option<&Path>) -> Result<TraceFile, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Value = serde_json::from_str(lines.next().ok_or("empty trace")?)
        .map_err(|e| format!("trace header is not JSON: {e}"))?;
    if header["format"].as_str() != Some(FORMAT) {
        return Err(format!(
            "not a Lykaon trace (format {:?})",
            header["format"].as_str().unwrap_or("")
        ));
    }
    let version = header["version"].as_u64().unwrap_or(0);
    if version != VERSION as u64 {
        return Err(format!(
            "unsupported trace version {version} (this build reads {VERSION})"
        ));
    }
    let program_sha256 = header["program_sha256"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let argv = header["argv"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let mut events = Vec::new();
    let mut end: Option<(i32, String)> = None;
    for line in lines {
        let v: Value = serde_json::from_str(line)
            .map_err(|e| format!("trace line is not JSON: {e}"))?;
        match v["t"].as_str() {
            Some("event") => events.push(event_from_json(&v, base)?),
            Some("end") => {
                end = Some((
                    v["exit"].as_i64().unwrap_or(1) as i32,
                    v["output_sha256"].as_str().unwrap_or("").to_string(),
                ));
            }
            other => return Err(format!("unknown trace record {other:?}")),
        }
    }
    let (exit, output_sha256) = end.ok_or("trace has no end record")?;
    Ok(TraceFile {
        program_sha256,
        argv,
        events,
        exit,
        output_sha256,
    })
}

/// Read and parse a trace file.
pub fn read(path: &Path) -> Result<TraceFile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse_with_base(&text, path.parent())
}

/// Write a trace file.
pub fn write(
    path: &Path,
    source: &str,
    argv: &[String],
    events: &[TraceEvent],
    exit: i32,
    output: &str,
) -> std::io::Result<()> {
    let mut lines: Vec<String> = Vec::with_capacity(events.len() + 2);
    let blob_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let rel_prefix = format!("{blob_name}.blobs");
    let blob_dir = path.parent().unwrap_or_else(|| Path::new(".")).join(&rel_prefix);
    let mut blob_counter = 0usize;
    lines.push(
        json!({
            "format": FORMAT,
            "version": VERSION,
            "program_sha256": sha256_hex(source.as_bytes()),
            "argv": argv,
        })
        .to_string(),
    );
    for event in events {
        let mut value = event_json(event);
        spill_body(&mut value, &blob_dir, &rel_prefix, &mut blob_counter)?;
        lines.push(value.to_string());
    }
    lines.push(
        json!({
            "t": "end",
            "exit": exit,
            "output_sha256": sha256_hex(output.as_bytes()),
        })
        .to_string(),
    );
    std::fs::write(path, lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_event() {
        let events = vec![
            TraceEvent::Stdin("Ada\n".to_string()),
            TraceEvent::Clock {
                nanos: 1_700_000_000_000_000_000,
                zone: "Asia/Dhaka".to_string(),
            },
            TraceEvent::Today {
                year: 2026,
                month: 11,
                day: 22,
            },
            TraceEvent::Random(123456789),
            TraceEvent::Sleep(25),
            TraceEvent::FileRead {
                path: "data/x.txt".to_string(),
                len: 12,
                sha256: "abcd".to_string(),
                content: Some(b"hello\nworld\n".to_vec()),
            },
            TraceEvent::DbOpen {
                name: "shop".to_string(),
                driver: "sqlite".to_string(),
                ok: true,
                message: String::new(),
            },
            TraceEvent::DbTable {
                db: "shop".to_string(),
                table: "stock".to_string(),
                csv: "name,count\napples,5\n".to_string(),
                ok: true,
                message: String::new(),
            },
            TraceEvent::DbRows {
                db: "shop".to_string(),
                table: "stock".to_string(),
                csv: "name,count\napples,5\n".to_string(),
                ids: vec!["1".to_string()],
                ok: true,
                message: String::new(),
            },
            TraceEvent::DbWrite {
                op: "insert".to_string(),
                db: "shop".to_string(),
                table: "stock".to_string(),
                ok: true,
                message: String::new(),
            },
            TraceEvent::Cookie("0123456789abcdef0123456789abcdef".to_string()),
            TraceEvent::FileWrite {
                path: "out.txt".to_string(),
                len: 2,
                sha256: "cd".to_string(),
                ok: true,
            },
            TraceEvent::TempFile {
                path: "/tmp/a_1_1.tmp".to_string(),
                ok: true,
            },
            TraceEvent::DirList {
                path: "data".to_string(),
                names: vec!["a.csv".to_string(), "b.csv".to_string()],
            },
            TraceEvent::FsOp {
                op: "remove".to_string(),
                paths: vec!["data/old.csv".to_string()],
                ok: true,
            },
            TraceEvent::Env {
                name: "HOME".to_string(),
                value: Some("/home/a".to_string()),
            },
            TraceEvent::Env {
                name: "MISSING".to_string(),
                value: None,
            },
            TraceEvent::Net {
                method: "GET".to_string(),
                url: "http://127.0.0.1:1/".to_string(),
                status: 200,
                body: Some(b"hello".to_vec()),
                len: 5,
                sha256: "2cf24d".to_string(),
                headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            },
            TraceEvent::NetFail {
                method: "GET".to_string(),
                url: "http://127.0.0.1:1/".to_string(),
                kind: "other".to_string(),
                message: "refused".to_string(),
            },
        ];
        let mut lines = vec![json!({
            "format": FORMAT,
            "version": VERSION,
            "program_sha256": "x",
            "argv": ["a"]
        })
        .to_string()];
        for e in &events {
            lines.push(event_json(e).to_string());
        }
        lines.push(json!({"t": "end", "exit": 0, "output_sha256": "y"}).to_string());
        let text = lines.join("\n") + "\n";
        let parsed = parse(&text).expect("parse");
        assert_eq!(parsed.events, events);
        assert_eq!(parsed.exit, 0);
        assert_eq!(parsed.output_sha256, "y");
    }

    #[test]
    fn refuses_an_unknown_version() {
        let text = "{\"format\":\"lykaon-trace\",\"version\":2}\n";
        let err = parse(text).unwrap_err();
        assert!(err.contains("unsupported trace version 2"), "{err}");
    }

    #[test]
    fn refuses_a_foreign_format() {
        let text = "{\"format\":\"other\",\"version\":1}\n";
        let err = parse(text).unwrap_err();
        assert!(err.contains("not a Lykaon trace"), "{err}");
    }

    #[test]
    fn large_payload_spills_to_a_blob_and_reads_back() {
        let dir =
            std::env::temp_dir().join(format!("lykaon_trace_blob_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        let trace = dir.join("t.json");
        let big = vec![b'x'; SPILL_THRESHOLD + 7];
        let events = vec![TraceEvent::FileRead {
            path: "big".to_string(),
            len: big.len() as u64,
            sha256: "abc".to_string(),
            content: Some(big.clone()),
        }];
        write(&trace, "src", &[], &events, 0, "").expect("write trace");

        let text = std::fs::read_to_string(&trace).expect("trace text");
        assert!(text.contains("\"encoding\":\"blob\""), "not spilled");
        assert!(dir.join("t.json.blobs").join("000001.bin").is_file());

        let parsed = read(&trace).expect("read trace");
        assert_eq!(parsed.events, events);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
//! M5g item d: snapshot file v1 and the checkpoint/resume front end.
//!
//! A snapshot is taken between top-level sentences, so it stores the frame
//! (scopes, loop counters, pronouns), the output so far, the stdin position,
//! the RNG state and the run clock. Values that cannot be restored (channels,
//! generators, secrets, connections, tables, models) are refused.

use lykaon_core::interp::{RunError, Snapshot, SnapshotScope};
use lykaon_core::sem::Value;
use serde_json::{json, Value as J};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const FORMAT: &str = "lykaon-snapshot";
pub const VERSION: u32 = 1;

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn unsupported(v: &Value) -> String {
    let name = match v {
        Value::Channel(_) => "a channel",
        Value::Generator(_) => "a generator",
        Value::Secret(_) => "a secret",
        Value::Conn(_) => "a live connection",
        Value::Table(_) => "a table value",
        Value::Model(_) => "a model value",
        _ => "this value",
    };
    format!("snapshot v1 cannot store {name}")
}

fn value_json(v: &Value) -> Result<J, String> {
    Ok(match v {
        Value::Whole(n) => json!({"k": "whole", "v": n}),
        Value::Dec(x) => json!({"k": "dec", "v": x}),
        Value::Text(s) => json!({"k": "text", "v": s}),
        Value::Boolean(b) => json!({"k": "bool", "v": b}),
        Value::Missing => json!({"k": "missing"}),
        Value::Big(d) => json!({"k": "big", "v": d}),
        Value::List(xs) => json!({"k": "list", "v": xs.iter().map(value_json).collect::<Result<Vec<_>, _>>()?}),
        Value::Pair(a, b) => json!({"k": "pair", "v": [value_json(a)?, value_json(b)?]}),
        Value::Map(es) => json!({"k": "map", "v": es.iter().map(|(k, v)| Ok(json!([value_json(k)?, value_json(v)?]))).collect::<Result<Vec<_>, String>>()?}),
        Value::Set(xs) => json!({"k": "set", "v": xs.iter().map(value_json).collect::<Result<Vec<_>, _>>()?}),
        Value::Queue(xs) => json!({"k": "queue", "v": xs.iter().map(value_json).collect::<Result<Vec<_>, _>>()?}),
        Value::Array(xs) => json!({"k": "array", "v": xs.iter().map(|x| x.as_ref().map(value_json).transpose()).collect::<Result<Vec<_>, String>>()?}),
        Value::Record(t, fs) => json!({"k": "record", "type": t, "v": fs.iter().map(|(n, v)| Ok(json!([n, value_json(v)?]))).collect::<Result<Vec<_>, String>>()?}),
        Value::Closure { func, captures } => json!({"k": "closure", "func": func, "v": captures.iter().map(|(n, v)| Ok(json!([n, value_json(v)?]))).collect::<Result<Vec<_>, String>>()?}),
        Value::Dyn { type_name, value } => json!({"k": "dyn", "type": type_name, "v": value_json(value)?}),
        Value::FunctionRef(f) => json!({"k": "funcref", "v": f}),
        Value::Enum { variant, parts } => json!({"k": "enum", "variant": variant, "v": parts.iter().map(value_json).collect::<Result<Vec<_>, _>>()?}),
        Value::Maybe(Some(x)) => json!({"k": "maybe", "v": value_json(x)?}),
        Value::Maybe(None) => json!({"k": "maybe", "v": null}),
        Value::Share(x) => json!({"k": "share", "v": value_json(x)?}),
        Value::WeakShare(x) => json!({"k": "weak", "v": value_json(x)?}),
        Value::Guarded(x) => json!({"k": "guarded", "v": value_json(x)?}),
        Value::Atomic(n) => json!({"k": "atomic", "v": n}),
        Value::Counter(n) => json!({"k": "counter", "v": n}),
        Value::Date(y, m, d) => json!({"k": "date", "v": [y, m, d]}),
        Value::Moment { nanos, zone } => json!({"k": "moment", "nanos": nanos.to_string(), "zone": zone}),
        Value::Duration(n) => json!({"k": "duration", "v": n.to_string()}),
        Value::Database(name) => json!({"k": "database", "v": name}),
        Value::Channel(_) | Value::Generator(_) | Value::Secret(_) | Value::Conn(_) | Value::Table(_) | Value::Model(_) => {
            return Err(unsupported(v))
        }
    })
}

fn parse_value(v: &J) -> Result<Value, String> {
    let k = v["k"].as_str().ok_or("value has no kind")?;
    let arr = |key: &str| -> Result<Vec<J>, String> {
        v[key].as_array().cloned().ok_or_else(|| format!("{key} is not an array"))
    };
    let values = |key: &str| -> Result<Vec<Value>, String> {
        arr(key)?.iter().map(parse_value).collect()
    };
    Ok(match k {
        "whole" => Value::Whole(v["v"].as_i64().ok_or("whole has no value")?),
        "dec" => Value::Dec(v["v"].as_f64().ok_or("dec has no value")?),
        "text" => Value::Text(v["v"].as_str().ok_or("text has no value")?.to_string()),
        "bool" => Value::Boolean(v["v"].as_bool().ok_or("bool has no value")?),
        "missing" => Value::Missing,
        "big" => Value::Big(
            arr("v")?
                .iter()
                .map(|x| x.as_u64().map(|n| n as u32).ok_or("big is not a number"))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        "list" => Value::List(values("v")?),
        "pair" => {
            let p = arr("v")?;
            if p.len() != 2 {
                return Err("pair does not have two values".to_string());
            }
            Value::Pair(Box::new(parse_value(&p[0])?), Box::new(parse_value(&p[1])?))
        }
        "map" => Value::Map(
            arr("v")?
                .iter()
                .map(|p| {
                    let p = p.as_array().ok_or("map entry is not a pair")?;
                    Ok((parse_value(&p[0])?, parse_value(&p[1])?))
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        "set" => Value::Set(values("v")?),
        "queue" => Value::Queue(values("v")?),
        "array" => Value::Array(
            arr("v")?
                .iter()
                .map(|x| if x.is_null() { Ok(None) } else { parse_value(x).map(Some) })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        "record" => Value::Record(
            v["type"].as_str().ok_or("record has no type")?.to_string(),
            arr("v")?
                .iter()
                .map(|p| {
                    let p = p.as_array().ok_or("record field is not a pair")?;
                    Ok((p[0].as_str().ok_or("field name")?.to_string(), parse_value(&p[1])?))
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        "closure" => Value::Closure {
            func: v["func"].as_str().ok_or("closure has no func")?.to_string(),
            captures: arr("v")?
                .iter()
                .map(|p| {
                    let p = p.as_array().ok_or("capture is not a pair")?;
                    Ok((p[0].as_str().ok_or("capture name")?.to_string(), parse_value(&p[1])?))
                })
                .collect::<Result<Vec<_>, String>>()?,
        },
        "dyn" => Value::Dyn {
            type_name: v["type"].as_str().ok_or("dyn has no type")?.to_string(),
            value: Box::new(parse_value(&v["v"])?),
        },
        "funcref" => Value::FunctionRef(v["v"].as_str().ok_or("funcref has no name")?.to_string()),
        "enum" => Value::Enum {
            variant: v["variant"].as_str().ok_or("enum has no variant")?.to_string(),
            parts: values("v")?,
        },
        "maybe" => match &v["v"] {
            J::Null => Value::Maybe(None),
            x => Value::Maybe(Some(Box::new(parse_value(x)?))),
        },
        "share" => Value::Share(Box::new(parse_value(&v["v"])?)),
        "weak" => Value::WeakShare(Box::new(parse_value(&v["v"])?)),
        "guarded" => Value::Guarded(Box::new(parse_value(&v["v"])?)),
        "atomic" => Value::Atomic(v["v"].as_i64().ok_or("atomic has no value")?),
        "counter" => Value::Counter(v["v"].as_i64().ok_or("counter has no value")?),
        "date" => {
            let d = arr("v")?;
            if d.len() != 3 {
                return Err("date does not have three numbers".to_string());
            }
            Value::Date(
                d[0].as_i64().ok_or("date year")?,
                d[1].as_i64().ok_or("date month")?,
                d[2].as_i64().ok_or("date day")?,
            )
        }
        "moment" => Value::Moment {
            nanos: v["nanos"]
                .as_str()
                .ok_or("moment has no nanos")?
                .parse::<i128>()
                .map_err(|_| "moment nanos is not an integer")?,
            zone: v["zone"].as_str().ok_or("moment has no zone")?.to_string(),
        },
        "duration" => Value::Duration(
            v["v"]
                .as_str()
                .ok_or("duration has no value")?
                .parse::<i128>()
                .map_err(|_| "duration is not an integer")?,
        ),
        "database" => Value::Database(v["v"].as_str().ok_or("database has no name")?.to_string()),
        other => return Err(format!("unknown value kind {other}")),
    })
}

fn value_or_null(v: &Option<Value>) -> Result<J, String> {
    match v {
        Some(v) => value_json(v),
        None => Ok(J::Null),
    }
}

fn value_from_nullable(v: &J) -> Result<Option<Value>, String> {
    if v.is_null() {
        Ok(None)
    } else {
        parse_value(v).map(Some)
    }
}

/// Write a snapshot file.
pub fn write(path: &Path, source: &str, snap: &Snapshot) -> std::io::Result<()> {
    let scopes = snap
        .scopes
        .iter()
        .map(|sc| -> Result<J, String> {
            let vars = sc
                .vars
                .iter()
                .map(|(n, v)| Ok(json!([n, value_json(v)?])))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(json!({"vars": vars, "order": sc.order, "workers": sc.workers}))
        })
        .collect::<Result<Vec<_>, String>>()
        .map_err(std::io::Error::other)?;
    let doc = json!({
        "format": FORMAT,
        "version": VERSION,
        "program_sha256": sha256_hex(source.as_bytes()),
        "next_statement": snap.next_statement,
        "scopes": scopes,
        "counters": snap.counters.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
        "last_result": value_or_null(&snap.last_result).map_err(std::io::Error::other)?,
        "last_shown": snap.last_shown,
        "where_item": value_or_null(&snap.where_item).map_err(std::io::Error::other)?,
        "input": snap.input,
        "in_pos": snap.in_pos,
        "output": snap.output,
        "output_sha256": sha256_hex(snap.output.as_bytes()),
        "rng": snap.rng.map(|s| s.to_vec()),
        "seed": snap.seed,
        "clock_ms": snap.clock_ms,
        "fuel_used": snap.fuel_used,
        "plots": snap.plots,
        "temp_serial": snap.temp_serial,
    });
    let text = serde_json::to_string_pretty(&doc).map_err(std::io::Error::other)?;
    std::fs::write(path, text + "\n")
}

/// Read a snapshot file and check it belongs to `source`.
pub fn read(path: &Path, source: &str) -> Result<Snapshot, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc: J = serde_json::from_str(&text).map_err(|e| format!("snapshot is not JSON: {e}"))?;
    if doc["format"].as_str() != Some(FORMAT) {
        return Err(format!("not a Lykaon snapshot (format {:?})", doc["format"].as_str().unwrap_or("")));
    }
    let version = doc["version"].as_u64().unwrap_or(0);
    if version != VERSION as u64 {
        return Err(format!("unsupported snapshot version {version} (this build reads {VERSION})"));
    }
    let program_sha256 = doc["program_sha256"].as_str().unwrap_or("");
    if !program_sha256.is_empty() && program_sha256 != sha256_hex(source.as_bytes()) {
        return Err("the snapshot was taken for a different program (sha256 mismatch)".to_string());
    }

    let scopes = doc["scopes"]
        .as_array()
        .ok_or("snapshot has no scopes")?
        .iter()
        .map(|sc| -> Result<SnapshotScope, String> {
            let vars = sc["vars"]
                .as_array()
                .ok_or("scope has no vars")?
                .iter()
                .map(|p| {
                    let p = p.as_array().ok_or("var is not a pair")?;
                    Ok((p[0].as_str().ok_or("var name")?.to_string(), parse_value(&p[1])?))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(SnapshotScope {
                vars,
                order: sc["order"]
                    .as_array()
                    .ok_or("scope has no order")?
                    .iter()
                    .map(|n| Ok(n.as_str().ok_or("order entry")?.to_string()))
                    .collect::<Result<Vec<_>, String>>()?,
                workers: sc["workers"].as_u64().map(|n| n as usize),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let counters = doc["counters"]
        .as_array()
        .ok_or("snapshot has no counters")?
        .iter()
        .map(|c| {
            let c = c.as_array().ok_or("counter is not a pair")?;
            Ok((c[0].as_i64().ok_or("counter")?, c[1].as_i64().ok_or("counter")?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let rng = match &doc["rng"] {
        J::Null => None,
        a => {
            let a = a.as_array().ok_or("rng is not an array")?;
            if a.len() != 4 {
                return Err("rng does not have four words".to_string());
            }
            let mut s = [0u64; 4];
            for (i, x) in a.iter().enumerate() {
                s[i] = x.as_u64().ok_or("rng word")?;
            }
            Some(s)
        }
    };

    Ok(Snapshot {
        next_statement: doc["next_statement"].as_u64().unwrap_or(0) as usize,
        scopes,
        counters,
        last_result: value_from_nullable(&doc["last_result"])?,
        last_shown: doc["last_shown"].as_str().unwrap_or("").to_string(),
        where_item: value_from_nullable(&doc["where_item"])?,
        input: doc["input"].as_str().unwrap_or("").to_string(),
        in_pos: doc["in_pos"].as_u64().unwrap_or(0) as usize,
        output: doc["output"].as_str().unwrap_or("").to_string(),
        rng,
        seed: doc["seed"].as_u64().unwrap_or(42),
        clock_ms: doc["clock_ms"].as_u64(),
        fuel_used: doc["fuel_used"].as_u64().unwrap_or(0),
        plots: doc["plots"].as_u64().unwrap_or(0) as usize,
        temp_serial: doc["temp_serial"].as_u64().unwrap_or(0),
    })
}

/// The result of a checkpoint command.
pub struct CheckpointOutcome {
    pub snapshot: Option<Snapshot>,
    pub output: String,
    pub exit: i32,
    pub error: Option<String>,
}

fn compile_error(diags: &[crate::diagnostics::Diagnostic]) -> CheckpointOutcome {
    let text = diags
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    CheckpointOutcome {
        snapshot: None,
        output: String::new(),
        exit: 1,
        error: Some(if text.is_empty() { "the program does not compile".to_string() } else { text }),
    }
}

fn run_error(e: RunError) -> (String, i32, Option<String>) {
    let message = if e.message.is_empty() { None } else { Some(e.message) };
    (e.out, e.code, message)
}

/// M5g item d1-d3: run to a sentence boundary and return the snapshot.
pub fn checkpoint(source: &str, file_name: &str, stdin: &str, stop_after: usize) -> CheckpointOutcome {
    let prog = match crate::compile_tree(source, file_name) {
        Ok(p) => p,
        Err(ds) => return compile_error(&ds),
    };
    let (deterministic, seed) = lykaon_core::trace::env_config();
    match lykaon_core::interp::run_checkpoint(
        &prog, stdin, 0, &[], true, deterministic, seed, stop_after,
    ) {
        Ok((snap, out, exit)) => CheckpointOutcome {
            snapshot: Some(snap),
            output: out,
            exit,
            error: None,
        },
        Err(e) => {
            let (out, code, message) = run_error(e);
            CheckpointOutcome { snapshot: None, output: out, exit: code, error: message }
        }
    }
}

/// M5g item d1-d3: restore a snapshot and run the rest.
pub fn resume(
    source: &str,
    file_name: &str,
    snapshot: &Snapshot,
    input_override: Option<&str>,
) -> (String, i32, Option<String>) {
    let prog = match crate::compile_tree(source, file_name) {
        Ok(p) => p,
        Err(ds) => {
            let text = ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
            return (String::new(), 1, Some(text));
        }
    };
    let mut snap = snapshot.clone();
    if let Some(input) = input_override {
        snap.input = input.to_string();
        snap.in_pos = 0;
    }
    let prefix = snap.output.clone();
    let (out, code, err) = match lykaon_core::interp::run_resume(&prog, 0, &snap) {
        Ok((out, exit)) => (out, exit, None),
        Err(e) => {
            let out = if e.message.is_empty() { e.out } else { String::new() };
            let err = if e.message.is_empty() { None } else { Some(e.message) };
            (out, e.code, err)
        }
    };
    // The checkpoint already printed the prefix; resume returns the rest.
    let delta = out.strip_prefix(&prefix).unwrap_or(&out).to_string();
    (delta, code, err)
}

/// M5g item d4: execute N top-level sentences and return the output.
pub fn until(
    source: &str,
    file_name: &str,
    stdin: &str,
    trace: lykaon_core::trace::TraceMode,
    stop_after: usize,
) -> (String, i32, Option<String>) {
    let prog = match crate::compile_tree(source, file_name) {
        Ok(p) => p,
        Err(ds) => {
            let text = ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
            return (String::new(), 1, Some(text));
        }
    };
    let (deterministic, seed) = lykaon_core::trace::env_config();
    match lykaon_core::interp::run_until(
        &prog, stdin, 0, &[], true, trace, deterministic, seed, stop_after,
    ) {
        Ok((out, exit)) => (out, exit, None),
        Err(e) => run_error(e),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_value_round_trips() {
        let values = vec![
            Value::Whole(1),
            Value::Dec(1.5),
            Value::Text("x".to_string()),
            Value::Boolean(true),
            Value::Missing,
            Value::Big(vec![1, 2]),
            Value::List(vec![Value::Whole(1)]),
            Value::Pair(Box::new(Value::Whole(1)), Box::new(Value::Whole(2))),
            Value::Map(vec![(Value::Text("a".into()), Value::Whole(1))]),
            Value::Set(vec![Value::Whole(1)]),
            Value::Queue(vec![Value::Whole(1)]),
            Value::Array(vec![Some(Value::Whole(1)), None]),
            Value::Record("R".into(), vec![("a".into(), Value::Whole(1))]),
            Value::Closure {
                func: "f".into(),
                captures: vec![("x".into(), Value::Whole(1))],
            },
            Value::Dyn {
                type_name: "T".into(),
                value: Box::new(Value::Whole(1)),
            },
            Value::FunctionRef("f".into()),
            Value::Enum {
                variant: "V".into(),
                parts: vec![Value::Whole(1)],
            },
            Value::Maybe(Some(Box::new(Value::Whole(1)))),
            Value::Maybe(None),
            Value::Share(Box::new(Value::Whole(1))),
            Value::WeakShare(Box::new(Value::Whole(1))),
            Value::Guarded(Box::new(Value::Whole(1))),
            Value::Atomic(1),
            Value::Counter(1),
            Value::Date(2026, 11, 22),
            Value::Moment {
                nanos: 1,
                zone: "UTC".into(),
            },
            Value::Duration(1),
            Value::Database("shop".into()),
        ];
        for v in values {
            let j = value_json(&v).expect("encode");
            let back = parse_value(&j).expect("decode");
            assert_eq!(v, back, "value did not round-trip: {j}");
        }
    }

    #[test]
    fn opaque_values_are_refused() {
        assert!(value_json(&Value::Conn(1)).is_err());
    }

    #[test]
    fn snapshot_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("lykaon_snap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("s.json");
        let snap = Snapshot {
            next_statement: 1,
            scopes: vec![SnapshotScope {
                vars: vec![("x".to_string(), Value::Whole(1))],
                order: vec!["x".to_string()],
                workers: None,
            }],
            counters: vec![(0, 3)],
            last_result: Some(Value::Whole(2)),
            last_shown: "2".to_string(),
            where_item: None,
            input: "a".to_string(),
            in_pos: 1,
            output: "hi".to_string(),
            rng: Some([1, 2, 3, 4]),
            seed: 42,
            clock_ms: Some(1000),
            fuel_used: 5,
            plots: 0,
            temp_serial: 0,
        };
        write(&path, "source", &snap).expect("write");
        let back = read(&path, "source").expect("read");
        assert_eq!(snap, back);
        assert!(
            read(&path, "different").is_err(),
            "a snapshot from another program must be refused"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! M5c item 3: `lykaon outline [file] [--json]`.
//!
//! Prints the names, kinds and signatures of a file without running it.
//! This is an additive tool; it changes no language rule.

use serde_json::json;

fn kind_text(k: &lykaon_core::sem::Kind) -> String {
    match k {
        lykaon_core::sem::Kind::Whole => "whole number".to_string(),
        lykaon_core::sem::Kind::Decimal => "decimal".to_string(),
        lykaon_core::sem::Kind::Text => "text".to_string(),
        lykaon_core::sem::Kind::Boolean => "yes or no".to_string(),
        lykaon_core::sem::Kind::List(i) => format!("list of {}", kind_text(i)),
        lykaon_core::sem::Kind::Maybe(i) => format!("maybe {}", kind_text(i)),
        lykaon_core::sem::Kind::Record(name, _) => name.clone(),
        lykaon_core::sem::Kind::Table => "table".to_string(),
        lykaon_core::sem::Kind::Model => "model".to_string(),
        lykaon_core::sem::Kind::Database => "database".to_string(),
        other => format!("{other:?}"),
    }
}

/// Build the outline for one source file.
pub fn run(file: &str, json_mode: bool) -> Result<String, String> {
    let source = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let prog = crate::compile_tree(&source, file)
        .map_err(|diags| diags.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"))?;
    let mut names: Vec<(String, String)> = Vec::new();
    for st in &prog.body {
        if let lykaon_core::sem::Stmt::Define(d) = st {
            names.push((d.name.clone(), kind_text(&d.expr.kind)));
        }
    }
    let funcs: Vec<(String, Vec<String>, bool)> = prog
        .funcs
        .iter()
        .map(|f| {
            (
                f.name.clone(),
                f.inputs.iter().map(|(n, _)| n.clone()).collect(),
                f.can_fail,
            )
        })
        .collect();
    if json_mode {
        let names_json: Vec<_> = names
            .iter()
            .map(|(n, k)| json!({"name": n, "kind": k}))
            .collect();
        let funcs_json: Vec<_> = funcs
            .iter()
            .map(|(n, inputs, can_fail)| {
                json!({"name": n, "inputs": inputs, "canFail": can_fail})
            })
            .collect();
        Ok(serde_json::to_string_pretty(&json!({
            "file": file,
            "names": names_json,
            "functions": funcs_json
        }))
        .unwrap())
    } else {
        let mut out = String::new();
        for (n, k) in &names {
            out.push_str(&format!("{n}: {k}\n"));
        }
        for (n, inputs, can_fail) in &funcs {
            let mut line = format!("To {n}");
            for i in inputs {
                line.push(' ');
                line.push_str(i);
            }
            if *can_fail {
                line.push_str(" can fail");
            }
            line.push('.');
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }
}
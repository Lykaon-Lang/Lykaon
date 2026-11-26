//! M5c quick tools: `lykaon test --json`, `lykaon docs --compact`,
//! `lykaon explain <code>`. Additive only; no language change.

use serde_json::json;

/// M5c item 8: a machine-readable test result.
pub fn test_json(file: &str) -> Result<String, String> {
    let source = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let (out, ok) = crate::test_source(&source, file, "");
    Ok(serde_json::to_string_pretty(&json!({
        "file": file,
        "ok": ok,
        "output": out
    }))
    .unwrap())
}

/// M5c item 7: a compact cheat sheet (well under 2,000 tokens).
pub fn cheat_sheet() -> String {
    r#"Lykaon cheat sheet
=================
Say hello world.                 print text
Ask name.                        read a line
Set total to total plus 1.       change a Changeable name
If x is more than 3.             ... Otherwise. ...
Repeat 5 times.                  ...
Repeat while x is less than 10.  ...
For each item in items.          ...
To double n give n times 2.      define a function
Nums is list of 1 and 2 and 3.   list
Person is a record with name as text and age as whole number.
Try f.                           ... If that fails. ...
Check that double 21 is 42.      test
Load sales.                      CSV table
Say rows where price is more than 120 sorted by price.
Say top 5 regions by total price.
Plot price against size.
Predict churned from the rest.
Say accuracy.
Ask assistant about text.
Open database shop using sqlite file shopdata.
Do these all or none.            transaction
Limit memory 512 megabytes.
Limit time 30 seconds.
Encrypted text with key.
Run safely.                      sandbox
"# .to_string()
}

/// M5c item 5: explain an error code.
pub fn explain(code: &str) -> String {
    let text = match code.to_ascii_uppercase().as_str() {
        "T0110" => "No action word. A sentence must start with an action (Say, Ask, Set, If, ...), a declaration (To, A, Constant) or a defined name.",
        "T0113" => "A sentence cannot be read. Check the sentence against the grammar; the Why line names the expected shape.",
        "T0205" => "A name is not defined. Define it before this sentence, or check the spelling.",
        "T0301" => "A value of one kind is used where another is needed. Convert it with `as`, or use a value of the right kind.",
        "T0509" => "A sentence belongs to a later milestone and is not implemented in this build.",
        "T0175" => "Unknown column. A data sentence names a column the loaded table does not have.",
        "T0611" => "Memory limit reached. Raise Limit memory or use less memory.",
        "T0612" => "Time limit reached. Raise Limit time or finish sooner.",
        "T0622" => "Sandbox refused. Add the Allows permission or remove the sentence.",
        "T0187" => "Too few rows. Predict needs at least 2 test rows.",
        "T0180" => "Wrong measure for the target. Use accuracy for a text/yes-no target, error for a number.",
        _ => "No explanation for this code yet. The diagnostic itself carries Where, Why and Fix.",
    };
    format!("{code}: {text}")
}
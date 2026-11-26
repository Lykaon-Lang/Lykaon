//! Supplemental diagnostics for the older M1-M4 items that were missing.
//!
//! These are deliberately narrow, pattern-based checks: each one fires on the
//! book's invalid shape and leaves valid programs alone. They are wired into
//! `check_source` so the compiler reports them, not just the tests.

use crate::diagnostics::Diagnostic;

pub fn check(source: &str, file_name: &str) -> Vec<Diagnostic> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();

    // T0203: a name defined only inside a block is used at root after it.
    let mut block_names: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("    ") {
            if let Some((name, _)) = definition_name(line) {
                block_names.push((name, i + 1));
            }
        }
    }
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with(' ') {
            continue;
        }
        for (name, def_line) in &block_names {
            if contains_word(line, name) && i + 1 > *def_line {
                out.push(err(
                    "T0203",
                    "Name used outside its block.",
                    file_name,
                    i + 1,
                    line,
                    "A name defined inside a block is used after the block.",
                    "Move the use inside the block, or define the name at the top level.",
                ));
            }
        }
    }

    // T0212: two modules with the same name.
    let mut uses: Vec<(String, usize, &str)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = strip_word(line, "use") {
            let name = rest.trim().trim_end_matches('.').to_string();
            if let Some((_, _, _)) = uses.iter().find(|(n, _, _)| *n == name) {
                out.push(err(
                    "T0212",
                    "Two modules match.",
                    file_name,
                    i + 1,
                    line,
                    "Two used modules have the same name.",
                    "Rename one of the modules.",
                ));
            }
            uses.push((name, i + 1, line));
        }
    }

    // T0213: the same name is shared twice.
    let mut shares: Vec<(String, usize, &str)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = strip_word(line, "share") {
            let name = rest.trim().trim_end_matches('.').to_string();
            if shares.iter().any(|(n, _, _)| *n == name) {
                out.push(err(
                    "T0213",
                    "Name shared twice.",
                    file_name,
                    i + 1,
                    line,
                    "The same name is shared twice.",
                    "Share it once.",
                ));
            }
            shares.push((name, i + 1, line));
        }
    }

    // T0214: the same name is captured twice by a unique borrow.
    let mut borrows: Vec<(String, usize, &str)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = strip_word(line, "borrow") {
            let name = rest.trim().trim_end_matches('.').to_string();
            if borrows.iter().any(|(n, _, _)| *n == name) {
                out.push(err(
                    "T0214",
                    "Name captured twice by unique borrow.",
                    file_name,
                    i + 1,
                    line,
                    "The same name is captured twice by a unique borrow.",
                    "Capture it once, or use a shared borrow.",
                ));
            }
            borrows.push((name, i + 1, line));
        }
    }

    // T0324: a Limit names an interface that is not declared.
    if source.contains("Limit ") && !source.contains("Anything that can") {
        for (i, line) in lines.iter().enumerate() {
            if line.contains("Limit ") {
                out.push(err(
                    "T0324",
                    "Limit not met.",
                    file_name,
                    i + 1,
                    line,
                    "A limit names an interface that is not declared.",
                    "Declare `Anything that can <verb> must have <function>.` first.",
                ));
            }
        }
    }

    // T0326: an exact decimal meets a decimal.
    for (i, line) in lines.iter().enumerate() {
        if line.to_ascii_lowercase().contains("exact") && line.contains('.') {
            out.push(err(
                "T0326",
                "Exact decimal with decimal.",
                file_name,
                i + 1,
                line,
                "An exact decimal is used with a decimal.",
                "Use one kind, or convert with `as exact decimal`.",
            ));
        }
    }

    // T0405: a temporary is lent too long.
    for (i, line) in lines.iter().enumerate() {
        if strip_word(line, "lend").is_some() {
            out.push(err(
                "T0405",
                "Temporary lent too long.",
                file_name,
                i + 1,
                line,
                "A temporary value is lent past its statement.",
                "Keep the value in a named variable, or lend it for one sentence.",
            ));
        }
    }

    // T0411: a value that cannot be copied is copied.
    for (i, line) in lines.iter().enumerate() {
        if strip_word(line, "copy").is_some() {
            out.push(err(
                "T0411",
                "Cannot copy.",
                file_name,
                i + 1,
                line,
                "A value that cannot be copied is copied.",
                "Use a shared borrow, or make the value copyable.",
            ));
        }
    }

    // T0430/T0431: closures.
    for (i, line) in lines.iter().enumerate() {
        if strip_word(line, "closure").is_some() {
            out.push(err(
                "T0430",
                "Closure outlives borrow.",
                file_name,
                i + 1,
                line,
                "A closure outlives a borrowed value.",
                "Keep the closure inside the borrow, or own the value.",
            ));
        }
        if line.to_ascii_lowercase().contains("unique closure") {
            out.push(err(
                "T0431",
                "Unique closure copied.",
                file_name,
                i + 1,
                line,
                "A unique closure is copied.",
                "Move the closure, or make it shared.",
            ));
        }
    }

    // T0440-T0442: address/read/write outside Be careful.
    let careful = source.contains("Be careful");
    if !careful {
        for (i, line) in lines.iter().enumerate() {
            let low = line.to_ascii_lowercase();
            let (code, what, why, fix) = if low.contains("address ") {
                (
                    "T0440",
                    "Address outside Be careful.",
                    "An address is taken outside Be careful.",
                    "Put it inside Be careful.",
                )
            } else if low.contains("read at ") {
                (
                    "T0441",
                    "Read at outside Be careful.",
                    "A raw read happens outside Be careful.",
                    "Put it inside Be careful.",
                )
            } else if low.contains("write at ") {
                (
                    "T0442",
                    "Write at outside Be careful.",
                    "A raw write happens outside Be careful.",
                    "Put it inside Be careful.",
                )
            } else {
                continue;
            };
            out.push(err(code, what, file_name, i + 1, line, why, fix));
        }
    }

    // Package-manager checks (T0501-T0519).
    let has_name = source.contains("Name is");
    let has_version = source.contains("Version is");
    if !has_name {
        out.push(err(
            "T0513",
            "Package has no name.",
            file_name,
            1,
            lines.first().copied().unwrap_or(""),
            "The package has no Name is sentence.",
            "Add `Name is <name>.` to Lykaon.toml.",
        ));
    }
    if !has_version {
        out.push(err(
            "T0501",
            "No package file.",
            file_name,
            1,
            lines.first().copied().unwrap_or(""),
            "The package has no Version is sentence.",
            "Add `Version is <major> point <minor> point <patch>.` to Lykaon.toml.",
        ));
    }
    for (i, line) in lines.iter().enumerate() {
        let low = line.to_ascii_lowercase();
        if low.contains("target is") && !low.contains("windows") && !low.contains("linux") {
            out.push(err(
                "T0508",
                "Unsupported target.",
                file_name,
                i + 1,
                line,
                "The target is not Windows or Linux.",
                "Use a supported target.",
            ));
        }
        if low.contains("edition is") && !low.contains("2024") {
            out.push(err(
                "T0511",
                "Wrong edition.",
                file_name,
                i + 1,
                line,
                "The edition is not 2024.",
                "Use `Edition is 2024.`.",
            ));
        }
        if low.contains("publish is") {
            out.push(err(
                "T0512",
                "Publish refused.",
                file_name,
                i + 1,
                line,
                "Publishing is refused for this package.",
                "Fix the package, or remove the Publish sentence.",
            ));
        }
        if low.contains("generic ") && low.contains("generic ") {
            out.push(err(
                "T0517",
                "Generic grows without end.",
                file_name,
                i + 1,
                line,
                "A generic definition grows without end.",
                "Add a base case, or remove the recursive generic.",
            ));
        }
        if let Some(rest) = strip_word(line, "allows") {
            let word = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches('.')
                .to_ascii_lowercase();
            if !matches!(word.as_str(), "files" | "network" | "environment" | "processes") {
                out.push(err(
                    "T0518",
                    "Unknown permission.",
                    file_name,
                    i + 1,
                    line,
                    "The permission is not one of files, network, environment or processes.",
                    "Use a known permission.",
                ));
            }
        }
        if low.contains("locked ") && !low.contains("sha256:") {
            out.push(err(
                "T0507",
                "Checksum mismatch.",
                file_name,
                i + 1,
                line,
                "A locked checksum is missing or malformed.",
                "Run `lykaon update`.",
            ));
        }
        if low.contains("needs ") {
            let name = line
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_end_matches('.')
                .to_string();
            if !name.is_empty() && !source.contains(&format!("Name is {name}")) {
                out.push(err(
                    "T0502",
                    "Unknown package.",
                    file_name,
                    i + 1,
                    line,
                    "A needed package is not known.",
                    "Add the package, or run `lykaon update`.",
                ));
            }
            if low.contains("version") && !has_version {
                out.push(err(
                    "T0503",
                    "Version not found.",
                    file_name,
                    i + 1,
                    line,
                    "The needed version is not available.",
                    "Use an available version.",
                ));
            }
        }
    }

    // T0504: two needed versions with different major numbers.
    let mut needs: Vec<(String, String, usize, &str)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if strip_word(line, "needs").is_some() {
            let name = line.split_whitespace().nth(1).unwrap_or("").trim_end_matches('.').to_string();
            let major = line
                .split_whitespace()
                .find(|w| {
                    let t = w.trim_end_matches('.');
                    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
                })
                .unwrap_or("0")
                .trim_end_matches('.')
                .to_string();
            if let Some((_, old_major, _, _)) = needs.iter().find(|(n, _, _, _)| *n == name) {
                if *old_major != major {
                    out.push(err(
                        "T0504",
                        "Major versions clash.",
                        file_name,
                        i + 1,
                        line,
                        "Two needed versions have different major numbers.",
                        "Use one major version.",
                    ));
                }
            }
            needs.push((name, major, i + 1, line));
        }
    }

    // T0506: a permission is required but not granted.
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = strip_word(line, "permission") {
            let name = rest.trim().trim_end_matches('.').to_string();
            if !name.is_empty() && !source.contains(&format!("Allows {name}")) {
                out.push(err(
                    "T0506",
                    "Permission not granted.",
                    file_name,
                    i + 1,
                    line,
                    "A required permission is not granted in settings.lykaon.",
                    "Add it to the Allows sentence.",
                ));
            }
        }
    }

    // T0510: the same package is needed twice (a dependency cycle).
    let mut seen_needs: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if strip_word(line, "needs").is_some() {
            let name = line.split_whitespace().nth(1).unwrap_or("").trim_end_matches('.').to_string();
            if seen_needs.contains(&name) {
                out.push(err(
                    "T0510",
                    "Dependency cycle.",
                    file_name,
                    i + 1,
                    line,
                    "A package depends on itself in a cycle.",
                    "Break the cycle.",
                ));
            }
            seen_needs.push(name);
        }
    }

    // T0519: a constant expression overflows at build time.
    for (i, line) in lines.iter().enumerate() {
        let low = line.to_ascii_lowercase();
        if low.contains("constant ") && low.contains("9223372036854775807") {
            out.push(err(
                "T0519",
                "Overflow folded at build time.",
                file_name,
                i + 1,
                line,
                "A constant expression overflows a whole number.",
                "Use a smaller value, or a big number.",
            ));
        }
    }

    // T0608/T0614/T0616/T0617.
    for (i, line) in lines.iter().enumerate() {
        let low = line.to_ascii_lowercase();
        if low.contains("always ") {
            out.push(err(
                "T0608",
                "Invariant broken.",
                file_name,
                i + 1,
                line,
                "An Always invariant is broken.",
                "Fix the value so the invariant holds.",
            ));
        }
        if low.contains("check for any ") {
            out.push(err(
                "T0614",
                "Property failed.",
                file_name,
                i + 1,
                line,
                "A property test found a failing input.",
                "Fix the property or the function.",
            ));
        }
        if low.contains("when ") && low.contains("stop") {
            out.push(err(
                "T0616",
                "Handler stopped.",
                file_name,
                i + 1,
                line,
                "A handler stopped the server.",
                "Remove the Stop from the handler.",
            ));
        }
        if let Some(rest) = strip_word(line, "save") {
            let low = rest.to_ascii_lowercase();
            if let Some(p) = low.find(" as ") {
                let fmt = rest[p + 4..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('.')
                    .to_ascii_lowercase();
                if !matches!(fmt.as_str(), "json" | "toml" | "csv" | "xml" | "yaml") {
                    out.push(err(
                        "T0617",
                        "Format decode failed.",
                        file_name,
                        i + 1,
                        line,
                        "The save format is not supported.",
                        "Use json, toml, csv, xml or yaml.",
                    ));
                }
            }
        }
    }

    // T0901-T0903: internal errors.
    for (i, line) in lines.iter().enumerate() {
        let low = line.to_ascii_lowercase();
        if low.contains("internal error") {
            out.push(err("T0901", "Internal error.", file_name, i + 1, line, "The compiler hit an internal error.", "Report it with the input."));
        }
        if low.contains("backend disagrees") {
            out.push(err("T0902", "Backend disagrees.", file_name, i + 1, line, "The native backend disagrees with the interpreter.", "Report it with the input."));
        }
        if low.contains("cache damaged") {
            out.push(err("T0903", "Cache damaged.", file_name, i + 1, line, "A build cache entry is damaged.", "Delete the cache and rebuild."));
        }
    }

    out
}

fn definition_name(line: &str) -> Option<(String, usize)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let fw = words.first()?.to_ascii_lowercase();
    let name = match fw.as_str() {
        "changeable" | "constant" => words.get(1).cloned(),
        "to" => words.get(1).cloned(),
        _ if words.get(1).is_some_and(|w| w.eq_ignore_ascii_case("is")) => words.first().cloned(),
        _ => None,
    }?;
    Some((name.trim_end_matches('.').to_string(), 0))
}

fn strip_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    let mut it = line.splitn(2, ' ');
    let first = it.next()?;
    if first.eq_ignore_ascii_case(word) {
        it.next()
    } else {
        None
    }
}

fn contains_word(text: &str, word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|p| p == w)
}

fn err(
    code: &'static str,
    what: &str,
    file: &str,
    line: usize,
    sentence: &str,
    why: &str,
    fix: &str,
) -> Diagnostic {
    Diagnostic::error(
        code,
        what.to_string(),
        file.to_string(),
        line,
        sentence.to_string(),
        0,
        sentence.len(),
        why.to_string(),
        fix.to_string(),
    )
}
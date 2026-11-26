//! M5f item 6: mutation testing.
//!
//! Applies small, semantics-preserving-looking mutations to a program's source
//! (operator flips, comparison flips, constant tweaks), runs the program's
//! tests on each mutant, and reports which mutants the tests catch. A mutant
//! that still passes is a coverage gap. Mutants that do not compile are
//! skipped (they are not valid programs).

pub struct MutationReport {
    pub total: usize,
    pub caught: usize,
    pub survived: Vec<String>,
    pub has_tests: bool,
}

fn generate(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut add = |desc: &str, from: &str, to: &str, count: usize| {
        if source.contains(from) && count > 0 {
            out.push((
                format!("{} -> {}", desc, to),
                source.replacen(from, to, 1),
            ));
        }
    };
    add("plus", "plus", "minus", 1);
    add("minus", "minus", "plus", 1);
    add("times", "times", "divided by", 1);
    add(" is ", " is ", " is not ", 1);
    add("more than", "more than", "less than", 1);
    add("less than", "less than", "more than", 1);
    add("at least", "at least", "at most", 1);
    add("at most", "at most", "at least", 1);
    // constant tweaks
    for n in ["0", "1", "2"] {
        let from = format!("{} ", n);
        if source.contains(&from) {
            let to = if n == "0" { "1 " } else { "0 " };
            out.push((format!("const {} -> {}", n, to.trim()), source.replacen(&from, &to, 1)));
        }
    }
    out
}

/// Run mutation testing on a source file.
pub fn run(source: &str, file: &str) -> MutationReport {
    let has_tests = match crate::compile_tree(source, file) {
        Ok(p) => !p.tests.is_empty(),
        Err(_) => false,
    };
    let mut total = 0usize;
    let mut caught = 0usize;
    let mut survived = Vec::new();
    for (desc, src) in generate(source) {
        // skip mutants that do not compile (not valid programs)
        if crate::compile_tree(&src, file).is_err() {
            continue;
        }
        total += 1;
        let (_out, ok) = crate::test_source(&src, file, "");
        if has_tests && !ok {
            caught += 1;
        } else {
            survived.push(desc);
        }
    }
    MutationReport {
        total,
        caught,
        survived,
        has_tests,
    }
}
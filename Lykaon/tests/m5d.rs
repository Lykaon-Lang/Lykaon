//! M5d item 3: the 15 tutorial lessons, each runnable.

use lykaon::run_source;

fn run(src: &str, stdin: &str) -> (String, i32) {
    run_source(src, "program.lykaon", stdin)
}

#[test]
fn m5d_tutorial_examples() {
    let cases: &[(&str, &str, &str)] = &[
    ("01-say", r#"Say Hello World.
"#, r#"Hello World"#),
    ("02-names", r#"X is 5.
Say X plus 1.
"#, r#"6"#),
    ("03-math", r#"Say 2 times 21.
"#, r#"42"#),
    ("05-if", r#"If 1 is less than 2.
    Say yes.
"#, r#"yes"#),
    ("06-repeat", r#"Repeat 3 times.
    Say hi.
"#, r#"hi
hi
hi"#),
    ("07-list", r#"Nums is list of 1 and 2 and 3.
Say how many in Nums.
"#, r#"3"#),
    ("08-foreach", r#"Nums is list of 1 and 2 and 3.
For each n in Nums.
    Say n.
"#, r#"1
2
3"#),
    ("09-function", r#"To double n give n times 2.
Say double 21.
"#, r#"42"#),
    ("10-record", r#"A person has name as text and age as whole number.
P is a person with name Ada and age 36.
Say name of P.
"#, r#"Ada"#),
    ("11-map", r#"Changeable Ages is map of text to whole number.
Put 36 in Ages under Ada.
Say value of Ada in Ages.
"#, r#"36"#),
    ("12-text", r#"Line is cat sat down.
Say upper of Line.
"#, r#"CAT SAT DOWN"#),
    ("13-file", r#"Write hello to file note.
Read file note.
Say it.
"#, r#"hello"#),
    ("14-data", r#"Load sales.
Say rows and average price.
"#, r#"rows 8
average price 111.88"#),
    ("15-set", r#"Say 10 divided by 4.
"#, r#"2.50"#),
    ];
    for (name, src, expected) in cases {
        let _ = std::fs::remove_file("note");
        let (out, code) = run(src, "");
        assert_eq!(code, 0, "{name} failed: {out}");
        assert_eq!(out.trim_end(), *expected, "{name}");
    }
}

#[test]
fn m5d_ask_lesson_reads_stdin() {
    let src = "Ask name.\nSay hello name.\n";
    let (out, code) = run(src, "Ada\n");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "hello Ada");
}

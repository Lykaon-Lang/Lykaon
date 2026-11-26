//! M5 (Appendix L 7.1) conformance: the sales table of Chapter 54,
//! statistics, grouping, row sentences, table printing and the CSV
//! round trip. The fixture is `data/sales.csv` at the crate root.
//!
//! Run: cargo test --test m5_data -- --nocapture

use lykaon::{check_source, run_source};

fn run(src: &str) -> (String, i32) {
    run_source(src, "program.lykaon", "")
}

fn out(src: &str) -> String {
    let (out, code) = run(src);
    assert_eq!(code, 0, "the program failed: {out}");
    out.trim_end().to_string()
}

const LOAD: &str = "Load sales.\n";

#[test]
fn m5_t02_rows_and_average() {
    assert_eq!(
        out(&format!("{LOAD}Say rows and average price.")),
        "rows 8\naverage price 111.88"
    );
}

#[test]
fn m5_t03_median() {
    assert_eq!(out(&format!("{LOAD}Say median price.")), "median price 112.50");
}

#[test]
fn m5_t04_total() {
    assert_eq!(out(&format!("{LOAD}Say total price.")), "total price 895");
}

#[test]
fn m5_t05_extremes_and_spread() {
    assert_eq!(
        out(&format!(
            "{LOAD}Say smallest price and biggest price and spread price."
        )),
        "smallest price 70\nbiggest price 160\nspread price 90"
    );
}

#[test]
fn m5_t06_how_many() {
    assert_eq!(
        out(&format!("{LOAD}Say how many price.")),
        "how many price 8"
    );
}

#[test]
fn m5_t07_top_groups() {
    assert_eq!(
        out(&format!("{LOAD}Say top 5 regions by total price.")),
        "region  total price\n\
east    250\n\
north   240\n\
south   205\n\
west    200"
    );
}

#[test]
fn m5_t08_average_by_region() {
    assert_eq!(
        out(&format!("{LOAD}Say average price by region.")),
        "region  average price\n\
north   120.00\n\
east    125.00\n\
south   102.50\n\
west    100.00"
    );
}

#[test]
fn m5_t09_top_two_by_average() {
    assert_eq!(
        out(&format!("{LOAD}Say top 2 regions by average price.")),
        "region  average price\n\
east    125.00\n\
north   120.00"
    );
}

#[test]
fn m5_t10_rows_where_sorted() {
    assert_eq!(
        out(&format!(
            "{LOAD}Say rows where price is more than 120 sorted by price."
        )),
        "region  size  price  churned\n\
south   45    125    no\n\
west    50    130    no\n\
north   55    140    yes\n\
east    60    160    yes"
    );
}

#[test]
fn m5_t11_rows_where_churned() {
    assert_eq!(
        out(&format!("{LOAD}Say rows where churned is yes.")),
        "region  size  price  churned\n\
north   55    140    yes\n\
east    60    160    yes\n\
west    25    70     yes"
    );
}

#[test]
fn m5_t12_blank_cell_is_missing() {
    assert_eq!(
        out("Load blank.\nSay average price."),
        "average price 170.00\nnote price: 1 missing value ignored"
    );
}

#[test]
fn m5_for_each_row_borrows_a_record() {
    assert_eq!(
        out(&format!(
            "{LOAD}For each row in sales.\n    Say price of row."
        )),
        "140\n100\n160\n90\n125\n80\n130\n70"
    );
}

#[test]
fn m5_columns_lists_the_header() {
    assert_eq!(
        out(&format!("{LOAD}Say columns.")),
        "columns region size price churned"
    );
}

#[test]
fn m5_t20_unknown_column_is_t0175() {
    let (text, ok) = check_source(&format!("{LOAD}Say average prise."), "program.lykaon");
    assert!(!ok, "expected T0175, got: {text}");
    assert!(text.contains("T0175"), "expected T0175, got: {text}");
    assert!(text.contains("Unknown column."), "got: {text}");
}

#[test]
fn m5_t21_two_tables_is_t0177() {
    let (text, ok) = check_source(
        "Load sales.\nLoad blank.\nSay rows.",
        "program.lykaon",
    );
    assert!(!ok, "expected T0177, got: {text}");
    assert!(text.contains("T0177"), "expected T0177, got: {text}");
    assert!(text.contains("Which table."), "got: {text}");
}

#[test]
fn m5_no_table_is_t0178() {
    let (text, ok) = check_source("Say rows.", "program.lykaon");
    assert!(!ok, "expected T0178, got: {text}");
    assert!(text.contains("T0178"), "expected T0178, got: {text}");
}

#[test]
fn m5_duplicate_column_is_t0182() {
    let (text, ok) = check_source("Load dup.", "program.lykaon");
    assert!(!ok, "expected T0182, got: {text}");
    assert!(text.contains("T0182"), "expected T0182, got: {text}");
}

#[test]
fn m5_hard_word_column_is_t0181() {
    let (text, ok) = check_source("Load bad.", "program.lykaon");
    assert!(!ok, "expected T0181, got: {text}");
    assert!(text.contains("T0181"), "expected T0181, got: {text}");
}

#[test]
fn m5_m2_how_many_in_still_works() {
    // M5 must not steal M2's `Say how many in <list>.`
    assert_eq!(
        out("Numbers is list of 1 and 2 and 3.\nSay how many in numbers."),
        "3"
    );
}

#[test]
fn m5_save_table_as_csv() {
    let path = std::env::current_dir()
        .unwrap()
        .join("m5out.csv");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        "{LOAD}Save sales as csv to file m5out with extension csv."
    );
    let (text, code) = run(&src);
    assert_eq!(code, 0, "save failed: {text}");
    let saved = std::fs::read_to_string(&path).expect("the csv file is written");
    let _ = std::fs::remove_file(&path);
    assert!(saved.starts_with("region,size,price,churned\n"));
    assert!(saved.contains("north,55,140,yes"));
    assert_eq!(saved.lines().count(), 9);
}
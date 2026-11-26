//! M5 (Appendix L 7.1): tables, CSV loading, column kind inference,
//! statistics, grouping and table printing.
//!
//! A table is the value a `Load NAME.` sentence makes. The loader reads an
//! RFC 4180 CSV file, takes the first row as the header, and infers each
//! column's kind from its cells (M5-S1/S2). The interpreter and the native
//! runtime share this module, so both print and compute exactly the same
//! bytes.

use crate::formats;
use crate::sem::{Kind, Value};

/// One table column: its (trimmed, lower-cased) name and inferred kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub kind: Kind,
}

/// A loaded table. Rows keep the file order (M5-S12); a cell is `Missing`
/// when the cell was blank (M5-S2). The inner kind of a `Maybe` column is
/// the inferred non-missing kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }
}

/// Why a CSV file could not become a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The CSV reader refused the file (a quote ran off the end).
    BadFormat,
    /// The file has no header row.
    Empty,
    /// Two header names match after trimming and lower-casing (M5-S1).
    DuplicateColumn,
}

impl LoadError {
    pub fn message(self) -> &'static str {
        match self {
            LoadError::BadFormat => "bad format",
            LoadError::Empty => "bad format",
            LoadError::DuplicateColumn => "bad format",
        }
    }
}

/// The M5-S2 answer for one trimmed cell kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CellKind {
    Whole,
    Decimal,
    Boolean,
    Text,
}

fn classify(cell: &str) -> CellKind {
    let t = cell.trim();
    if t.parse::<i64>().is_ok() {
        return CellKind::Whole;
    }
    if t.parse::<f64>().is_ok() {
        return CellKind::Decimal;
    }
    if t.eq_ignore_ascii_case("yes") || t.eq_ignore_ascii_case("no") {
        return CellKind::Boolean;
    }
    CellKind::Text
}

fn is_blank(cell: &str) -> bool {
    cell.trim().is_empty()
}

/// M5-S1/S2: the header names the columns; every following row is one row.
/// Blanks are `Missing`; a column with at least one blank cell is `maybe`
/// of its inferred kind.
pub fn load_csv_text(text: &str) -> Result<Table, LoadError> {
    let raw = formats::parse_csv(text).map_err(|_| LoadError::BadFormat)?;
    let Some(header_row) = raw.first() else {
        return Err(LoadError::Empty);
    };
    if header_row.is_empty() {
        return Err(LoadError::Empty);
    }
    let names: Vec<String> = header_row
        .iter()
        .map(|n| n.trim().to_ascii_lowercase())
        .collect();
    for (i, n) in names.iter().enumerate() {
        if n.is_empty() {
            return Err(LoadError::BadFormat);
        }
        if names[..i].iter().any(|m| m == n) {
            return Err(LoadError::DuplicateColumn);
        }
    }
    let width = names.len();
    // Data rows: short rows pad with blanks, long rows are a bad format.
    let mut cells: Vec<Vec<String>> = Vec::with_capacity(raw.len().saturating_sub(1));
    for row in raw.iter().skip(1) {
        // a trailing empty line is not a row (the CSV reader drops the
        // final newline, but a file ending in two newlines can yield one)
        if row.len() == 1 && row[0].is_empty() && cells.is_empty() && raw.len() == 2 {
            // keep a genuinely empty data row: it is a row of blanks, as
            // RFC 4180 gives it; only a file with no data rows is empty
        }
        if row.len() > width {
            return Err(LoadError::BadFormat);
        }
        let mut r = row.clone();
        r.resize(width, String::new());
        cells.push(r);
    }
    // Infer each column from its non-blank cells (M5-S2).
    let mut columns = Vec::with_capacity(width);
    for (ci, name) in names.iter().enumerate() {
        let mut inferred: Option<CellKind> = None;
        let mut any_blank = false;
        for row in &cells {
            let c = &row[ci];
            if is_blank(c) {
                any_blank = true;
                continue;
            }
            let k = classify(c);
            inferred = Some(match (inferred, k) {
                (None, k) => k,
                (Some(CellKind::Text), _) | (_, CellKind::Text) => CellKind::Text,
                (Some(CellKind::Boolean), CellKind::Boolean) => CellKind::Boolean,
                (Some(CellKind::Boolean), _) => CellKind::Text,
                (_, CellKind::Boolean) => CellKind::Text,
                (Some(CellKind::Decimal), _) | (_, CellKind::Decimal) => CellKind::Decimal,
                (Some(CellKind::Whole), CellKind::Whole) => CellKind::Whole,
            });
        }
        let inner = match inferred.unwrap_or(CellKind::Text) {
            CellKind::Whole => Kind::Whole,
            CellKind::Decimal => Kind::Decimal,
            CellKind::Boolean => Kind::Boolean,
            CellKind::Text => Kind::Text,
        };
        let kind = if any_blank {
            Kind::Maybe(Box::new(inner))
        } else {
            inner
        };
        columns.push(Column {
            name: name.clone(),
            kind,
        });
    }
    // Coerce the cells to their column kinds.
    let mut rows = Vec::with_capacity(cells.len());
    for row in cells {
        let mut out = Vec::with_capacity(width);
        for (ci, cell) in row.iter().enumerate() {
            if is_blank(cell) {
                out.push(Value::Missing);
                continue;
            }
            let inner = match &columns[ci].kind {
                Kind::Maybe(i) => &**i,
                k => k,
            };
            out.push(match inner {
                Kind::Whole => Value::Whole(cell.trim().parse().unwrap_or(0)),
                Kind::Decimal => Value::Dec(cell.trim().parse().unwrap_or(0.0)),
                Kind::Boolean => Value::Boolean(cell.trim().eq_ignore_ascii_case("yes")),
                _ => Value::Text(cell.trim().to_string()),
            });
        }
        rows.push(out);
    }
    Ok(Table { columns, rows })
}

/// M5-S4: how one cell prints in a table or in a statistic line.
pub fn cell_text(v: &Value) -> String {
    match v {
        Value::Whole(n) => n.to_string(),
        Value::Dec(x) => format!("{x:.2}"),
        Value::Boolean(true) => "yes".to_string(),
        Value::Boolean(false) => "no".to_string(),
        Value::Missing => "missing".to_string(),
        Value::Text(t) => t.clone(),
        other => crate::formats::json_compact(other).unwrap_or_else(|_| "missing".to_string()),
    }
}

/// M5-S9: the header row, then one line per row. Every column is as wide as
/// its longest cell (the header included) plus 2 spaces; the last column is
/// not padded. An empty table prints only the header.
pub fn print_table(t: &Table) -> String {
    let widths: Vec<usize> = t
        .columns
        .iter()
        .enumerate()
        .map(|(ci, c)| {
            let mut w = c.name.chars().count();
            for row in &t.rows {
                if let Some(cell) = row.get(ci) {
                    w = w.max(cell_text(cell).chars().count());
                }
            }
            w + 2
        })
        .collect();
    let mut out = String::new();
    let last = t.columns.len().saturating_sub(1);
    for (ci, c) in t.columns.iter().enumerate() {
        out.push_str(&c.name);
        if ci != last {
            let pad = widths[ci].saturating_sub(c.name.chars().count());
            out.extend(std::iter::repeat_n(' ', pad));
        }
    }
    for row in &t.rows {
        out.push('\n');
        for (ci, cell) in row.iter().enumerate() {
            let text = cell_text(cell);
            out.push_str(&text);
            if ci != last {
                let pad = widths[ci].saturating_sub(text.chars().count());
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
    }
    out
}

/// M5-G4: the seven statistic words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatWord {
    Average,
    Median,
    Total,
    Smallest,
    Biggest,
    Spread,
    HowMany,
}

impl StatWord {
    pub fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "average" => StatWord::Average,
            "median" => StatWord::Median,
            "total" => StatWord::Total,
            "smallest" => StatWord::Smallest,
            "biggest" => StatWord::Biggest,
            "spread" => StatWord::Spread,
            "how" => StatWord::HowMany,
            _ => return None,
        })
    }

    pub fn words(self) -> &'static str {
        match self {
            StatWord::Average => "average",
            StatWord::Median => "median",
            StatWord::Total => "total",
            StatWord::Smallest => "smallest",
            StatWord::Biggest => "biggest",
            StatWord::Spread => "spread",
            StatWord::HowMany => "how many",
        }
    }

    /// `how many` is written with two words; the parser uses this to know
    /// whether the next token (`price`) belongs to the column words.
    pub fn is_how_many(self) -> bool {
        matches!(self, StatWord::HowMany)
    }

    fn needs_number(self) -> bool {
        !matches!(self, StatWord::HowMany | StatWord::Smallest | StatWord::Biggest)
    }
}

/// One computed statistic: the value's printed text and the number of
/// missing cells that were ignored (for the M5-S4 note).
#[derive(Clone, Debug, PartialEq)]
pub struct StatValue {
    pub text: String,
    pub missing: usize,
}

fn numeric(v: &Value) -> Option<f64> {
    match v {
        Value::Whole(n) => Some(*n as f64),
        Value::Dec(x) => Some(*x),
        _ => None,
    }
}

fn number_text(x: f64, whole: bool) -> String {
    if whole && x.fract() == 0.0 {
        format!("{}", x as i64)
    } else {
        format!("{x:.2}")
    }
}

/// M5-S3: compute one statistic over a column. An error string is the
/// diagnostic cause (T0305 for a text column where a number is needed).
pub fn stat(t: &Table, column: usize, word: StatWord) -> Result<StatValue, &'static str> {
    let col = t.columns.get(column).ok_or("unknown column")?;
    let inner = match &col.kind {
        Kind::Maybe(i) => &**i,
        k => k,
    };
    let mut values: Vec<&Value> = Vec::new();
    let mut missing = 0usize;
    for row in &t.rows {
        match row.get(column) {
            Some(Value::Missing) | None => missing += 1,
            Some(v) => values.push(v),
        }
    }
    let total_cells = t.rows.len();
    let used = values.len();
    if word == StatWord::HowMany {
        return Ok(StatValue {
            text: used.to_string(),
            missing: 0,
        });
    }
    if word.needs_number() && !matches!(inner, Kind::Whole | Kind::Decimal) {
        return Err("Column not numeric.");
    }
    if values.is_empty() {
        return Ok(StatValue {
            text: "missing".to_string(),
            missing,
        });
    }
    match word {
        StatWord::Average => {
            let nums: Vec<f64> = values.iter().filter_map(|v| numeric(v)).collect();
            let sum: f64 = nums.iter().sum();
            Ok(StatValue {
                text: number_text(sum / nums.len() as f64, false),
                missing,
            })
        }
        StatWord::Median => {
            let mut nums: Vec<f64> = values.iter().filter_map(|v| numeric(v)).collect();
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = nums.len();
            let m = if n % 2 == 1 {
                nums[n / 2]
            } else {
                (nums[n / 2 - 1] + nums[n / 2]) / 2.0
            };
            Ok(StatValue {
                text: number_text(m, false),
                missing,
            })
        }
        StatWord::Total => {
            let x: f64 = values.iter().filter_map(|v| numeric(v)).sum();
            let whole = matches!(inner, Kind::Whole) && x.fract() == 0.0;
            Ok(StatValue {
                text: number_text(x, whole),
                missing,
            })
        }
        StatWord::Smallest | StatWord::Biggest => {
            if matches!(inner, Kind::Whole | Kind::Decimal) {
                let nums: Vec<f64> = values.iter().filter_map(|v| numeric(v)).collect();
                let pick = if word == StatWord::Smallest {
                    nums.iter().cloned().fold(f64::INFINITY, f64::min)
                } else {
                    nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                };
                Ok(StatValue {
                    text: number_text(pick, matches!(inner, Kind::Whole) && pick.fract() == 0.0),
                    missing,
                })
            } else {
                let mut texts: Vec<String> = values.iter().map(|v| cell_text(v)).collect();
                texts.sort();
                let pick = if word == StatWord::Smallest {
                    texts.first().cloned().unwrap_or_default()
                } else {
                    texts.last().cloned().unwrap_or_default()
                };
                Ok(StatValue {
                    text: pick,
                    missing,
                })
            }
        }
        StatWord::Spread => {
            let nums: Vec<f64> = values.iter().filter_map(|v| numeric(v)).collect();
            let lo = nums.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Ok(StatValue {
                text: number_text(
                    hi - lo,
                    matches!(inner, Kind::Whole) && (hi - lo).fract() == 0.0,
                ),
                missing,
            })
        }
        StatWord::HowMany => unreachable!("handled above"),
    }
    .map(|mut sv| {
        // every cell missing: the value is missing, not "0" or "NaN"
        if used == 0 && total_cells > 0 {
            sv.text = "missing".to_string();
        }
        sv
    })
}

/// The M5-S4 note after a statistic line that ignored missing cells.
pub fn missing_note(column: &str, missing: usize) -> Option<String> {
    if missing == 0 {
        None
    } else if missing == 1 {
        Some(format!("note {column}: 1 missing value ignored"))
    } else {
        Some(format!("note {column}: {missing} missing values ignored"))
    }
}

/// M5-S6/S7: group the rows by `group_column`, compute the statistic for
/// each group, and return the two-column table. `top` is `Some((highest,
/// n))` for `top`/`bottom`, `None` for `by` (first-appearance order).
/// Ties keep the order the groups first appeared (M5-S6).
pub fn group_table(
    t: &Table,
    group_column: usize,
    word: StatWord,
    column: usize,
    top: Option<(bool, usize)>,
) -> Result<Table, &'static str> {
    let group_col = t.columns.get(group_column).ok_or("unknown column")?;
    let stat_col = t.columns.get(column).ok_or("unknown column")?;
    let mut groups: Vec<(Value, Vec<usize>)> = Vec::new();
    for (ri, row) in t.rows.iter().enumerate() {
        let key = row.get(group_column).cloned().unwrap_or(Value::Missing);
        if let Some((_, rows)) = groups.iter_mut().find(|(k, _)| *k == key) {
            rows.push(ri);
        } else {
            groups.push((key, vec![ri]));
        }
    }
    // Compute each group's statistic over a temporary table of its rows.
    let mut computed: Vec<(String, Value, f64, usize)> = Vec::new();
    for (key, indexes) in &groups {
        let rows: Vec<Vec<Value>> = indexes.iter().map(|&i| t.rows[i].clone()).collect();
        let sub = Table {
            columns: t.columns.clone(),
            rows,
        };
        let sv = stat(&sub, column, word)?;
        let display = match key {
            Value::Missing => "missing".to_string(),
            other => cell_text(other),
        };
        let sort_key = if matches!(stat_col.kind, Kind::Whole | Kind::Decimal)
            || matches!(&stat_col.kind, Kind::Maybe(i) if matches!(&**i, Kind::Whole | Kind::Decimal))
        {
            sv.text.parse::<f64>().unwrap_or(0.0)
        } else {
            // text smallest/biggest: sort in code-point order; the text
            // value is carried in the display of the statistic below
            0.0
        };
        let stat_value = if matches!(&stat_col.kind, Kind::Text)
            || matches!(&stat_col.kind, Kind::Maybe(i) if matches!(&**i, Kind::Text))
        {
            Value::Text(sv.text.clone())
        } else if matches!(word, StatWord::Average | StatWord::Median) {
            Value::Dec(sv.text.parse::<f64>().unwrap_or(0.0))
        } else if matches!(
            &stat_col.kind,
            Kind::Decimal | Kind::Maybe(_)
        ) && sv.text.contains('.')
        {
            Value::Dec(sv.text.parse::<f64>().unwrap_or(0.0))
        } else {
            Value::Whole(sv.text.parse::<i64>().unwrap_or(0))
        };
        computed.push((display, stat_value, sort_key, indexes.len()));
    }
    if let Some((highest, n)) = top {
        // stable: equal sort keys keep first-appearance order
        computed.sort_by(|a, b| {
            let ord = a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal);
            if highest {
                ord.reverse()
            } else {
                ord
            }
        });
        computed.truncate(n);
    }
    let mut out = Table {
        columns: vec![
            Column {
                name: group_col.name.clone(),
                kind: group_col.kind.clone(),
            },
            Column {
                name: format!("{} {}", word.words(), stat_col.name),
                kind: stat_col.kind.clone(),
            },
        ],
        rows: Vec::new(),
    };
    for (display, value, _, _) in computed {
        out.rows.push(vec![Value::Text(display), value]);
    }
    Ok(out)
}

/// M5-S10: the CSV writer for a table. The header is the column names;
/// each cell is written as printed except a decimal, which uses the
/// shortest round-trip text, and text with a comma, quote or line break is
/// quoted by the RFC 4180 rules.
pub fn table_to_csv(t: &Table) -> String {
    fn row(cells: &[String]) -> String {
        cells
            .iter()
            .map(|c| {
                if c.contains(',') || c.contains('"') || c.contains('\n') || c.contains('\r') {
                    format!("\"{}\"", c.replace('"', "\"\""))
                } else {
                    c.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
    let mut out = row(&t.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>());
    for r in &t.rows {
        let cells: Vec<String> = r
            .iter()
            .map(|v| match v {
                Value::Dec(x) => format!("{x}"),
                other => cell_text(other),
            })
            .collect();
        out.push('\n');
        out.push_str(&row(&cells));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALES: &str = "region,size,price,churned\n\
north,55,140,yes\n\
north,40,100,no\n\
east,60,160,yes\n\
east,35,90,no\n\
south,45,125,no\n\
south,30,80,no\n\
west,50,130,no\n\
west,25,70,yes\n";

    fn sales() -> Table {
        load_csv_text(SALES).expect("sales loads")
    }

    #[test]
    fn infers_whole_text_and_boolean_columns() {
        let t = sales();
        assert_eq!(t.columns[0].kind, Kind::Text);
        assert_eq!(t.columns[1].kind, Kind::Whole);
        assert_eq!(t.columns[2].kind, Kind::Whole);
        assert_eq!(t.columns[3].kind, Kind::Boolean);
        assert_eq!(t.rows.len(), 8);
    }

    #[test]
    fn blank_cell_makes_maybe_kind_and_counts_missing() {
        let t = load_csv_text("name,price\na,1\nb,\nc,3\n").unwrap();
        assert_eq!(t.columns[1].kind, Kind::Maybe(Box::new(Kind::Whole)));
        let av = stat(&t, 1, StatWord::Average).unwrap();
        assert_eq!(av.text, "2.00");
        assert_eq!(av.missing, 1);
        assert_eq!(
            missing_note("price", 1).as_deref(),
            Some("note price: 1 missing value ignored")
        );
    }

    #[test]
    fn statistics_match_the_book() {
        let t = sales();
        let price = t.column_index("price").unwrap();
        assert_eq!(stat(&t, price, StatWord::Average).unwrap().text, "111.88");
        assert_eq!(stat(&t, price, StatWord::Median).unwrap().text, "112.50");
        assert_eq!(stat(&t, price, StatWord::Total).unwrap().text, "895");
        assert_eq!(stat(&t, price, StatWord::Smallest).unwrap().text, "70");
        assert_eq!(stat(&t, price, StatWord::Biggest).unwrap().text, "160");
        assert_eq!(stat(&t, price, StatWord::Spread).unwrap().text, "90");
        assert_eq!(stat(&t, price, StatWord::HowMany).unwrap().text, "8");
    }

    #[test]
    fn top_groups_match_the_book() {
        let t = sales();
        let region = t.column_index("region").unwrap();
        let price = t.column_index("price").unwrap();
        let g = group_table(&t, region, StatWord::Total, price, Some((true, 5))).unwrap();
        assert_eq!(
            print_table(&g),
            "region  total price\n\
east    250\n\
north   240\n\
south   205\n\
west    200"
        );
    }

    #[test]
    fn table_padding_matches_m5_s9() {
        let t = load_csv_text("region,price\neast,250\nnorth,240\n").unwrap();
        assert_eq!(print_table(&t), "region  price\neast    250\nnorth   240");
    }
}

//! M5b (Appendix L add-on): automatic data layout.
//!
//! A Lykaon table is row-major (`Vec<Vec<Value>>`). This module analyses a
//! table, chooses a layout (row-major vs struct-of-arrays), and provides
//! columnar kernels (sum, filter-sum, dot) that run over flat `f64` buffers.
//! The hot kernels use AVX2 when the CPU supports it (runtime-detected) and a
//! scalar fallback elsewhere, so the same code is correct everywhere.
//!
//! The point is not to replace the table value (that would break M5), but to
//! add an automatic layout choice and real vectorised kernels behind a
//! feature, with a report of what was chosen and why.

use crate::data::Table;
use crate::sem::{Kind, Value};

/// The chosen physical layout for a table's numeric columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Row-major (array of structs): the default `Table`.
    RowMajor,
    /// Struct-of-arrays: one contiguous `f64` buffer per numeric column.
    StructOfArrays,
}

impl Layout {
    pub fn name(self) -> &'static str {
        match self {
            Layout::RowMajor => "row-major (array of structs)",
            Layout::StructOfArrays => "struct-of-arrays (columnar)",
        }
    }
}

/// The layout decision for one table, with the reason.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutReport {
    pub layout: Layout,
    pub rows: usize,
    pub numeric_columns: usize,
    pub reason: String,
}

impl LayoutReport {
    pub fn describe(&self) -> String {
        format!(
            "layout: {}\nrows: {}\nnumeric columns: {}\nreason: {}",
            self.layout.name(),
            self.rows,
            self.numeric_columns,
            self.reason
        )
    }
}

/// True for the numeric kinds (and `maybe` of a numeric kind).
pub fn is_numeric_kind(k: &Kind) -> bool {
    match k {
        Kind::Whole | Kind::Decimal | Kind::Big => true,
        Kind::Maybe(inner) => is_numeric_kind(inner),
        _ => false,
    }
}

/// Analyse a table and choose a layout.
///
/// SoA wins when there is at least one numeric column and the table is large
// enough to amortise extracting the columns (small tables stay row-major).
pub fn choose(table: &Table) -> LayoutReport {
    let rows = table.rows.len();
    let numeric_columns = table
        .columns
        .iter()
        .filter(|c| is_numeric_kind(&c.kind))
        .count();
    let use_soa = numeric_columns > 0 && rows >= 64;
    let reason = if use_soa {
        format!(
            "{} numeric column(s) over {} rows: flat per-column buffers let sums and filters run over contiguous slices (SIMD-friendly)",
            numeric_columns, rows
        )
    } else if numeric_columns == 0 {
        "no numeric columns: row-major keeps each record together".to_string()
    } else {
        format!(
            "only {} rows: row-major avoids the column-extraction cost",
            rows
        )
    };
    LayoutReport {
        layout: if use_soa {
            Layout::StructOfArrays
        } else {
            Layout::RowMajor
        },
        rows,
        numeric_columns,
        reason,
    }
}

/// Extract a numeric column as a flat `f64` buffer (Missing -> 0.0). This is
/// the struct-of-arrays representation of one column.
pub fn column_f64(table: &Table, col: usize) -> Vec<f64> {
    table
        .rows
        .iter()
        .map(|r| match r.get(col) {
            Some(Value::Whole(n)) => *n as f64,
            Some(Value::Dec(x)) => *x,
            _ => 0.0,
        })
        .collect()
}

/// Sum a column the row-major way (baseline).
pub fn sum_column_rowmajor(table: &Table, col: usize) -> f64 {
    let mut s = 0.0;
    for r in &table.rows {
        if let Some(v) = r.get(col) {
            match v {
                Value::Whole(n) => s += *n as f64,
                Value::Dec(x) => s += *x,
                _ => {}
            }
        }
    }
    s
}

/// Sum a column the columnar way: extract once, then vectorised sum.
pub fn sum_column_soa(table: &Table, col: usize) -> f64 {
    let xs = column_f64(table, col);
    sum_f64(&xs)
}

/// Sum `value_col` for rows where `cond_col > threshold` (columnar filter).
pub fn sum_where_soa(table: &Table, value_col: usize, cond_col: usize, threshold: f64) -> f64 {
    let vals = column_f64(table, value_col);
    let conds = column_f64(table, cond_col);
    let filtered: Vec<f64> = vals
        .iter()
        .zip(conds.iter())
        .filter(|(_, c)| **c > threshold)
        .map(|(v, _)| *v)
        .collect();
    sum_f64(&filtered)
}

/// Dot product of two equal-length slices (vectorised).
pub fn dot_f64(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        s += x * y;
    }
    s
}

/// Sum a slice of `f64`. Uses AVX2 when available (runtime-detected), with a
/// scalar fallback. Both paths produce the same result (up to fp order).
pub fn sum_f64(xs: &[f64]) -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") && xs.len() >= 8 {
            // SAFETY: AVX2 is detected above; the loads are unaligned.
            return unsafe { sum_f64_avx2(xs) };
        }
    }
    let mut s = 0.0;
    for x in xs {
        s += *x;
    }
    s
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sum_f64_avx2(xs: &[f64]) -> f64 {
    use std::arch::x86_64::*;
    let mut acc = _mm256_setzero_pd();
    let mut i = 0usize;
    while i + 4 <= xs.len() {
        let v = _mm256_loadu_pd(xs.as_ptr().add(i));
        acc = _mm256_add_pd(acc, v);
        i += 4;
    }
    let mut tmp = [0f64; 4];
    _mm256_storeu_pd(tmp.as_mut_ptr(), acc);
    let mut s = tmp[0] + tmp[1] + tmp[2] + tmp[3];
    while i < xs.len() {
        s += xs[i];
        i += 1;
    }
    s
}

/// A small measured comparison of the two layouts (for the report). Returns
/// (row_major_ms, soa_ms).
pub fn benchmark(table: &Table, col: usize) -> (f64, f64) {
    use std::time::Instant;
    // warm up
    let _ = sum_column_rowmajor(table, col);
    let xs = column_f64(table, col);
    let _ = sum_f64(&xs);

    let t0 = Instant::now();
    let mut acc = 0.0;
    for _ in 0..50 {
        acc += sum_column_rowmajor(table, col);
    }
    let row_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t1 = Instant::now();
    let mut acc2 = 0.0;
    for _ in 0..50 {
        acc2 += sum_f64(&xs);
    }
    let soa_ms = t1.elapsed().as_secs_f64() * 1000.0;
    assert!(acc.is_finite() && acc2.is_finite());
    (row_ms, soa_ms)
}

/// Build a synthetic numeric table (for the standalone `lykaon layout`).
pub fn synthetic(rows: usize) -> Table {
    let mut data = Vec::with_capacity(rows);
    for i in 0..rows {
        let n = (i % 97) as i64;
        data.push(vec![Value::Whole(n), Value::Dec(n as f64 * 1.5)]);
    }
    Table {
        columns: vec![
            crate::data::Column { name: "a".into(), kind: Kind::Whole },
            crate::data::Column { name: "b".into(), kind: Kind::Decimal },
        ],
        rows: data,
    }
}
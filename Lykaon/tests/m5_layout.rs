//! M5b: automatic data layout (SoA + SIMD + report).

use lykaon_core::data::Table;
use lykaon_core::layout::{self, Layout};
use lykaon_core::sem::{Kind, Value};

fn table(rows: usize) -> Table {
    layout::synthetic(rows)
}

#[test]
fn soa_sum_matches_rowmajor() {
    let t = table(1000);
    let a = layout::sum_column_rowmajor(&t, 0);
    let b = layout::sum_column_soa(&t, 0);
    assert!((a - b).abs() < 1e-6, "row-major {a} vs SoA {b}");
}

#[test]
fn simd_sum_matches_scalar() {
    // A slice long enough to exercise the AVX2 path.
    let xs: Vec<f64> = (0..1000).map(|i| i as f64).collect();
    let simd = layout::sum_f64(&xs);
    let scalar: f64 = xs.iter().sum();
    assert!((simd - scalar).abs() < 1e-6, "simd {simd} vs scalar {scalar}");
}

#[test]
fn layout_choice() {
    let big = layout::choose(&table(1000));
    assert_eq!(big.layout, Layout::StructOfArrays, "{}", big.reason);
    let small = layout::choose(&table(8));
    assert_eq!(small.layout, Layout::RowMajor, "{}", small.reason);
}

#[test]
fn filter_sum_matches_manual() {
    // synthetic column 0 is i%97, column 1 is i%97*1.5. Sum column 1 where
    // column 0 > 50, and compare to a manual walk.
    let t = table(500);
    let got = layout::sum_where_soa(&t, 1, 0, 50.0);
    let mut want = 0.0;
    for r in &t.rows {
        let a = match r[0] {
            Value::Whole(n) => n as f64,
            _ => 0.0,
        };
        let b = match r[1] {
            Value::Dec(x) => x,
            _ => 0.0,
        };
        if a > 50.0 {
            want += b;
        }
    }
    assert!((got - want).abs() < 1e-6, "filter {got} vs manual {want}");
}

#[test]
fn report_describes_the_choice() {
    let t = table(1000);
    let r = layout::choose(&t);
    let text = r.describe();
    assert!(text.contains("struct-of-arrays"), "{text}");
    assert!(text.contains("numeric columns: 2"), "{text}");
}

#[test]
fn benchmark_runs() {
    let t = table(2000);
    let (row_ms, soa_ms) = layout::benchmark(&t, 0);
    assert!(row_ms.is_finite() && soa_ms.is_finite());
    let _ = Kind::Whole; // keep the import used
}
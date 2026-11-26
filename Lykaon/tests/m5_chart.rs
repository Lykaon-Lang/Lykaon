//! M5 (Appendix L 7.2) chart conformance: SVG output, determinism and the
//! T0184 text-column rule. The fixture is `data/sales.csv`.
//!
//! Run: cargo test --test m5_chart -- --nocapture

use lykaon::run_source;

fn run(src: &str) -> (String, i32) {
    run_source(src, "program.lykaon", "")
}

fn clean(path: &str) {
    let _ = std::fs::remove_file(path);
}

#[test]
fn m5_t13_dots_svg_shape() {
    clean("charta.svg");
    let (out, code) = run("Load sales.\nPlot price against size to file charta.");
    assert_eq!(code, 0, "plot failed: {out}");
    let svg = std::fs::read_to_string("charta.svg").expect("charta.svg is written");
    clean("charta.svg");
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("width=\"800\""));
    assert!(svg.contains("height=\"480\""));
    assert!(svg.contains("fill=\"#ffffff\""));
    assert!(svg.contains("stroke=\"#444444\""));
    assert!(svg.contains("r=\"3\""));
    assert!(svg.contains("fill=\"#1f6f8b\""));
    assert!(svg.contains(">size</text>"));
    assert!(svg.contains(">price</text>"));
}

#[test]
fn m5_t13_barsand_dots_are_deterministic() {
    clean("chartb1.svg");
    clean("chartb2.svg");
    let (out1, c1) = run("Load sales.\nPlot price against size to file chartb1.");
    let (out2, c2) = run("Load sales.\nPlot price against size to file chartb2.");
    assert_eq!((c1, c2), (0, 0), "{out1}\n{out2}");
    let a = std::fs::read("chartb1.svg").unwrap();
    let b = std::fs::read("chartb2.svg").unwrap();
    clean("chartb1.svg");
    clean("chartb2.svg");
    assert_eq!(a, b, "the SVG bytes must be deterministic");
}

#[test]
fn m5_t14_bars_from_text_column() {
    clean("barsa.svg");
    let (out, code) = run("Load sales.\nPlot region against price as bars to file barsa.");
    assert_eq!(code, 0, "bars failed: {out}");
    let svg = std::fs::read_to_string("barsa.svg").expect("barsa.svg is written");
    clean("barsa.svg");
    assert!(svg.contains("<rect"));
    assert!(svg.contains(">north</text>"));
    assert!(svg.contains(">east</text>"));
}

#[test]
fn m5_t14_dots_on_a_text_column_is_t0184() {
    let (out, code) = run("Load sales.\nPlot region against price.");
    assert_eq!(code, 1, "dots on text must fail: {out}");
    assert!(out.contains("T0184"), "expected T0184, got: {out}");
    assert!(out.contains("Column not numeric."), "got: {out}");
}

#[test]
fn m5_t15_png_shape_and_determinism() {
    clean("chartp1.png");
    clean("chartp2.png");
    let (out1, c1) = run("Load sales.\nPlot price against size to file chartp1 with extension png.");
    let (out2, c2) = run("Load sales.\nPlot price against size to file chartp2 with extension png.");
    assert_eq!((c1, c2), (0, 0), "{out1}\n{out2}");
    let a = std::fs::read("chartp1.png").expect("chartp1.png is written");
    let b = std::fs::read("chartp2.png").expect("chartp2.png is written");
    clean("chartp1.png");
    clean("chartp2.png");
    assert_eq!(a, b, "the PNG bytes must be deterministic");
    assert_eq!(&a[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    assert_eq!(&a[12..16], b"IHDR");
    assert_eq!(u32::from_be_bytes(a[16..20].try_into().unwrap()), 800);
    assert_eq!(u32::from_be_bytes(a[20..24].try_into().unwrap()), 480);
    assert!(a.len() > 1000, "a chart PNG should not be empty");
}

#[test]
fn m5_default_plot_goes_to_output_plot_1() {
    let dir = std::path::Path::new("output");
    let _ = std::fs::remove_dir_all(dir);
    let (out, code) = run("Load sales.\nPlot price against size.");
    assert_eq!(code, 0, "plot failed: {out}");
    let path = dir.join("plot-1.svg");
    assert!(path.exists(), "output/plot-1.svg must be written");
    let svg = std::fs::read_to_string(&path).unwrap();
    assert!(svg.contains("width=\"800\""));
    let _ = std::fs::remove_dir_all(dir);
}
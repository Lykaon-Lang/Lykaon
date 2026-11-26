//! M5 (Appendix L 7.2): charts written as SVG or PNG.
//!
//! `Plot VERTICAL against HORIZONTAL [as dots|bars|line] [to file F]`.
//! The first column is the vertical axis (M5-S13). Output is an 800 by 480
//! SVG with axes, round tick values, the column names as axis labels and
//! no title (M5-S15). The bytes are deterministic so tests can compare
//! them (M5-T13/T14). `render_png` rasterizes the same layout with the
//! public-domain 8x8 font in `font8x8` and a zlib IDAT from flate2.

use crate::data::{cell_text, Table};
use crate::sem::{Kind, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    Dots,
    Bars,
    Line,
}

impl PlotKind {
    pub fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "dots" => PlotKind::Dots,
            "bars" => PlotKind::Bars,
            "line" => PlotKind::Line,
            _ => return None,
        })
    }
}

/// Why a plot cannot be drawn (the caller turns it into T0184).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlotError {
    /// A number was needed and the column is text.
    NotNumeric(&'static str),
}

impl PlotError {
    pub fn message(&self) -> &'static str {
        match self {
            PlotError::NotNumeric(_) => "Column not numeric.",
        }
    }
}

fn is_number_kind(k: &Kind) -> bool {
    match k {
        Kind::Whole | Kind::Decimal => true,
        Kind::Maybe(inner) => is_number_kind(inner),
        _ => false,
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Whole(n) => Some(*n as f64),
        Value::Dec(x) => Some(*x),
        _ => None,
    }
}

/// A round tick step giving about `target` intervals (M5-S15: 5 to 10
/// round values).
fn nice_step(range: f64, target: usize) -> f64 {
    if range <= 0.0 || !range.is_finite() {
        return 1.0;
    }
    let raw = range / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let mult = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    mult * mag
}

fn ticks(min: f64, max: f64) -> Vec<f64> {
    let (lo, hi) = if min <= max {
        (min, max)
    } else {
        (max, min)
    };
    if !lo.is_finite() || !hi.is_finite() {
        return vec![0.0];
    }
    let step = nice_step(hi - lo, 6);
    let start = (lo / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = start;
    let mut guard = 0;
    while v <= hi + step * 1e-9 && guard < 16 {
        out.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
        v += step;
        guard += 1;
    }
    if out.len() < 2 {
        out = vec![lo, hi];
    }
    out
}

/// Deterministic number text: up to 2 decimals, trailing zeros removed.
fn num_text(x: f64) -> String {
    if !x.is_finite() {
        return "0".to_string();
    }
    if x.fract() == 0.0 && x.abs() < 1e15 {
        return format!("{}", x as i64);
    }
    let mut s = format!("{x:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn coord(x: f64, y: f64) -> String {
    format!("{},{}", num_text(x), num_text(y))
}

const W: f64 = 800.0;
const H: f64 = 480.0;
const LEFT: f64 = 70.0;
const RIGHT: f64 = 20.0;
const TOP: f64 = 20.0;
const BOTTOM: f64 = 50.0;

struct Frame {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Frame {
    fn new() -> Self {
        Frame {
            x0: LEFT,
            x1: W - RIGHT,
            y0: TOP,
            y1: H - BOTTOM,
        }
    }
    fn px(&self, x: f64, lo: f64, hi: f64) -> f64 {
        if hi - lo <= 0.0 {
            return (self.x0 + self.x1) / 2.0;
        }
        self.x0 + (x - lo) / (hi - lo) * (self.x1 - self.x0)
    }
    fn py(&self, y: f64, lo: f64, hi: f64) -> f64 {
        if hi - lo <= 0.0 {
            return (self.y0 + self.y1) / 2.0;
        }
        self.y1 - (y - lo) / (hi - lo) * (self.y1 - self.y0)
    }
}

fn svg_open() -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        num_text(W),
        num_text(H),
        num_text(W),
        num_text(H)
    ));
    s.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>\n",
        num_text(W),
        num_text(H)
    ));
    s
}

fn axis_frame(out: &mut String, f: &Frame) {
    out.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#444444\" stroke-width=\"1\"/>\n",
        num_text(f.x0),
        num_text(f.y0),
        num_text(f.x0),
        num_text(f.y1)
    ));
    out.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#444444\" stroke-width=\"1\"/>\n",
        num_text(f.x0),
        num_text(f.y1),
        num_text(f.x1),
        num_text(f.y1)
    ));
}

fn y_axis(out: &mut String, f: &Frame, lo: f64, hi: f64) {
    for t in ticks(lo, hi) {
        let y = f.py(t, lo, hi);
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#444444\" stroke-width=\"1\"/>\n",
            num_text(f.x0 - 5.0),
            num_text(y),
            num_text(f.x0),
            num_text(y)
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"12\" fill=\"#444444\" text-anchor=\"end\">{}</text>\n",
            num_text(f.x0 - 8.0),
            num_text(y + 4.0),
            esc(&num_text(t))
        ));
    }
}

fn x_axis_numbers(out: &mut String, f: &Frame, lo: f64, hi: f64) {
    for t in ticks(lo, hi) {
        let x = f.px(t, lo, hi);
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#444444\" stroke-width=\"1\"/>\n",
            num_text(x),
            num_text(f.y1),
            num_text(x),
            num_text(f.y1 + 5.0)
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"12\" fill=\"#444444\" text-anchor=\"middle\">{}</text>\n",
            num_text(x),
            num_text(f.y1 + 20.0),
            esc(&num_text(t))
        ));
    }
}

fn axis_labels(out: &mut String, vertical: &str, horizontal: &str) {
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"13\" fill=\"#444444\" text-anchor=\"middle\">{}</text>\n",
        num_text((LEFT + W - RIGHT) / 2.0),
        num_text(H - 8.0),
        esc(horizontal)
    ));
    out.push_str(&format!(
        "<text x=\"18\" y=\"{}\" font-family=\"monospace\" font-size=\"13\" fill=\"#444444\" text-anchor=\"middle\" transform=\"rotate(-90 18 {})\">{}</text>\n",
        num_text((TOP + H - BOTTOM) / 2.0),
        num_text((TOP + H - BOTTOM) / 2.0),
        esc(vertical)
    ));
}

fn finish(out: &mut String) -> String {
    out.push_str("</svg>\n");
    std::mem::take(out)
}

/// M5-S13..S16: draw the chart and return the SVG bytes.
pub fn render_svg(
    t: &Table,
    vertical: usize,
    horizontal: usize,
    kind: Option<PlotKind>,
) -> Result<String, PlotError> {
    let vcol0 = t
        .columns
        .get(vertical)
        .ok_or(PlotError::NotNumeric("vertical"))?;
    let hcol0 = t
        .columns
        .get(horizontal)
        .ok_or(PlotError::NotNumeric("horizontal"))?;
    let v_num0 = is_number_kind(&vcol0.kind);
    let h_num0 = is_number_kind(&hcol0.kind);
    let kind = match kind {
        Some(k) => k,
        None => {
            if !h_num0 {
                PlotKind::Bars
            } else {
                PlotKind::Dots
            }
        }
    };
    // M5-S13/S14: the first column is the vertical axis for numeric plots.
    // A bar chart is the exception the book's own example relies on
    // (`Plot region against price as bars.`): the text column is the
    // horizontal category axis and the number column the vertical value.
    let (vi, hi) = if kind == PlotKind::Bars && !v_num0 && h_num0 {
        (horizontal, vertical)
    } else {
        (vertical, horizontal)
    };
    let vcol = &t.columns[vi];
    let hcol = &t.columns[hi];
    let v_num = is_number_kind(&vcol.kind);
    let h_num = is_number_kind(&hcol.kind);
    let f = Frame::new();
    let mut out = svg_open();

    match kind {
        PlotKind::Bars => {
            if !v_num {
                return Err(PlotError::NotNumeric("vertical"));
            }
            // one bar per distinct text value, total of the vertical column,
            // in first-appearance order (the book fixes the value, not the
            // category order).
            let mut groups: Vec<(String, f64)> = Vec::new();
            for row in &t.rows {
                let key = row
                    .get(hi)
                    .map(cell_text)
                    .unwrap_or_else(|| "missing".to_string());
                let val = row.get(vi).and_then(number).unwrap_or(0.0);
                if let Some((_, sum)) = groups.iter_mut().find(|(k, _)| *k == key) {
                    *sum += val;
                } else {
                    groups.push((key, val));
                }
            }
            let max = groups
                .iter()
                .map(|(_, v)| *v)
                .fold(0.0_f64, f64::max)
                .max(0.0);
            let hi = if max > 0.0 { max } else { 1.0 };
            axis_frame(&mut out, &f);
            y_axis(&mut out, &f, 0.0, hi);
            axis_labels(&mut out, &vcol.name, &hcol.name);
            let band = (f.x1 - f.x0) / groups.len().max(1) as f64;
            let bw = band * 0.6;
            for (i, (name, value)) in groups.iter().enumerate() {
                let cx = f.x0 + band * (i as f64 + 0.5);
                let y = f.py(*value, 0.0, hi);
                let height = f.y1 - y;
                out.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#1f6f8b\"/>\n",
                    num_text(cx - bw / 2.0),
                    num_text(y),
                    num_text(bw),
                    num_text(height.max(0.0))
                ));
                out.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"11\" fill=\"#444444\" text-anchor=\"middle\">{}</text>\n",
                    num_text(cx),
                    num_text(f.y1 + 16.0),
                    esc(name)
                ));
            }
            Ok(finish(&mut out))
        }
        PlotKind::Dots | PlotKind::Line => {
            if !v_num || !h_num {
                return Err(PlotError::NotNumeric(if !h_num {
                    "horizontal"
                } else {
                    "vertical"
                }));
            }
            let mut pts: Vec<(f64, f64)> = t
                .rows
                .iter()
                .map(|r| {
                    (
                        r.get(hi).and_then(number).unwrap_or(0.0),
                        r.get(vi).and_then(number).unwrap_or(0.0),
                    )
                })
                .collect();
            if kind == PlotKind::Line {
                pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }
            let (mut xmin, mut xmax) = pts
                .iter()
                .map(|(x, _)| *x)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(x), b.max(x))
                });
            let (mut ymin, mut ymax) = pts
                .iter()
                .map(|(_, y)| *y)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), y| {
                    (a.min(y), b.max(y))
                });
            if !xmin.is_finite() || !xmax.is_finite() {
                xmin = 0.0;
                xmax = 1.0;
            }
            if xmin == xmax {
                xmin -= 1.0;
                xmax += 1.0;
            }
            if !ymin.is_finite() || !ymax.is_finite() {
                ymin = 0.0;
                ymax = 1.0;
            }
            if ymin == ymax {
                ymin -= 1.0;
                ymax += 1.0;
            }
            axis_frame(&mut out, &f);
            y_axis(&mut out, &f, ymin, ymax);
            x_axis_numbers(&mut out, &f, xmin, xmax);
            axis_labels(&mut out, &vcol.name, &hcol.name);
            if kind == PlotKind::Line {
                let points: Vec<String> = pts
                    .iter()
                    .map(|(x, y)| coord(f.px(*x, xmin, xmax), f.py(*y, ymin, ymax)))
                    .collect();
                out.push_str(&format!(
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"#1f6f8b\" stroke-width=\"2\"/>\n",
                    points.join(" ")
                ));
            } else {
                for (x, y) in &pts {
                    out.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"#1f6f8b\"/>\n",
                        num_text(f.px(*x, xmin, xmax)),
                        num_text(f.py(*y, ymin, ymax))
                    ));
                }
            }
            Ok(finish(&mut out))
        }
    }
}

/// M5-S13..S16: draw the chart and return PNG bytes.
///
/// The SVG is the source of truth for the layout; this rasterizes the small
/// subset the chart writer emits (rect, line, circle, polyline, text) with
/// the public-domain 8x8 font. The output is deterministic.
pub fn render_png(
    t: &Table,
    vertical: usize,
    horizontal: usize,
    kind: Option<PlotKind>,
) -> Result<Vec<u8>, PlotError> {
    let svg = render_svg(t, vertical, horizontal, kind)?;
    Ok(rasterize_svg(&svg))
}

fn attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let mut i = 0usize;
    while let Some(p) = line[i..].find(name) {
        let abs = i + p;
        let before_ok = abs == 0 || line.as_bytes()[abs - 1] == b' ';
        let after = abs + name.len();
        if before_ok && line[after..].starts_with("=\"") {
            let start = after + 2;
            let end = start + line[start..].find('"')?;
            return Some(&line[start..end]);
        }
        i = after;
    }
    None
}

fn attr_f64(line: &str, name: &str, default: f64) -> f64 {
    attr(line, name)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}

fn color(line: &str, key: &str) -> [u8; 3] {
    attr(line, key)
        .and_then(parse_hex)
        .unwrap_or([0x44, 0x44, 0x44])
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

fn rasterize_svg(svg: &str) -> Vec<u8> {
    use crate::png::Canvas;
    let mut c = Canvas::new(800, 480, [0xff, 0xff, 0xff]);
    for raw in svg.lines() {
        let t = raw.trim();
        if t.starts_with("<rect ") && t.ends_with("/>") {
            let x = attr_f64(t, "x", 0.0);
            let y = attr_f64(t, "y", 0.0);
            let w = attr_f64(t, "width", 0.0);
            let h = attr_f64(t, "height", 0.0);
            c.fill_rect(x, y, w, h, color(t, "fill"));
        } else if t.starts_with("<line ") && t.ends_with("/>") {
            let x1 = attr_f64(t, "x1", 0.0);
            let y1 = attr_f64(t, "y1", 0.0);
            let x2 = attr_f64(t, "x2", 0.0);
            let y2 = attr_f64(t, "y2", 0.0);
            let thick = attr_f64(t, "stroke-width", 1.0);
            c.line(x1, y1, x2, y2, color(t, "stroke"), thick);
        } else if t.starts_with("<circle ") && t.ends_with("/>") {
            let cx = attr_f64(t, "cx", 0.0);
            let cy = attr_f64(t, "cy", 0.0);
            let r = attr_f64(t, "r", 0.0);
            c.circle(cx, cy, r, color(t, "fill"));
        } else if t.starts_with("<polyline ") && t.ends_with("/>") {
            let pts = attr(t, "points").unwrap_or("");
            let nums: Vec<f64> = pts
                .split_whitespace()
                .map(|p| p.parse::<f64>().unwrap_or(0.0))
                .collect();
            let thick = attr_f64(t, "stroke-width", 1.0);
            let col = color(t, "stroke");
            let mut prev: Option<(f64, f64)> = None;
            for pair in nums.chunks(2) {
                if pair.len() < 2 {
                    continue;
                }
                let cur = (pair[0], pair[1]);
                if let Some((px, py)) = prev {
                    c.line(px, py, cur.0, cur.1, col, thick);
                }
                prev = Some(cur);
            }
        } else if t.starts_with("<text ") {
            let Some(open_end) = t.find('>') else {
                continue;
            };
            let Some(close) = t.rfind("</text>") else {
                continue;
            };
            if close <= open_end {
                continue;
            }
            let content = unescape(&t[open_end + 1..close]);
            let x = attr_f64(t, "x", 0.0);
            let y = attr_f64(t, "y", 0.0);
            let size = attr_f64(t, "font-size", 12.0);
            let anchor = attr(t, "text-anchor").unwrap_or("start");
            let col = color(t, "fill");
            if let Some(tr) = attr(t, "transform") {
                if tr.contains("rotate(-90") {
                    c.text_rotated(x, y, &content, size, col);
                    continue;
                }
            }
            c.text(x, y, &content, size, anchor, col);
        }
    }
    c.to_png()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::load_csv_text;

    const SALES: &str = "region,size,price,churned\n\
north,55,140,yes\n\
east,60,160,yes\n\
south,45,125,no\n\
west,50,130,no\n";

    #[test]
    fn png_is_deterministic_and_has_the_fixed_shape() {
        let t = load_csv_text(SALES).unwrap();
        let a = render_png(
            &t,
            t.column_index("price").unwrap(),
            t.column_index("size").unwrap(),
            None,
        )
        .unwrap();
        let b = render_png(
            &t,
            t.column_index("price").unwrap(),
            t.column_index("size").unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(a, b, "the PNG bytes must be deterministic");
        assert_eq!(&a[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        assert_eq!(&a[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes(a[16..20].try_into().unwrap()), 800);
        assert_eq!(u32::from_be_bytes(a[20..24].try_into().unwrap()), 480);
        assert!(a.len() > 1000, "a chart PNG should not be empty");
    }

    #[test]
    fn dots_are_deterministic_and_have_the_fixed_shape() {
        let t = load_csv_text(SALES).unwrap();
        let a = render_svg(
            &t,
            t.column_index("price").unwrap(),
            t.column_index("size").unwrap(),
            None,
        )
        .unwrap();
        let b = render_svg(
            &t,
            t.column_index("price").unwrap(),
            t.column_index("size").unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(a, b, "the bytes must be deterministic");
        assert!(a.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(a.contains("width=\"800\""));
        assert!(a.contains("height=\"480\""));
        assert!(a.contains("fill=\"#1f6f8b\""));
        assert!(a.contains(">size</text>"));
        assert!(a.contains(">price</text>"));
        assert!(a.contains("<circle"));
        assert!(!a.contains("<rect x=\"70"));
    }

    #[test]
    fn bars_total_by_category() {
        let t = load_csv_text(SALES).unwrap();
        let svg = render_svg(
            &t,
            t.column_index("price").unwrap(),
            t.column_index("region").unwrap(),
            Some(PlotKind::Bars),
        )
        .unwrap();
        assert!(svg.contains("<rect"));
        assert!(svg.contains(">north</text>"));
        assert!(svg.contains(">east</text>"));
    }

    #[test]
    fn a_text_horizontal_with_dots_is_t0184() {
        let t = load_csv_text(SALES).unwrap();
        let err = render_svg(
            &t,
            t.column_index("price").unwrap(),
            t.column_index("region").unwrap(),
            Some(PlotKind::Dots),
        )
        .unwrap_err();
        assert_eq!(err.message(), "Column not numeric.");
    }
}
//! A tiny deterministic RGB canvas and PNG encoder for chart output.
//!
//! The chart writer emits SVG; this module rasterizes the small SVG subset
//! it uses (rect, line, circle, polyline, text) into an 800x480 PNG. Text
//! uses the public-domain 8x8 bitmap font in `font8x8`. The encoder writes
//! 8-bit RGB, filter 0, with a zlib IDAT from `flate2`, so the bytes are
//! reproducible on Windows and Linux.

use crate::font8x8::FONT8X8;
use std::io::Write;

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>, // RGB, 3 bytes per pixel
}

impl Canvas {
    pub fn new(w: usize, h: usize, fill: [u8; 3]) -> Self {
        let mut px = Vec::with_capacity(w * h * 3);
        for _ in 0..w * h {
            px.extend_from_slice(&fill);
        }
        Canvas { w, h, px }
    }

    fn set(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 3;
        self.px[i] = c[0];
        self.px[i + 1] = c[1];
        self.px[i + 2] = c[2];
    }

    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, c: [u8; 3]) {
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + w).ceil().min(self.w as f64) as i32;
        let y1 = (y + h).ceil().min(self.h as f64) as i32;
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, c: [u8; 3], thick: f64) {
        let mut x0 = x0.round() as i32;
        let mut y0 = y0.round() as i32;
        let x1 = x1.round() as i32;
        let y1 = y1.round() as i32;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let r = (thick / 2.0).floor().max(0.0) as i32;
        loop {
            for oy in -r..=r {
                for ox in -r..=r {
                    self.set(x0 + ox, y0 + oy, c);
                }
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, c: [u8; 3]) {
        let r = r.max(0.5);
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            let span = (r * r - (dy * dy) as f64).max(0.0).sqrt() as i32;
            for dx in -span..=span {
                self.set(cx.round() as i32 + dx, cy.round() as i32 + dy, c);
            }
        }
    }

    pub fn text(&mut self, x: f64, y: f64, text: &str, size: f64, anchor: &str, c: [u8; 3]) {
        let scale = if size >= 16.0 { 2 } else { 1 };
        let (w, h, mask) = text_mask(text, scale);
        let x0 = match anchor {
            "middle" => x - w as f64 / 2.0,
            "end" => x - w as f64,
            _ => x,
        };
        let y0 = y - h as f64 + scale as f64;
        self.blit(x0.round() as i32, y0.round() as i32, w, h, &mask, c);
    }

    pub fn text_rotated(&mut self, cx: f64, cy: f64, text: &str, size: f64, c: [u8; 3]) {
        let scale = if size >= 16.0 { 2 } else { 1 };
        let (w, h, mask) = text_mask(text, scale);
        // Rotate 90 degrees counter-clockwise: (x, y) -> (y, w-1-x).
        let nw = h;
        let nh = w;
        let mut rot = vec![false; nw * nh];
        for y in 0..h {
            for x in 0..w {
                if mask[y * w + x] {
                    let nx = y;
                    let ny = w - 1 - x;
                    rot[ny * nw + nx] = true;
                }
            }
        }
        let x0 = cx.round() as i32 - nw as i32 / 2;
        let y0 = cy.round() as i32 - nh as i32 / 2;
        self.blit(x0, y0, nw, nh, &rot, c);
    }

    fn blit(&mut self, x0: i32, y0: i32, w: usize, h: usize, mask: &[bool], c: [u8; 3]) {
        for y in 0..h {
            for x in 0..w {
                if mask[y * w + x] {
                    self.set(x0 + x as i32, y0 + y as i32, c);
                }
            }
        }
    }

    /// Encode as an 8-bit RGB PNG with filter 0 on every scanline.
    pub fn to_png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity((self.w * 3 + 1) * self.h);
        for y in 0..self.h {
            raw.push(0); // filter: none
            let start = y * self.w * 3;
            raw.extend_from_slice(&self.px[start..start + self.w * 3]);
        }
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw).expect("in-memory zlib");
        let idat = enc.finish().expect("in-memory zlib");

        let mut out = Vec::new();
        out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB, deflate, no interlace
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &idat);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

fn text_mask(text: &str, scale: usize) -> (usize, usize, Vec<bool>) {
    let bytes: Vec<u8> = text
        .bytes()
        .map(|b| if b < 128 { b } else { b'?' })
        .collect();
    let w = bytes.len() * 8 * scale;
    let h = 8 * scale;
    let mut mask = vec![false; w * h];
    for (i, b) in bytes.iter().enumerate() {
        let g = &FONT8X8[*b as usize];
        for row in 0..8 {
            for col in 0..8 {
                if (g[row] >> col) & 1 != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = i * 8 * scale + col * scale + sx;
                            let py = row * scale + sy;
                            mask[py * w + px] = true;
                        }
                    }
                }
            }
        }
    }
    (w, h, mask)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// CRC-32 (IEEE), table-based; PNG chunks use the same polynomial as zip.
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        table[i as usize] = c;
    }
    let mut crc = 0xffff_ffffu32;
    for b in data {
        crc = table[((crc ^ (*b as u32)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}
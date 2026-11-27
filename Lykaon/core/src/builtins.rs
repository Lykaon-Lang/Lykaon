//! M6b standard-library built-ins.
//!
//! These are real functions callable from a Lykaon program by name, resolved
//! by the interpreter through the same function-call path as user functions.
//! They fill the gaps found by `docs/stdlib-audit.md`: math functions, regex
//! replace/split, sha256/md5, base64, struct pack/unpack, subprocess capture,
//! a default map, raw TCP sockets, and statistics mode/stdev.
//!
//! Every function is total unless `can_fail` is set; a failure is a normal
//! Lykaon failure, never a silent workaround.

use crate::sem::{Kind, Value};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct Builtin {
    pub name: &'static str,
    pub inputs: Vec<Kind>,
    pub result: Kind,
    pub can_fail: bool,
    pub f: fn(&[Value]) -> Result<Value, String>,
}

pub fn all() -> Vec<Builtin> {
    vec![
        // ---- math ----
        bi("sqrt", &[K::Decimal], K::Decimal, false, b_sqrt),
        bi("floor", &[K::Decimal], K::Whole, false, b_floor),
        bi("ceiling", &[K::Decimal], K::Whole, false, b_ceil),
        // `log` is a reserved sentence word (`Log info ...`, `Use log level`),
        // so the natural logarithm is spelled out: `logarithm`.
        bi("logarithm", &[K::Decimal], K::Decimal, false, b_log),
        bi("power", &[K::Decimal, K::Decimal], K::Decimal, false, b_pow),
        // ---- hashlib ----
        bi("sha256", &[K::Text], K::Text, false, b_sha256),
        bi("md5", &[K::Text], K::Text, false, b_md5),
        // ---- base64 ----
        bi("b64encode", &[K::Text], K::Text, false, b_b64e),
        bi("b64decode", &[K::Text], K::Text, true, b_b64d),
        // ---- struct ----
        bi("packwhole", &[K::Whole], K::Text, false, b_packwhole),
        bi("unpackwhole", &[K::Text], K::Whole, true, b_unpackwhole),
        bi("packdecimal", &[K::Decimal], K::Text, false, b_pack_dec),
        bi("unpackdecimal", &[K::Text], K::Decimal, true, b_unpack_dec),
        // ---- subprocess ----
        bi("capture", &[K::Text], K::Text, true, b_capture),
        // ---- collections ----
        bi(
            "mapdefault",
            &[K::Unknown, K::Text, K::Unknown],
            K::Unknown,
            false,
            b_mapdefault,
        ),
        // ---- re ----
        bi(
            "regexreplace",
            &[K::Text, K::Text, K::Text],
            K::Text,
            false,
            b_regexreplace,
        ),
        bi(
            "regexsplit",
            &[K::Text, K::Text],
            K::List(Box::new(K::Text)),
            false,
            b_regexsplit,
        ),
        // ---- socket ----
        bi("tcpopen", &[K::Text, K::Whole], K::Connection, true, b_tcpopen),
        bi("tcpsend", &[K::Connection, K::Text], K::Whole, true, b_tcpsend),
        bi("tcpclose", &[K::Connection], K::Whole, false, b_tcpclose),
        // ---- statistics ----
        bi("mode", &[K::Unknown], K::Unknown, false, b_mode),
        bi("stdev", &[K::Unknown], K::Decimal, false, b_stdev),
    ]
}

fn bi(
    name: &'static str,
    inputs: &[K],
    result: K,
    can_fail: bool,
    f: fn(&[Value]) -> Result<Value, String>,
) -> Builtin {
    Builtin {
        name,
        inputs: inputs.to_vec(),
        result,
        can_fail,
        f,
    }
}

// A local alias so the table above stays short.
use crate::sem::Kind as K;

pub fn find(name: &str) -> Option<Builtin> {
    all().into_iter().find(|b| b.name.eq_ignore_ascii_case(name))
}

// ---- value helpers --------------------------------------------------------

fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::Whole(n) => Ok(*n as f64),
        Value::Dec(x) => Ok(*x),
        Value::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        _ => Err("a number is needed".to_string()),
    }
}

fn as_text(v: &Value) -> Result<String, String> {
    match v {
        Value::Text(s) => Ok(s.clone()),
        _ => Err("text is needed".to_string()),
    }
}

fn as_list(v: &Value) -> Result<Vec<Value>, String> {
    match v {
        Value::List(xs) => Ok(xs.clone()),
        _ => Err("a list is needed".to_string()),
    }
}

fn num(x: f64) -> Value {
    if x.fract() == 0.0 && x.is_finite() && x.abs() < 9.0e15 {
        Value::Whole(x as i64)
    } else {
        Value::Dec(x)
    }
}

// ---- math -----------------------------------------------------------------

fn b_sqrt(a: &[Value]) -> Result<Value, String> {
    let x = as_f64(&a[0])?;
    if x < 0.0 {
        return Err("the square root of a negative number is not real".to_string());
    }
    Ok(num(x.sqrt()))
}
fn b_floor(a: &[Value]) -> Result<Value, String> {
    Ok(Value::Whole(as_f64(&a[0])?.floor() as i64))
}
fn b_ceil(a: &[Value]) -> Result<Value, String> {
    Ok(Value::Whole(as_f64(&a[0])?.ceil() as i64))
}
fn b_log(a: &[Value]) -> Result<Value, String> {
    let x = as_f64(&a[0])?;
    if x <= 0.0 {
        return Err("the logarithm needs a positive number".to_string());
    }
    Ok(Value::Dec(x.ln()))
}
fn b_pow(a: &[Value]) -> Result<Value, String> {
    Ok(num(as_f64(&a[0])?.powf(as_f64(&a[1])?)))
}

// ---- hashlib --------------------------------------------------------------

fn b_sha256(a: &[Value]) -> Result<Value, String> {
    use sha2::{Digest, Sha256};
    let t = as_text(&a[0])?;
    let d = Sha256::digest(t.as_bytes());
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    Ok(Value::Text(s))
}

fn b_md5(a: &[Value]) -> Result<Value, String> {
    let t = as_text(&a[0])?;
    let d = md5_hex(t.as_bytes());
    Ok(Value::Text(d))
}

// ---- base64 ---------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b_b64e(a: &[Value]) -> Result<Value, String> {
    let t = as_text(&a[0])?;
    let data = t.as_bytes();
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        out.push(B64[(b[0] >> 2) as usize] as char);
        out.push(B64[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(b[2] & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    Ok(Value::Text(out))
}

fn b_b64d(a: &[Value]) -> Result<Value, String> {
    let t = as_text(&a[0])?;
    let mut rev = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let bytes: Vec<u8> = t.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return Err("bad base64".to_string());
        }
        let mut n = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            let v = rev[*b as usize];
            if v == 255 {
                return Err("bad base64".to_string());
            }
            n |= (v as u32) << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(Value::Text(String::from_utf8_lossy(&out).to_string()))
}

// ---- struct ---------------------------------------------------------------

fn b_packwhole(a: &[Value]) -> Result<Value, String> {
    let n = match &a[0] {
        Value::Whole(n) => *n,
        _ => return Err("a whole number is needed".to_string()),
    };
    Ok(Value::Text(String::from_utf8_lossy(&n.to_le_bytes()).to_string()))
}
fn b_unpackwhole(a: &[Value]) -> Result<Value, String> {
    let t = as_text(&a[0])?;
    let b = t.as_bytes();
    if b.len() < 8 {
        return Err("packwhole needs 8 bytes".to_string());
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&b[..8]);
    Ok(Value::Whole(i64::from_le_bytes(arr)))
}
fn b_pack_dec(a: &[Value]) -> Result<Value, String> {
    let x = as_f64(&a[0])?;
    Ok(Value::Text(String::from_utf8_lossy(&x.to_le_bytes()).to_string()))
}
fn b_unpack_dec(a: &[Value]) -> Result<Value, String> {
    let t = as_text(&a[0])?;
    let b = t.as_bytes();
    if b.len() < 8 {
        return Err("packdecimal needs 8 bytes".to_string());
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&b[..8]);
    Ok(Value::Dec(f64::from_le_bytes(arr)))
}

// ---- subprocess -----------------------------------------------------------

fn b_capture(a: &[Value]) -> Result<Value, String> {
    let cmd = as_text(&a[0])?;
    let out = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", &cmd])
            .output()
    } else {
        std::process::Command::new("sh").arg("-c").arg(&cmd).output()
    };
    let out = out.map_err(|e| format!("cannot run the command: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the command failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(Value::Text(String::from_utf8_lossy(&out.stdout).to_string()))
}

// ---- collections ----------------------------------------------------------

fn b_mapdefault(a: &[Value]) -> Result<Value, String> {
    let default = a[2].clone();
    if let Value::Map(pairs) = &a[0] {
        for (k, v) in pairs {
            if k == &a[1] {
                return Ok(v.clone());
            }
        }
    }
    Ok(default)
}

// ---- re (a small backtracking engine) ------------------------------------

fn b_regexreplace(a: &[Value]) -> Result<Value, String> {
    let text = as_text(&a[0])?;
    let pat = as_text(&a[1])?;
    let rep = as_text(&a[2])?;
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i <= chars.len() {
        if let Some(end) = regex_match_at(&pat, &chars, i) {
            out.push_str(&rep);
            i = end;
        } else if i < chars.len() {
            out.push(chars[i]);
            i += 1;
        } else {
            break;
        }
    }
    Ok(Value::Text(out))
}

fn b_regexsplit(a: &[Value]) -> Result<Value, String> {
    let text = as_text(&a[0])?;
    let pat = as_text(&a[1])?;
    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut i = 0usize;
    while i <= chars.len() {
        if let Some(end) = regex_match_at(&pat, &chars, i) {
            if end == i {
                // zero-width match: skip one char to avoid a loop
                if i < chars.len() {
                    cur.push(chars[i]);
                    i += 1;
                    continue;
                }
            }
            parts.push(Value::Text(cur.clone()));
            cur.clear();
            i = end;
        } else if i < chars.len() {
            cur.push(chars[i]);
            i += 1;
        } else {
            break;
        }
    }
    parts.push(Value::Text(cur));
    Ok(Value::List(parts))
}

/// Try to match `pat` at `chars[start..]`; return the end index on success.
fn regex_match_at(pat: &str, chars: &[char], start: usize) -> Option<usize> {
    let p: Vec<char> = pat.chars().collect();
    let anchored_start = p.first() == Some(&'^');
    let anchored_end = p.last() == Some(&'$') && p.len() > 1;
    if anchored_start && start != 0 {
        return None;
    }
    let body: &[char] = if anchored_start { &p[1..] } else { &p[..] };
    let body = if anchored_end { &body[..body.len() - 1] } else { body };
    let ends = match_here(body, chars, start)?;
    if anchored_end && ends != chars.len() {
        return None;
    }
    Some(ends)
}

/// Match a sequence of (atom, quantifier) items starting at `si`; return the
/// end index of the first overall match (greedy with backtracking).
fn match_here(pat: &[char], chars: &[char], si: usize) -> Option<usize> {
    if pat.is_empty() {
        return Some(si);
    }
    let (atom, quant, rest) = split_atom(pat);
    match quant {
        Quant::One => {
            let next = match_atom(atom, chars, si)?;
            match_here(rest, chars, next)
        }
        Quant::Opt => {
            if let Some(next) = match_atom(atom, chars, si) {
                if let Some(e) = match_here(rest, chars, next) {
                    return Some(e);
                }
            }
            match_here(rest, chars, si)
        }
        Quant::Star => {
            let mut count = 0usize;
            let mut pos = si;
            while let Some(next) = match_atom(atom, chars, pos) {
                pos = next;
                count += 1;
                if count > chars.len() + 1 {
                    break;
                }
            }
            loop {
                if let Some(e) = match_here(rest, chars, pos) {
                    return Some(e);
                }
                if pos == si {
                    return None;
                }
                pos -= 1;
            }
        }
        Quant::Plus => {
            let first = match_atom(atom, chars, si)?;
            let mut count = 0usize;
            let mut pos = first;
            loop {
                if let Some(e) = match_here(rest, chars, pos) {
                    return Some(e);
                }
                if count > chars.len() + 1 {
                    return None;
                }
                match match_atom(atom, chars, pos) {
                    Some(next) => {
                        pos = next;
                        count += 1;
                    }
                    None => return None,
                }
            }
        }
    }
}

enum Quant {
    One,
    Opt,
    Star,
    Plus,
}

/// Split the next atom and its quantifier from the front of `pat`.
fn split_atom(pat: &[char]) -> (&[char], Quant, &[char]) {
    // pat is non-empty.
    let (atom_len, quant) = if pat[0] == '\\' && pat.len() > 1 {
        (2, Quant::One)
    } else if pat[0] == '[' {
        let mut i = 1;
        if i < pat.len() && pat[i] == '^' {
            i += 1;
        }
        while i < pat.len() && pat[i] != ']' {
            i += 1;
        }
        (i + 1, Quant::One)
    } else {
        (1, Quant::One)
    };
    let mut i = atom_len;
    let quant = if i < pat.len() {
        match pat[i] {
            '*' => {
                i += 1;
                Quant::Star
            }
            '+' => {
                i += 1;
                Quant::Plus
            }
            '?' => {
                i += 1;
                Quant::Opt
            }
            _ => quant,
        }
    } else {
        quant
    };
    (&pat[..atom_len], quant, &pat[i..])
}

fn match_atom(atom: &[char], chars: &[char], si: usize) -> Option<usize> {
    if si >= chars.len() {
        return None;
    }
    let c = chars[si];
    if atom.is_empty() {
        return None;
    }
    if atom[0] == '\\' && atom.len() > 1 {
        let ok = match atom[1] {
            'd' => c.is_ascii_digit(),
            'w' => c.is_alphanumeric() || c == '_',
            's' => c.is_whitespace(),
            'n' => c == '\n',
            't' => c == '\t',
            other => c == other,
        };
        return if ok { Some(si + 1) } else { None };
    }
    if atom[0] == '[' {
        let mut i = 1;
        let neg = i < atom.len() && atom[i] == '^';
        if neg {
            i += 1;
        }
        let mut hit = false;
        while i < atom.len() && atom[i] != ']' {
            if i + 2 < atom.len() && atom[i + 1] == '-' && atom[i + 2] != ']' {
                if c >= atom[i] && c <= atom[i + 2] {
                    hit = true;
                }
                i += 3;
            } else {
                if c == atom[i] {
                    hit = true;
                }
                i += 1;
            }
        }
        return if hit != neg { Some(si + 1) } else { None };
    }
    if atom[0] == '.' {
        return Some(si + 1);
    }
    if c == atom[0] {
        Some(si + 1)
    } else {
        None
    }
}

// ---- socket ---------------------------------------------------------------

thread_local! {
    static SOCKS: RefCell<HashMap<u64, std::net::TcpStream>> = RefCell::new(HashMap::new());
    static NEXT_ID: RefCell<u64> = const { RefCell::new(1) };
}

fn b_tcpopen(a: &[Value]) -> Result<Value, String> {
    use std::net::TcpStream;
    let host = as_text(&a[0])?;
    let port = match &a[1] {
        Value::Whole(n) => *n as u16,
        _ => return Err("a whole port is needed".to_string()),
    };
    let s = TcpStream::connect((host.as_str(), port)).map_err(|e| format!("cannot connect: {e}"))?;
    let id = NEXT_ID.with(|n| {
        let mut n = n.borrow_mut();
        let id = *n;
        *n += 1;
        id
    });
    SOCKS.with(|m| m.borrow_mut().insert(id, s));
    Ok(Value::Conn(id))
}

fn b_tcpsend(a: &[Value]) -> Result<Value, String> {
    use std::io::Write;
    let id = match &a[0] {
        Value::Conn(id) => *id,
        _ => return Err("a connection is needed".to_string()),
    };
    let text = as_text(&a[1])?;
    SOCKS.with(|m| {
        let mut m = m.borrow_mut();
        let s = m.get_mut(&id).ok_or("the connection is closed")?;
        s.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        Ok(Value::Whole(text.len() as i64))
    })
}

fn b_tcpclose(a: &[Value]) -> Result<Value, String> {
    let id = match &a[0] {
        Value::Conn(id) => *id,
        _ => return Err("a connection is needed".to_string()),
    };
    let n = SOCKS.with(|m| m.borrow_mut().remove(&id)).is_some();
    Ok(Value::Whole(if n { 1 } else { 0 }))
}

// ---- statistics -----------------------------------------------------------

fn b_mode(a: &[Value]) -> Result<Value, String> {
    let xs = as_list(&a[0])?;
    if xs.is_empty() {
        return Ok(Value::Missing);
    }
    let mut best: Option<(Value, usize)> = None;
    for x in &xs {
        let c = xs.iter().filter(|y| *y == x).count();
        if best.as_ref().map(|(_, bc)| c > *bc).unwrap_or(true) {
            best = Some((x.clone(), c));
        }
    }
    Ok(best.map(|(v, _)| v).unwrap_or(Value::Missing))
}

fn b_stdev(a: &[Value]) -> Result<Value, String> {
    let xs = as_list(&a[0])?;
    if xs.is_empty() {
        return Ok(Value::Dec(0.0));
    }
    let v: Vec<f64> = xs.iter().map(as_f64).collect::<Result<_, _>>()?;
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    Ok(Value::Dec(var.sqrt()))
}

// ---- md5 (RFC 1321, compact) ---------------------------------------------

fn md5_hex(input: &[u8]) -> String {
    let d = md5_digest(input);
    let mut s = String::with_capacity(32);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn md5_digest(msg: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6,
        10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;
    let mut data = msg.to_vec();
    let bitlen = (msg.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bitlen.to_le_bytes());
    for chunk in data.chunks(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(K[i])
                    .wrapping_add(m[g])
                    .rotate_left(S[i]),
            );
            a = tmp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}
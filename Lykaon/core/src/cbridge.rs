//! M6 C bridge: `.stub.shelf` declarations and record layout phrases.
//!
//! Grammar: M6-G33/G34 and M6-G38..G40. The stub parser is standalone; the
//! compiler will call it when `Use C library` is wired.

#[derive(Clone, Debug, PartialEq)]
pub enum CKind {
    Whole,
    Decimal,
    YesNo,
    Letter,
    Text,
    Bytes,
    Record(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CDecl {
    pub symbol: String,
    pub name: String,
    /// The library named by `Use C library NAME.` (filled by the compiler).
    pub library: String,
    pub inputs: Vec<CKind>,
    pub result: CKind,
    pub can_fail: bool,
    pub safe: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub as_c: bool,
    pub packed: bool,
    pub align: Option<usize>,
}

/// Parse a `.stub.shelf` file. Each non-empty line is one C declaration.
pub fn parse_stub(source: &str) -> Result<Vec<CDecl>, String> {
    let mut out = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("Note ") {
            continue;
        }
        let line = line.strip_suffix('.').unwrap_or(line).trim();
        let lw: Vec<&str> = line.split_whitespace().collect();
        let mut p = 0usize;
        if !lw.first().is_some_and(|w| w.eq_ignore_ascii_case("c")) {
            return Err(format!("line {}: a stub line must start with C", i + 1));
        }
        p += 1;
        let safe = lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("safe"));
        if safe {
            p += 1;
        }
        if !lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("function")) {
            return Err(format!("line {}: expected function", i + 1));
        }
        p += 1;
        let symbol = lw
            .get(p)
            .ok_or_else(|| format!("line {}: missing C symbol", i + 1))?
            .to_string();
        p += 1;
        if !lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("as")) {
            return Err(format!("line {}: expected as", i + 1));
        }
        p += 1;
        let name = lw
            .get(p)
            .ok_or_else(|| format!("line {}: missing Tern name", i + 1))?
            .to_string();
        p += 1;
        if !lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("takes")) {
            return Err(format!("line {}: expected takes", i + 1));
        }
        let mut inputs = Vec::new();
        let takes_at = p;
        let gives_at = lw
            .iter()
            .position(|w| w.eq_ignore_ascii_case("gives"))
            .ok_or_else(|| format!("line {}: expected gives", i + 1))?;
        let input_words: Vec<&str> = lw[takes_at + 1..gives_at]
            .iter()
            .copied()
            .filter(|w| !w.eq_ignore_ascii_case("and"))
            .collect();
        if !input_words.is_empty() && !input_words[0].eq_ignore_ascii_case("nothing") {
            let mut j = 0;
            while j < input_words.len() {
                let (kind, used) = parse_ckind(&input_words[j..])?;
                inputs.push(kind);
                j += used;
            }
        }
        let result_words: Vec<&str> = lw[gives_at + 1..]
            .iter()
            .copied()
            .filter(|w| !w.eq_ignore_ascii_case("and") || !w.eq_ignore_ascii_case("can"))
            .collect();
        // `and can fail` is not a kind; strip it.
        let mut result_end = result_words.len();
        if result_words.len() >= 2
            && result_words[result_words.len() - 2].eq_ignore_ascii_case("can")
            && result_words[result_words.len() - 1].eq_ignore_ascii_case("fail")
        {
            result_end -= 2;
        }
        let (result, _) = parse_ckind(&result_words[..result_end])?;
        let can_fail = line.to_ascii_lowercase().contains("can fail");
        out.push(CDecl {
            symbol,
            name,
            library: String::new(),
            inputs,
            result,
            can_fail,
            safe,
        });
    }
    Ok(out)
}

pub fn parse_ckind(words: &[&str]) -> Result<(CKind, usize), String> {
    if words.is_empty() {
        return Err("missing kind".to_string());
    }
    let w = words[0].to_ascii_lowercase();
    match w.as_str() {
        "whole" if words.get(1).is_some_and(|x| x.eq_ignore_ascii_case("number")) => {
            Ok((CKind::Whole, 2))
        }
        "yes" if words.get(1).is_some_and(|x| x.eq_ignore_ascii_case("or")) => Ok((CKind::YesNo, 2)),
        "decimal" => Ok((CKind::Decimal, 1)),
        "letter" => Ok((CKind::Letter, 1)),
        "text" => Ok((CKind::Text, 1)),
        "bytes" => Ok((CKind::Bytes, 1)),
        other => Ok((CKind::Record(other.to_string()), 1)),
    }
}

/// Parse the layout phrases after a record's last field (M6-G33).
pub fn parse_layout(words: &[&str]) -> Result<Layout, String> {
    let mut layout = Layout::default();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].to_ascii_lowercase();
        match w.as_str() {
            "as" if words.get(i + 1).is_some_and(|x| x.eq_ignore_ascii_case("c")) => {
                layout.as_c = true;
                i += 2;
                if words.get(i).is_some_and(|x| x.eq_ignore_ascii_case("does")) {
                    i += 1;
                }
            }
            "packed" => {
                layout.packed = true;
                i += 1;
            }
            "aligned" if words.get(i + 1).is_some_and(|x| x.eq_ignore_ascii_case("to")) => {
                let n = words
                    .get(i + 2)
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or("aligned to needs a number")?;
                if n == 0 || !n.is_power_of_two() || n > 4096 {
                    return Err("aligned to needs a power of 2 from 1 to 4096".to_string());
                }
                if layout.packed {
                    return Err("T0192: packed and aligned to conflict".to_string());
                }
                layout.align = Some(n);
                i += 3;
            }
            "and" => i += 1,
            other => return Err(format!("unknown layout phrase: {other}")),
        }
    }
    if layout.packed && layout.align.is_some() {
        return Err("T0192: packed and aligned to conflict".to_string());
    }
    Ok(layout)
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldLayout {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

/// A loaded plugin (M6-G35).
#[derive(Clone, Debug)]
pub struct Plugin {
    pub name: String,
    pub handle: usize,
}

/// Load a plugin from `plugins/<name>.dll`, `lib<name>.so` or `<name>.so`.
pub fn load_plugin(name: &str, dir: &std::path::Path) -> Result<Plugin, String> {
    let candidates = [
        dir.join(format!("{name}.dll")),
        dir.join(format!("lib{name}.so")),
        dir.join(format!("{name}.so")),
    ];
    for p in &candidates {
        if p.is_file() {
            let handle = load_library(&p.to_string_lossy())?;
            return Ok(Plugin {
                name: name.to_string(),
                handle,
            });
        }
    }
    Err(format!("plugin {name} is not found"))
}

/// Resolve a symbol from a loaded plugin.
pub fn plugin_symbol(plugin: &Plugin, symbol: &str) -> Result<usize, String> {
    resolve_symbol(plugin.handle, symbol)
}

/// Generate a C header for `Share for C.` exports (M6-G32).
pub fn generate_header(exports: &[(String, Vec<CKind>, CKind, bool)]) -> String {
    let mut s = String::new();
    s.push_str("#ifndef LYKAON_EXPORTS_H\n#define LYKAON_EXPORTS_H\n\n");
    s.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");
    for (name, inputs, result, can_fail) in exports {
        s.push_str(&format!("{} {}(", c_type(result), name));
        if inputs.is_empty() {
            s.push_str("void");
        } else {
            let args: Vec<String> = inputs
                .iter()
                .enumerate()
                .map(|(i, k)| format!("{} a{}", c_type(k), i))
                .collect();
            s.push_str(&args.join(", "));
        }
        s.push_str(");\n");
        if *can_fail {
            s.push_str(&format!("int {}_can_fail(void);\n", name));
        }
    }
    s.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n#endif\n");
    s
}

fn c_type(k: &CKind) -> &'static str {
    match k {
        CKind::Whole => "int64_t",
        CKind::Decimal => "double",
        CKind::YesNo => "int",
        CKind::Letter => "char",
        CKind::Text | CKind::Bytes => "const char *",
        CKind::Record(_) => "void *",
    }
}

/// Load a dynamic library by file name (M6-G31). The handle is an opaque
/// platform value; use `resolve_symbol` to look up a C symbol.
pub fn load_library(file: &str) -> Result<usize, String> {
    load_library_impl(file, false)
}

/// Load a dynamic library with its symbols in the global namespace
/// (`RTLD_GLOBAL` on Unix). Python C extensions resolve `libpython` symbols
/// against the global namespace, so the interpreter library must be loaded
/// this way; loading it `RTLD_LOCAL` makes numpy fail with
/// `undefined symbol: PyObject_SelfIter`.
pub fn load_library_global(file: &str) -> Result<usize, String> {
    load_library_impl(file, true)
}

fn load_library_impl(file: &str, global: bool) -> Result<usize, String> {
    let c = std::ffi::CString::new(file).map_err(|_| "bad library name".to_string())?;
    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn dlopen(filename: *const std::os::raw::c_char, flag: i32) -> *mut std::os::raw::c_void;
        }
        // RTLD_NOW = 2, RTLD_GLOBAL = 0x100.
        let flag = if global { 2 | 0x100 } else { 2 };
        let h = dlopen(c.as_ptr(), flag);
        if h.is_null() {
            Err("the library cannot be loaded".to_string())
        } else {
            Ok(h as usize)
        }
    }
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" {
            fn LoadLibraryA(name: *const u8) -> *mut std::os::raw::c_void;
        }
        let _ = global; // LoadLibraryA already puts the library in the process namespace.
        let h = LoadLibraryA(c.as_ptr() as *const u8);
        if h.is_null() {
            Err("the library cannot be loaded".to_string())
        } else {
            Ok(h as usize)
        }
    }
}

thread_local! {
    static LIBS: std::cell::RefCell<std::collections::HashMap<String, usize>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    static C_FUNCS: std::cell::RefCell<Vec<CDecl>> =
        std::cell::RefCell::new(Vec::new());
}

/// Register the C functions declared by `.stub.shelf` files before parsing.
pub fn register_c_functions(decls: &[CDecl]) {
    C_FUNCS.with(|m| {
        *m.borrow_mut() = decls.to_vec();
    });
}

/// The registered C functions.
pub fn c_functions() -> Vec<CDecl> {
    C_FUNCS.with(|m| m.borrow().clone())
}

/// Load a C library by the name used in `Use C library NAME.` (M6-G31).
pub fn load_library_name(name: &str) -> Result<usize, String> {
    load_library_name_at(std::path::Path::new("."), name)
}

/// Load a C library, looking in `dir` first.
pub fn load_library_name_at(dir: &std::path::Path, name: &str) -> Result<usize, String> {
    if let Some(h) = LIBS.with(|m| m.borrow().get(name).copied()) {
        return Ok(h);
    }
    let candidates = [
        dir.join(format!("lib{name}.so")),
        dir.join(format!("{name}.so")),
        dir.join(format!("{name}.dll")),
        dir.join(format!("lib{name}.dylib")),
        // Linux ships versioned sonames (libm.so.6); a bare `Use C library m.`
        // should find the system library without the user spelling the version.
        dir.join(format!("lib{name}.so.6")),
        dir.join(name),
    ];
    for c in &candidates {
        if c.is_file() {
            let h = load_library(&c.to_string_lossy())?;
            LIBS.with(|m| {
                m.borrow_mut().insert(name.to_string(), h);
            });
            return Ok(h);
        }
    }
    // System search path (dlopen/LoadLibrary) for names without a local file.
    for c in [
        name.to_string(),
        format!("{name}.dll"),
        format!("lib{name}.so"),
        format!("lib{name}.so.6"),
    ] {
        if let Ok(h) = load_library(&c) {
            LIBS.with(|m| {
                m.borrow_mut().insert(name.to_string(), h);
            });
            return Ok(h);
        }
    }
    Err(format!("the C library {name} is not found"))
}

/// The handle of a loaded C library, if any.
pub fn library_handle(name: &str) -> Option<usize> {
    LIBS.with(|m| m.borrow().get(name).copied())
}

/// Resolve a symbol from a loaded C library.
pub fn library_symbol(name: &str, symbol: &str) -> Result<usize, String> {
    let h = library_handle(name).ok_or_else(|| format!("the C library {name} is not loaded"))?;
    resolve_symbol(h, symbol)
}

/// Call a C function taking one `double` and returning `double`.
/// This is the M6 FFI call primitive; the language layer will use it for
/// stub-declared functions with that signature.
pub fn call_c_double(name: &str, symbol: &str, x: f64) -> Result<f64, String> {
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(f64) -> f64 = unsafe { std::mem::transmute(p) };
    Ok(f(x))
}

/// Call a C function taking one `const char *` and returning `double`.
pub fn call_c_text_double(name: &str, symbol: &str, x: &str) -> Result<f64, String> {
    use std::os::raw::c_char;
    let c = std::ffi::CString::new(x).map_err(|_| "bad text value".to_string())?;
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(*const c_char) -> f64 = unsafe { std::mem::transmute(p) };
    Ok(f(c.as_ptr()))
}

/// Call a C function taking one `const char *` and returning `int`.
pub fn call_c_text_int(name: &str, symbol: &str, x: &str) -> Result<i32, String> {
    use std::os::raw::c_char;
    let c = std::ffi::CString::new(x).map_err(|_| "bad text value".to_string())?;
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(*const c_char) -> i32 = unsafe { std::mem::transmute(p) };
    Ok(f(c.as_ptr()))
}

/// Call a C function taking one `const char *` and returning `char`.
pub fn call_c_text_letter(name: &str, symbol: &str, x: &str) -> Result<u8, String> {
    use std::os::raw::c_char;
    let c = std::ffi::CString::new(x).map_err(|_| "bad text value".to_string())?;
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(*const c_char) -> u8 = unsafe { std::mem::transmute(p) };
    Ok(f(c.as_ptr()))
}

/// Call a C function taking one `const char *` and returning `int64_t`.
pub fn call_c_text_whole(name: &str, symbol: &str, x: &str) -> Result<i64, String> {
    use std::os::raw::c_char;
    let c = std::ffi::CString::new(x).map_err(|_| "bad text value".to_string())?;
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(*const c_char) -> i64 = unsafe { std::mem::transmute(p) };
    Ok(f(c.as_ptr()))
}

/// Call a C function taking one `const char *` and returning `const char *`.
pub fn call_c_text(name: &str, symbol: &str, x: &str) -> Result<String, String> {
    use std::os::raw::c_char;
    let c = std::ffi::CString::new(x).map_err(|_| "bad text value".to_string())?;
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(*const c_char) -> *const c_char = unsafe { std::mem::transmute(p) };
    let r = f(c.as_ptr());
    if r.is_null() {
        Ok(String::new())
    } else {
        Ok(unsafe { std::ffi::CStr::from_ptr(r) }
            .to_string_lossy()
            .to_string())
    }
}

/// Call a C function taking one `int64_t` and returning `int64_t`.
pub fn call_c_whole(name: &str, symbol: &str, x: i64) -> Result<i64, String> {
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(p) };
    Ok(f(x))
}

/// Call a C function taking one `int64_t` and returning `int`.
pub fn call_c_int(name: &str, symbol: &str, x: i64) -> Result<i32, String> {
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(i64) -> i32 = unsafe { std::mem::transmute(p) };
    Ok(f(x))
}

/// Call a C function taking one `int64_t` and returning `char`.
pub fn call_c_letter(name: &str, symbol: &str, x: i64) -> Result<u8, String> {
    let p = library_symbol(name, symbol)?;
    let f: extern "C" fn(i64) -> u8 = unsafe { std::mem::transmute(p) };
    Ok(f(x))
}

/// Resolve a symbol from a library handle.
pub fn resolve_symbol(handle: usize, symbol: &str) -> Result<usize, String> {
    let c = std::ffi::CString::new(symbol).map_err(|_| "bad symbol".to_string())?;
    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn dlsym(handle: *mut std::os::raw::c_void, symbol: *const std::os::raw::c_char)
                -> *mut std::os::raw::c_void;
        }
        let p = dlsym(handle as *mut _, c.as_ptr());
        if p.is_null() {
            Err("the symbol is not found".to_string())
        } else {
            Ok(p as usize)
        }
    }
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" {
            fn GetProcAddress(module: *mut std::os::raw::c_void, name: *const u8)
                -> *mut std::os::raw::c_void;
        }
        let p = GetProcAddress(handle as *mut _, c.as_ptr() as *const u8);
        if p.is_null() {
            Err("the symbol is not found".to_string())
        } else {
            Ok(p as usize)
        }
    }
}

/// Compute the C ABI layout of a record (M6-G33/G34).
pub fn layout_record(
    fields: &[(String, CKind)],
    layout: &Layout,
) -> Result<(Vec<FieldLayout>, usize, usize), String> {
    let mut out = Vec::with_capacity(fields.len());
    let mut offset = 0usize;
    let mut max_align = 1usize;
    for (name, kind) in fields {
        let (size, natural_align) = match kind {
            CKind::Whole | CKind::Decimal => (8, 8),
            CKind::YesNo | CKind::Letter => (1, 1),
            CKind::Text | CKind::Bytes | CKind::Record(_) => (8, 8),
        };
        let align = if layout.packed {
            1
        } else {
            natural_align.min(layout.align.unwrap_or(usize::MAX))
        };
        if !layout.packed {
            offset = align_up(offset, align);
        }
        out.push(FieldLayout {
            name: name.clone(),
            offset,
            size,
        });
        offset += size;
        max_align = max_align.max(align);
    }
    if !layout.packed {
        offset = align_up(offset, max_align);
    }
    if let Some(n) = layout.align {
        offset = align_up(offset, n);
        max_align = max_align.max(n);
    }
    Ok((out, offset, max_align))
}

fn align_up(x: usize, align: usize) -> usize {
    if align <= 1 {
        x
    } else {
        (x + align - 1) / align * align
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_decl() {
        let d = parse_stub("C safe function cos as cosine takes decimal gives decimal.\n").unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].symbol, "cos");
        assert_eq!(d[0].name, "cosine");
        assert_eq!(d[0].inputs, vec![CKind::Decimal]);
        assert_eq!(d[0].result, CKind::Decimal);
        assert!(d[0].safe);
    }

    #[test]
    fn stub_takes_nothing_and_can_fail() {
        let d = parse_stub("C function now as now takes nothing gives whole number and can fail.\n")
            .unwrap();
        assert!(d[0].inputs.is_empty());
        assert_eq!(d[0].result, CKind::Whole);
        assert!(d[0].can_fail);
    }

    #[test]
    fn layout_phrases() {
        let l = parse_layout(&["as", "C", "does", "and", "aligned", "to", "16"]).unwrap();
        assert!(l.as_c);
        assert_eq!(l.align, Some(16));
        assert!(parse_layout(&["packed", "and", "aligned", "to", "16"]).is_err());
    }

    #[test]
    fn record_offsets() {
        let fields = vec![
            ("a".to_string(), CKind::Whole),
            ("b".to_string(), CKind::Text),
        ];
        let (layout, size, align) = layout_record(&fields, &Layout::default()).unwrap();
        assert_eq!(layout[1].offset, 8);
        assert_eq!(size, 16);
        assert_eq!(align, 8);
    }

    #[test]
    fn packed_record_has_no_padding() {
        let fields = vec![
            ("a".to_string(), CKind::Whole),
            ("b".to_string(), CKind::Letter),
        ];
        let layout = Layout {
            packed: true,
            ..Default::default()
        };
        let (fields, size, align) = layout_record(&fields, &layout).unwrap();
        assert_eq!(fields[1].offset, 8);
        assert_eq!(size, 9);
        assert_eq!(align, 1);
    }

    #[test]
    fn header_declares_exports() {
        let h = generate_header(&[(
            "double".to_string(),
            vec![CKind::Whole],
            CKind::Whole,
            false,
        )]);
        assert!(h.contains("int64_t double(int64_t a0);"));
    }

    #[cfg(unix)]
    #[test]
    fn load_libm_and_resolve_cos() {
        let h = load_library("libm.so.6")
            .or_else(|_| load_library("libm.so"))
            .expect("libm");
        resolve_symbol(h, "cos").expect("cos");
    }

    #[cfg(windows)]
    #[test]
    fn load_msvcrt_and_resolve_cos() {
        let h = load_library("msvcrt.dll").expect("msvcrt");
        resolve_symbol(h, "cos").expect("cos");
    }

    #[cfg(windows)]
    #[test]
    fn load_plugin_from_system32() {
        let p = load_plugin("msvcrt", std::path::Path::new(r"C:\Windows\System32")).expect("plugin");
        plugin_symbol(&p, "cos").expect("cos");
    }

    #[cfg(unix)]
    #[test]
    fn call_cos_from_libm() {
        load_library_name("m").expect("libm");
        let v = call_c_double("m", "cos", 0.0).expect("cos");
        assert!((v - 1.0).abs() < 1e-9, "cos(0) = {v}");
    }

    #[cfg(windows)]
    #[test]
    fn call_cos_from_msvcrt() {
        load_library_name("msvcrt").expect("msvcrt");
        let v = call_c_double("msvcrt", "cos", 0.0).expect("cos");
        assert!((v - 1.0).abs() < 1e-9, "cos(0) = {v}");
    }
}
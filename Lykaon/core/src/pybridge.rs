//! M6b Python library bridge.
//!
//! A Lykaon program calls a Python library through the Python C API, resolved
//! dynamically from the installed interpreter (`python3xx.dll` /
//! `libpython3xx.so`). No link-time dependency on Python: the symbols are
//! looked up at run time the same way the C bridge loads a `.dll`.
//!
//! Grammar (a `.py.shelf` file, one declaration per line):
//!
//! ```text
//! Python function sqrt as py_sqrt takes decimal gives decimal.
//! Python function dumps as py_dumps takes text gives text.
//! ```
//!
//! The module name is the file stem: `math.py.shelf` calls into the `math`
//! module, `numpy.py.shelf` into `numpy`. Every call is an unsafe boundary:
//! the values are converted at the edge and a Python error becomes a normal
//! Lykaon failure (never a silent workaround).

use crate::cbridge::{parse_ckind, CKind};
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

/// One Python function declared by a `.py.shelf` file.
#[derive(Clone, Debug, PartialEq)]
pub struct PyDecl {
    /// The Python attribute name (e.g. `sqrt`, `dumps`, `get`).
    pub symbol: String,
    /// The Lykaon name it is called by.
    pub name: String,
    /// The Python module named by the file stem (e.g. `math`, `numpy`).
    pub module: String,
    pub inputs: Vec<CKind>,
    pub result: CKind,
    pub can_fail: bool,
}

/// Parse a `.py.shelf` file. Each non-empty line is one Python declaration.
pub fn parse_py_stub(source: &str) -> Result<Vec<PyDecl>, String> {
    let mut out = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("Note ") {
            continue;
        }
        let line = line.strip_suffix('.').unwrap_or(line).trim();
        let lw: Vec<&str> = line.split_whitespace().collect();
        let mut p = 0usize;
        if !lw.first().is_some_and(|w| w.eq_ignore_ascii_case("python")) {
            return Err(format!("line {}: a stub line must start with Python", i + 1));
        }
        p += 1;
        if !lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("function")) {
            return Err(format!("line {}: expected function", i + 1));
        }
        p += 1;
        let symbol = lw
            .get(p)
            .ok_or_else(|| format!("line {}: missing Python symbol", i + 1))?
            .to_string();
        p += 1;
        if !lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("as")) {
            return Err(format!("line {}: expected as", i + 1));
        }
        p += 1;
        let name = lw
            .get(p)
            .ok_or_else(|| format!("line {}: missing Lykaon name", i + 1))?
            .to_string();
        p += 1;
        if !lw.get(p).is_some_and(|w| w.eq_ignore_ascii_case("takes")) {
            return Err(format!("line {}: expected takes", i + 1));
        }
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
        let mut inputs = Vec::new();
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
            .filter(|w| !w.eq_ignore_ascii_case("and"))
            .collect();
        let mut result_end = result_words.len();
        if result_words.len() >= 2
            && result_words[result_words.len() - 2].eq_ignore_ascii_case("can")
            && result_words[result_words.len() - 1].eq_ignore_ascii_case("fail")
        {
            result_end -= 2;
        }
        let (result, _) = parse_ckind(&result_words[..result_end])?;
        let can_fail = line.to_ascii_lowercase().contains("can fail");
        out.push(PyDecl {
            symbol,
            name,
            module: String::new(),
            inputs,
            result,
            can_fail,
        });
    }
    Ok(out)
}

thread_local! {
    static PY_FUNCS: std::cell::RefCell<Vec<PyDecl>> =
        std::cell::RefCell::new(Vec::new());
}

/// Register the Python functions declared by `.py.shelf` files before parsing.
pub fn register_py_functions(decls: &[PyDecl]) {
    PY_FUNCS.with(|m| {
        *m.borrow_mut() = decls.to_vec();
    });
}

/// The registered Python functions.
pub fn py_functions() -> Vec<PyDecl> {
    PY_FUNCS.with(|m| m.borrow().clone())
}

/// A value passed across the Python boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum PyArg {
    Whole(i64),
    Decimal(f64),
    YesNo(bool),
    Text(String),
}

/// A value returned across the Python boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum PyOut {
    Whole(i64),
    Decimal(f64),
    YesNo(bool),
    Text(String),
}

// ---- dynamically resolved Python C API -----------------------------------

type FnInit = unsafe extern "C" fn();
type FnIsInit = unsafe extern "C" fn() -> i32;
type FnImport = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type FnGetAttr = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type FnTupleNew = unsafe extern "C" fn(i64) -> *mut c_void;
type FnTupleSet = unsafe extern "C" fn(*mut c_void, i64, *mut c_void) -> i32;
type FnCall = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
type FnLongFrom = unsafe extern "C" fn(i64) -> *mut c_void;
type FnFloatFrom = unsafe extern "C" fn(f64) -> *mut c_void;
type FnUnicodeFrom = unsafe extern "C" fn(*const u8, i64) -> *mut c_void;
type FnBoolFrom = unsafe extern "C" fn(i32) -> *mut c_void;
type FnLongAs = unsafe extern "C" fn(*mut c_void) -> i64;
type FnFloatAs = unsafe extern "C" fn(*mut c_void) -> f64;
type FnUnicodeAsUTF8 = unsafe extern "C" fn(*mut c_void) -> *const u8;
type FnIsTrue = unsafe extern "C" fn(*mut c_void) -> i32;
type FnDecRef = unsafe extern "C" fn(*mut c_void);
type FnErrFetch = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void, *mut *mut c_void);
type FnErrClear = unsafe extern "C" fn();
type FnObjStr = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnGILEnsure = unsafe extern "C" fn() -> i32;
type FnGILRelease = unsafe extern "C" fn(i32);

#[derive(Clone, Copy)]
struct PyApi {
    init: FnInit,
    is_init: FnIsInit,
    import: FnImport,
    get_attr: FnGetAttr,
    tuple_new: FnTupleNew,
    tuple_set: FnTupleSet,
    call: FnCall,
    long_from: FnLongFrom,
    float_from: FnFloatFrom,
    unicode_from: FnUnicodeFrom,
    bool_from: FnBoolFrom,
    long_as: FnLongAs,
    float_as: FnFloatAs,
    unicode_utf8: FnUnicodeAsUTF8,
    is_true: FnIsTrue,
    decref: FnDecRef,
    err_fetch: FnErrFetch,
    err_clear: FnErrClear,
    obj_str: FnObjStr,
    gil_ensure: FnGILEnsure,
    gil_release: FnGILRelease,
}

impl PyApi {
    fn load() -> Result<PyApi, String> {
        let candidates: &[&str] = if cfg!(windows) {
            &[
                "python314.dll",
                "python313.dll",
                "python312.dll",
                "python311.dll",
                "python310.dll",
                "python39.dll",
                "python3.dll",
            ]
        } else {
            &[
                "libpython3.14.so.1.0",
                "libpython3.14.so",
                "libpython3.13.so.1.0",
                "libpython3.13.so",
                "libpython3.12.so.1.0",
                "libpython3.12.so",
                "libpython3.11.so.1.0",
                "libpython3.11.so",
                "libpython3.so",
            ]
        };
        let mut handle = None;
        let mut last = String::new();
        for c in candidates {
            match crate::cbridge::load_library(c) {
                Ok(h) => {
                    handle = Some(h);
                    break;
                }
                Err(e) => last = e,
            }
        }
        let handle = handle.ok_or_else(|| {
            format!(
                "the Python library cannot be loaded ({last}); install Python or add it to PATH"
            )
        })?;
        unsafe fn sym<T: Copy>(h: usize, name: &str) -> Result<T, String> {
            let p = crate::cbridge::resolve_symbol(h, name)?;
            Ok(std::mem::transmute_copy(&p))
        }
        unsafe {
            Ok(PyApi {
                init: sym(handle, "Py_Initialize")?,
                is_init: sym(handle, "Py_IsInitialized")?,
                import: sym(handle, "PyImport_ImportModule")?,
                get_attr: sym(handle, "PyObject_GetAttrString")?,
                tuple_new: sym(handle, "PyTuple_New")?,
                tuple_set: sym(handle, "PyTuple_SetItem")?,
                call: sym(handle, "PyObject_Call")?,
                long_from: sym(handle, "PyLong_FromLongLong")?,
                float_from: sym(handle, "PyFloat_FromDouble")?,
                unicode_from: sym(handle, "PyUnicode_FromStringAndSize")?,
                bool_from: sym(handle, "PyBool_FromLong")?,
                long_as: sym(handle, "PyLong_AsLongLong")?,
                float_as: sym(handle, "PyFloat_AsDouble")?,
                unicode_utf8: sym(handle, "PyUnicode_AsUTF8")?,
                is_true: sym(handle, "PyObject_IsTrue")?,
                decref: sym(handle, "Py_DecRef")?,
                err_fetch: sym(handle, "PyErr_Fetch")?,
                err_clear: sym(handle, "PyErr_Clear")?,
                obj_str: sym(handle, "PyObject_Str")?,
                gil_ensure: sym(handle, "PyGILState_Ensure")?,
                gil_release: sym(handle, "PyGILState_Release")?,
            })
        }
    }
}

thread_local! {
    static PY_API: std::cell::RefCell<Option<PyApi>> = const { std::cell::RefCell::new(None) };
}

fn python_api() -> Result<PyApi, String> {
    PY_API.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some(api) = *slot {
            return Ok(api);
        }
        let api = PyApi::load()?;
        *slot = Some(api);
        Ok(api)
    })
}

/// Call a declared Python function. Every call is an unsafe boundary; a Python
/// error is returned as a normal failure string.
pub fn py_call(decl: &PyDecl, args: &[PyArg]) -> Result<PyOut, String> {
    let api = python_api()?;
    unsafe {
        if (api.is_init)() == 0 {
            (api.init)();
        }
        let gil = (api.gil_ensure)();
        let r = py_call_inner(&api, decl, args);
        (api.gil_release)(gil);
        r
    }
}

unsafe fn py_call_inner(api: &PyApi, decl: &PyDecl, args: &[PyArg]) -> Result<PyOut, String> {
    if args.len() != decl.inputs.len() {
        return Err(format!(
            "{} expects {} inputs, but {} were given",
            decl.name,
            decl.inputs.len(),
            args.len()
        ));
    }
    let module_c = std::ffi::CString::new(decl.module.clone()).map_err(|_| "bad module name".to_string())?;
    let module = (api.import)(module_c.as_ptr());
    if module.is_null() {
        return Err(fetch_error(api, &format!("the Python module {} is not found", decl.module)));
    }
    let sym_c = std::ffi::CString::new(decl.symbol.clone()).map_err(|_| "bad symbol".to_string())?;
    let func = (api.get_attr)(module, sym_c.as_ptr());
    if func.is_null() {
        (api.decref)(module);
        return Err(fetch_error(
            api,
            &format!("the Python module {} has no {}", decl.module, decl.symbol),
        ));
    }
    let tuple = (api.tuple_new)(args.len() as i64);
    if tuple.is_null() {
        (api.decref)(func);
        (api.decref)(module);
        return Err("python could not build the argument tuple".to_string());
    }
    for (i, a) in args.iter().enumerate() {
        let obj = arg_to_py(api, a)?;
        // PyTuple_SetItem steals the reference.
        if (api.tuple_set)(tuple, i as i64, obj) != 0 {
            (api.decref)(tuple);
            (api.decref)(func);
            (api.decref)(module);
            return Err(fetch_error(api, "python could not set an argument"));
        }
    }
    let result = (api.call)(func, tuple, std::ptr::null_mut());
    (api.decref)(func);
    (api.decref)(tuple);
    (api.decref)(module);
    if result.is_null() {
        return Err(fetch_error(api, "the Python call failed"));
    }
    let out = py_to_out(api, decl.result.clone(), result);
    (api.decref)(result);
    out
}

unsafe fn arg_to_py(api: &PyApi, a: &PyArg) -> Result<*mut c_void, String> {
    Ok(match a {
        PyArg::Whole(n) => (api.long_from)(*n),
        PyArg::Decimal(x) => (api.float_from)(*x),
        PyArg::YesNo(b) => (api.bool_from)(*b as i32),
        PyArg::Text(s) => {
            let bytes = s.as_bytes();
            (api.unicode_from)(bytes.as_ptr(), bytes.len() as i64)
        }
    })
}

unsafe fn py_to_out(api: &PyApi, kind: CKind, obj: *mut c_void) -> Result<PyOut, String> {
    use CKind::*;
    Ok(match kind {
        Whole => PyOut::Whole((api.long_as)(obj)),
        Decimal => PyOut::Decimal((api.float_as)(obj)),
        YesNo => PyOut::YesNo((api.is_true)(obj) != 0),
        Letter | Text | Bytes => {
            let p = (api.unicode_utf8)(obj);
            if p.is_null() {
                return Err(fetch_error(api, "the Python result is not text"));
            }
            PyOut::Text(CStr::from_ptr(p as *const c_char).to_string_lossy().to_string())
        }
        Record(name) => {
            return Err(format!(
                "the Python result of record {name} is not wired; use a scalar kind"
            ))
        }
    })
}

/// Fetch and clear the current Python error, returning its message.
fn fetch_error(api: &PyApi, fallback: &str) -> String {
    unsafe {
        let mut t: *mut c_void = std::ptr::null_mut();
        let mut v: *mut c_void = std::ptr::null_mut();
        let mut tb: *mut c_void = std::ptr::null_mut();
        (api.err_fetch)(&mut t, &mut v, &mut tb);
        if v.is_null() {
            (api.err_clear)();
            return fallback.to_string();
        }
        let s = (api.obj_str)(v);
        let msg = if s.is_null() {
            fallback.to_string()
        } else {
            let p = (api.unicode_utf8)(s);
            let m = if p.is_null() {
                fallback.to_string()
            } else {
                CStr::from_ptr(p as *const c_char).to_string_lossy().to_string()
            };
            (api.decref)(s);
            m
        };
        (api.decref)(v);
        if !t.is_null() {
            (api.decref)(t);
        }
        if !tb.is_null() {
            (api.decref)(tb);
        }
        (api.err_clear)();
        msg
    }
}
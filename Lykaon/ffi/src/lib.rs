//! LYKAON stable C ABI (v1 draft).
//!
//! A small, additive C surface for hosts. The full event/command protocol
//! lives in `docs/C_ABI.md` and `Prompt/SCREENS_ELEMENT_INTEGRATION.md`;
//! this crate only evaluates source and returns its output.

use std::os::raw::c_char;

/// Version of the C ABI (currently 1).
#[unsafe(no_mangle)]
pub extern "C" fn lykaon_abi_version() -> u32 {
    1
}

/// Evaluate Lykaon source into a caller-owned buffer.
///
/// `out` must have room for `cap` bytes. At most `cap - 1` bytes are written,
/// the result is null-terminated, and the needed length is stored in
/// `*out_len` (when non-null). Returns the Lykaon exit code (0 = ok,
/// 1 = program error, 2 = ABI usage error).
#[unsafe(no_mangle)]
pub extern "C" fn lykaon_eval(
    src: *const c_char,
    out: *mut c_char,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if src.is_null() || out.is_null() || cap == 0 {
        return 2;
    }
    let source = unsafe { std::ffi::CStr::from_ptr(src) }.to_string_lossy();
    let (text, code) = lykaon::run_source(&source, "ffi.lykaon", "");
    let bytes = text.as_bytes();
    let n = bytes.len().min(cap - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
        *out.add(n) = 0;
        if !out_len.is_null() {
            *out_len = n;
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(lykaon_abi_version(), 1);
    }

    #[test]
    fn eval_returns_output() {
        let src = CString::new("Say hello.").unwrap();
        let mut out = [0 as c_char; 256];
        let mut n = 0usize;
        let code = lykaon_eval(src.as_ptr(), out.as_mut_ptr(), out.len(), &mut n);
        assert_eq!(code, 0);
        let text = unsafe { std::ffi::CStr::from_ptr(out.as_ptr()) }
            .to_string_lossy()
            .to_string();
        assert_eq!(text.trim(), "hello");
        assert_eq!(n, 6);
    }
}
//! lykaon-playground (M5d-preview extra): the Lykaon interpreter compiled to
//! WebAssembly for the browser playground. Additive and off-plan: it changes
//! no language rule and is not part of the public repo until it works.
//!
//! ABI (no wasm-bindgen; raw exports):
//!   lykaon_alloc(len)  -> input pointer (fixed static region, at most 1 MiB)
//!   lykaon_run(ptr,len) -> (out_ptr << 32) | out_len; output text lands in a
//!                          256 KiB static buffer (truncated beyond that)
//! The caller writes the source into memory at the returned pointer, calls
//! lykaon_run, and reads the output bytes from memory.

use std::cell::UnsafeCell;

const INPUT_CAP: usize = 1 << 20;
const OUTPUT_CAP: usize = 1 << 18;

struct Buf<const N: usize>(UnsafeCell<[u8; N]>);
unsafe impl<const N: usize> Sync for Buf<N> {}

static INPUT: Buf<INPUT_CAP> = Buf(UnsafeCell::new([0; INPUT_CAP]));
static OUTPUT: Buf<OUTPUT_CAP> = Buf(UnsafeCell::new([0; OUTPUT_CAP]));

/// Base of the writable input region (also the exported memory base).
#[unsafe(no_mangle)]
pub extern "C" fn lykaon_alloc(len: u32) -> u32 {
    let _ = len;
    INPUT.0.get() as u32
}

/// Run one program. `ptr`/`len` address UTF-8 source in memory. Returns
/// (out_ptr << 32) | out_len; out_ptr is an absolute memory offset.
#[unsafe(no_mangle)]
pub extern "C" fn lykaon_run(ptr: u32, len: u32) -> u64 {
    let text = (|| {
        if len as usize > INPUT_CAP {
            return "Error: program is too large for the playground (limit 1 MiB).\n".to_string();
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
        let source = String::from_utf8_lossy(bytes).to_string();
        let (out, _code) = lykaon::run_source(&source, "playground.lykaon", "");
        out
    })();
    let n = text.len().min(OUTPUT_CAP);
    unsafe {
        let out = OUTPUT.0.get() as *mut u8;
        std::ptr::copy_nonoverlapping(text.as_bytes().as_ptr(), out, n);
    }
    ((OUTPUT.0.get() as u64) << 32) | n as u64
}
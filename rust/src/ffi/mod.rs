//! Hand-written Windows API bindings: only what InputSwitch uses, so no binding crates are needed.
//! Grouped by area; everything is re-exported here, so `use crate::ffi::*` brings in the lot.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

mod window;
mod controls;
mod gdi;
mod input;
mod shell;
mod display;
mod system;
mod theming;

pub use window::*;
pub use controls::*;
pub use gdi::*;
pub use input::*;
pub use shell::*;
pub use display::*;
pub use system::*;
pub use theming::*;

pub type HWND = isize;
pub type HANDLE = isize;
pub type HINSTANCE = isize;
pub type HMENU = isize;
pub type HICON = isize;
pub type HFONT = isize;
pub type HMONITOR = isize;
pub type HKEY = isize;
pub type HDC = isize;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type BOOL = i32;

/// All-zero value of a plain C struct.
pub fn zeroed<T>() -> T {
    unsafe { core::mem::zeroed() }
}

/// NUL-terminated UTF-16 copy of a string.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// String from a (possibly NUL-terminated) UTF-16 buffer.
pub fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

/// Copies a string into a fixed UTF-16 buffer, truncating and NUL-terminating.
pub fn copy_wide(dst: &mut [u16], s: &str) {
    let mut i = 0;
    for c in s.encode_utf16() {
        if i + 1 >= dst.len() {
            break;
        }
        dst[i] = c;
        i += 1;
    }
    dst[i] = 0;
}

pub fn loword(x: usize) -> u16 {
    (x & 0xFFFF) as u16
}

pub fn hiword(x: usize) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
}

pub fn hinstance() -> HINSTANCE {
    unsafe { GetModuleHandleW(core::ptr::null()) }
}

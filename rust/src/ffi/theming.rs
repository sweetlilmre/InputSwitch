//! Visual styles (uxtheme) and the window frame (DWM).

use super::*;

pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmSetWindowAttribute(h: HWND, attr: u32, value: *const core::ffi::c_void, size: u32) -> i32;
}

#[link(name = "uxtheme")]
extern "system" {
    pub fn SetWindowTheme(h: HWND, app: *const u16, idlist: *const u16) -> i32;
}

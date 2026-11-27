//! Painting: device contexts, brushes, pens, fonts, text, and invalidation.

use super::*;

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

pub const DT_CENTER: u32 = 0x0001;
pub const DT_VCENTER: u32 = 0x0004;
pub const DT_SINGLELINE: u32 = 0x0020;
pub const DT_NOPREFIX: u32 = 0x0800;
pub const DT_END_ELLIPSIS: u32 = 0x8000;
pub const DT_BOTTOM: u32 = 0x0008;
pub const DT_WORDBREAK: u32 = 0x0010;
pub const DT_CALCRECT: u32 = 0x0400;
pub const TRANSPARENT: i32 = 1;
pub const PS_SOLID: i32 = 0;
pub const NULL_BRUSH: i32 = 5;
pub const NULL_PEN: i32 = 8;
pub const RDW_INVALIDATE: u32 = 0x0001;
pub const RDW_ERASE: u32 = 0x0004;
pub const RDW_ALLCHILDREN: u32 = 0x0080;
pub const RDW_FRAME: u32 = 0x0400;
pub const SRCCOPY: u32 = 0x00CC_0020;

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateFontW(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32,
        charset: u32, out_prec: u32, clip_prec: u32, quality: u32, pitch: u32, face: *const u16) -> HFONT;
    pub fn DeleteObject(o: isize) -> BOOL;
    pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn CreateSolidBrush(color: u32) -> isize;
    pub fn CreatePen(style: i32, width: i32, color: u32) -> isize;
    pub fn SelectObject(hdc: HDC, obj: isize) -> isize;
    pub fn RoundRect(hdc: HDC, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> BOOL;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn GetStockObject(i: i32) -> isize;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateCompatibleBitmap(hdc: HDC, w: i32, h: i32) -> isize;
    pub fn BitBlt(dst: HDC, x: i32, y: i32, w: i32, h: i32, src: HDC, sx: i32, sy: i32, rop: u32) -> BOOL;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
}

#[link(name = "user32")]
extern "system" {
    pub fn InvalidateRect(h: HWND, r: *const RECT, erase: BOOL) -> BOOL;
    pub fn FillRect(hdc: HDC, r: *const RECT, brush: isize) -> i32;
    pub fn DrawTextW(hdc: HDC, text: *const u16, len: i32, r: *mut RECT, format: u32) -> i32;
    pub fn RedrawWindow(h: HWND, r: *const RECT, rgn: isize, flags: u32) -> BOOL;
    pub fn GetDC(h: HWND) -> HDC;
    pub fn ReleaseDC(h: HWND, hdc: HDC) -> i32;
    pub fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> BOOL;
}

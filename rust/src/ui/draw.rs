//! Painting primitives: solid and rounded fills, text, text measuring, flicker-free painting,
//! and a few `RECT` helpers. Colours are COLORREFs (see [`super::theme::rgb`]).

use super::theme::brush;
use crate::ffi::*;

pub fn fill(hdc: HDC, r: RECT, color: u32) {
    unsafe { FillRect(hdc, &r, brush(color)) };
}

/// Fills a rounded rectangle, optionally with a 1-pixel border.
pub fn round_rect(hdc: HDC, r: RECT, radius: i32, fill: u32, border: Option<u32>) {
    rounded(hdc, r, radius, Some(fill), border);
}

/// A rounded rectangle's 1-pixel outline (a focus ring).
pub fn outline(hdc: HDC, r: RECT, radius: i32, color: u32) {
    rounded(hdc, r, radius, None, Some(color));
}

fn rounded(hdc: HDC, r: RECT, radius: i32, fill: Option<u32>, border: Option<u32>) {
    unsafe {
        let pen = match border {
            Some(c) => CreatePen(PS_SOLID, 1, c),
            None => GetStockObject(NULL_PEN),
        };
        let old_pen = SelectObject(hdc, pen);
        let fill_brush = fill.map_or_else(|| GetStockObject(NULL_BRUSH), brush);
        let old_brush = SelectObject(hdc, fill_brush);
        // RoundRect leaves out the right and bottom edge when there's no pen: add it back.
        let extra = if border.is_none() { 1 } else { 0 };
        RoundRect(hdc, r.left, r.top, r.right + extra, r.bottom + extra, radius * 2, radius * 2);
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        if border.is_some() {
            DeleteObject(pen);
        }
    }
}

/// Draws text in a rectangle (`DT_*` format flags; prefixes like `&` are never interpreted).
pub fn text(hdc: HDC, s: &str, mut r: RECT, font: HFONT, color: u32, format: u32) {
    if s.is_empty() {
        return; // an empty Vec's pointer isn't valid to hand to DrawTextW
    }
    let w: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        let old = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, color);
        DrawTextW(hdc, w.as_ptr(), w.len() as i32, &mut r, format | DT_NOPREFIX);
        SelectObject(hdc, old);
    }
}

/// A single glyph (e.g. a Segoe Fluent Icons code point), centred vertically in `r`.
pub fn glyph(hdc: HDC, c: char, r: RECT, font: HFONT, color: u32, format: u32) {
    text(hdc, c.encode_utf8(&mut [0; 4]), r, font, color, format | DT_SINGLELINE);
}

/// The size `text` needs: on one line, or wrapped to `wrap_width`.
fn measure(s: &str, font: HFONT, wrap_width: Option<i32>) -> (i32, i32) {
    if s.is_empty() {
        return (0, 0);
    }
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut r = RECT { left: 0, top: 0, right: wrap_width.unwrap_or(0), bottom: 0 };
    let flags = DT_CALCRECT | DT_NOPREFIX | if wrap_width.is_some() { DT_WORDBREAK } else { DT_SINGLELINE };
    unsafe {
        let dc = GetDC(0);
        let old = SelectObject(dc, font);
        DrawTextW(dc, w.as_ptr(), w.len() as i32, &mut r, flags);
        SelectObject(dc, old);
        ReleaseDC(0, dc);
    }
    (r.right - r.left, r.bottom - r.top)
}

/// Width of `s` on one line.
pub fn text_width(s: &str, font: HFONT) -> i32 {
    measure(s, font, None).0
}

/// Height of `s` wrapped to `width`.
pub fn text_height(s: &str, font: HFONT, width: i32) -> i32 {
    measure(s, font, Some(width)).1
}

/// Handles WM_PAINT by drawing off screen and copying the result, so nothing flickers.
/// `paint` gets the memory DC and the client rectangle.
pub fn paint_buffered(h: HWND, paint: impl FnOnce(HDC, RECT)) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(h, &mut ps);
        let rc = client_rect(h);
        let mem = CreateCompatibleDC(hdc);
        let bitmap = CreateCompatibleBitmap(hdc, rc.right.max(1), rc.bottom.max(1));
        let old = SelectObject(mem, bitmap);
        paint(mem, rc);
        BitBlt(hdc, 0, 0, rc.right, rc.bottom, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old);
        DeleteObject(bitmap);
        DeleteDC(mem);
        EndPaint(h, &ps);
    }
}

pub fn client_rect(h: HWND) -> RECT {
    let mut rc = RECT::default();
    unsafe { GetClientRect(h, &mut rc) };
    rc
}

/// A rectangle from position and size.
pub const fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT { left: x, top: y, right: x + w, bottom: y + h }
}

pub trait RectExt {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    /// Shrunk by `dx` on the left and right and `dy` on the top and bottom (negative grows).
    fn inset(&self, dx: i32, dy: i32) -> RECT;
    /// Moved by `dx`, `dy`.
    fn offset(&self, dx: i32, dy: i32) -> RECT;
}

impl RectExt for RECT {
    fn width(&self) -> i32 {
        self.right - self.left
    }
    fn height(&self) -> i32 {
        self.bottom - self.top
    }
    fn inset(&self, dx: i32, dy: i32) -> RECT {
        RECT { left: self.left + dx, top: self.top + dy, right: self.right - dx, bottom: self.bottom - dy }
    }
    fn offset(&self, dx: i32, dy: i32) -> RECT {
        RECT { left: self.left + dx, top: self.top + dy, right: self.right + dx, bottom: self.bottom + dy }
    }
}

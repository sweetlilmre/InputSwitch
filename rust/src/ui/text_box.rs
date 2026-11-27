//! Text boxes like WinUI's TextBox: a borderless edit box inside a painted frame (a faint border
//! and a bottom line; darker, with an accent line, while focused), its text centred vertically,
//! and a "×" button at the end that clears it, shown while it has focus and isn't empty.
//!
//! The parent paints the frame ([`TextBox::paint`]) and passes the edit box's notifications to
//! [`TextBox::on_command`], so the "×" and the frame follow the focus.

use super::draw::{self, RectExt};
use super::{theme, Fonts};
use crate::ffi::*;
use std::cell::RefCell;
use std::collections::HashMap;

const CLEAR_CLASS: &str = "ui.TextBoxClear";
const CLEAR_W: i32 = 32;
const PAD_X: i32 = 10;

#[derive(Clone, Copy, PartialEq)]
pub struct TextBox {
    pub edit: HWND,
    clear: HWND,
}

thread_local! {
    /// Each "×" button's edit box, and the icon font it's drawn with.
    static CLEAR_TARGETS: RefCell<HashMap<HWND, (HWND, HFONT)>> = RefCell::new(HashMap::new());
}

impl TextBox {
    /// A text box with control id `id`, in `font`.
    pub fn create(parent: HWND, id: i32, font: HFONT, fonts: &Fonts) -> TextBox {
        let edit = super::edit(parent, id, font);
        super::register_class(CLEAR_CLASS, clear_proc, false, 0);
        let clear = unsafe {
            CreateWindowExW(0, wide(CLEAR_CLASS).as_ptr(), core::ptr::null(), WS_CHILD, 0, 0, 0, 0, parent, 0, hinstance(), core::ptr::null())
        };
        CLEAR_TARGETS.with(|t| t.borrow_mut().insert(clear, (edit, fonts.icon_12)));
        TextBox { edit, clear }
    }

    /// Puts the box in `frame` (where the parent paints its frame).
    pub fn place(&self, frame: RECT, dpi: u32) {
        let s = |v| super::scale(v, dpi);
        let font = unsafe { SendMessageW(self.edit, WM_GETFONT, 0, 0) } as HFONT;
        let line = draw::text_height("Ag", font, 1000);
        let clear = RECT { left: frame.right - s(CLEAR_W) - s(4), ..frame.inset(0, s(4)) };
        let text = RECT { left: frame.left + s(PAD_X), right: clear.left, top: frame.top + (frame.height() - line) / 2, bottom: 0 };
        super::place(self.edit, text.left, text.top, text.width(), line);
        super::place_rect(self.clear, RECT { right: frame.right - s(4), ..clear });
    }

    /// The edit box's notification: shows or hides the "×" (focused and not empty).
    pub fn on_command(&self, code: u16) {
        if matches!(code, EN_SETFOCUS | EN_KILLFOCUS | EN_CHANGE) {
            let show = unsafe { GetFocus() } == self.edit && unsafe { GetWindowTextLengthW(self.edit) } > 0;
            unsafe { ShowWindow(self.clear, if show { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
        }
    }

    /// Paints the frame behind the box.
    pub fn paint(&self, hdc: HDC, frame: RECT, dpi: u32) {
        paint_frame(hdc, frame, dpi, unsafe { GetFocus() } == self.edit);
    }

    pub fn set_font(&self, font: HFONT, fonts: &Fonts) {
        super::set_font(self.edit, font);
        CLEAR_TARGETS.with(|t| {
            if let Some(entry) = t.borrow_mut().get_mut(&self.clear) {
                entry.1 = fonts.icon_12;
            }
        });
    }

    pub fn destroy(&self) {
        unsafe {
            DestroyWindow(self.edit);
            DestroyWindow(self.clear);
        }
    }
}

/// Paints a text box's frame: the field, a faint border and the bottom line, which turns
/// accent-coloured and thicker while it has focus (and the field darker).
pub fn paint_frame(hdc: HDC, frame: RECT, dpi: u32, focused: bool) {
    let p = theme::palette();
    let s = |v| super::scale(v, dpi);
    draw::round_rect(hdc, frame, s(4), field_color(focused), Some(p.field_stroke));
    let line = if focused { s(2) } else { 1 };
    let underline = RECT { left: frame.left + s(3), top: frame.bottom - line, right: frame.right - s(3), bottom: frame.bottom };
    draw::fill(hdc, underline, if focused { p.accent } else { p.field_underline });
}

/// A text box's background.
pub fn field_color(focused: bool) -> u32 {
    let p = theme::palette();
    if focused { p.field_focused } else { p.field }
}

/// The colours for an edit box (WM_CTLCOLOREDIT): its field, darker while focused.
pub fn edit_colors(hdc: HDC, edit: HWND) -> LRESULT {
    let focused = unsafe { GetFocus() } == edit;
    theme::set_control_colors(hdc, theme::palette().text, field_color(focused))
}

unsafe extern "system" fn clear_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_PAINT => {
            draw::paint_buffered(h, |hdc, rc| {
                let p = theme::palette();
                draw::fill(hdc, rc, p.field_focused);
                if let Some((_, font)) = CLEAR_TARGETS.with(|t| t.borrow().get(&h).copied()) {
                    draw::glyph(hdc, super::widgets::ICON_CLEAR, rc, font, p.text2, DT_CENTER | DT_VCENTER);
                }
            });
            return 0;
        }
        // Clears the box without taking the focus from it (which would hide this button).
        WM_MOUSEACTIVATE => return MA_NOACTIVATE,
        WM_LBUTTONDOWN => {
            if let Some((edit, _)) = CLEAR_TARGETS.with(|t| t.borrow().get(&h).copied()) {
                super::set_text(edit, "");
                SetFocus(edit);
            }
            return 0;
        }
        WM_NCDESTROY => {
            CLEAR_TARGETS.with(|t| t.borrow_mut().remove(&h));
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

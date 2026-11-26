//! Owner-drawn buttons and checkboxes in a WinUI-like style: rounded, with hover and pressed
//! states, an optional Segoe Fluent Icons glyph, and an accent style for the main action.
//! Checkboxes are drawn too, so their labels stay readable in dark mode (themed Win32 checkboxes
//! keep black text).
//!
//! Parents must forward WM_DRAWITEM to [`draw`] and call [`clicked`] on BN_CLICKED.

use crate::ffi::*;
use crate::theme::{self, Surface};
use std::cell::RefCell;
use std::collections::HashMap;

pub const ICON_ADD: char = '\u{E710}';
pub const ICON_DELETE: char = '\u{E74D}';
pub const ICON_REFRESH: char = '\u{E72C}';
pub const ICON_EDIT: char = '\u{E70F}';
pub const ICON_PLAY: char = '\u{E768}';
pub const ICON_RENAME: char = '\u{E8AC}';
pub const ICON_UP: char = '\u{E70E}';
pub const ICON_DOWN: char = '\u{E70D}';
const ICON_CHECK: char = '\u{E73E}';

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Standard,
    Accent,
    Check,
    /// A WinUI-style on/off toggle switch.
    Switch,
}

struct Info {
    kind: Kind,
    icon: Option<char>,
    checked: bool,
    hot: bool,
    text_font: HFONT,
    icon_font: HFONT,
}

thread_local! {
    static INFO: RefCell<HashMap<HWND, Info>> = RefCell::new(HashMap::new());
}

fn create(parent: HWND, text: &str, id: i32, kind: Kind, icon: Option<char>, font: HFONT, icon_font: HFONT, surface: Surface) -> HWND {
    let h = crate::ui::child(parent, "BUTTON", text, BS_OWNERDRAW | WS_TABSTOP, 0, id, font);
    theme::set_surface(h, surface, false);
    INFO.with(|i| i.borrow_mut().insert(h, Info { kind, icon, checked: false, hot: false, text_font: font, icon_font }));
    unsafe { SetWindowSubclass(h, proc_, 1, 0) };
    h
}

pub fn button(parent: HWND, text: &str, id: i32, icon: Option<char>, fonts: &crate::ui::Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Standard, icon, fonts.normal, fonts.icon, surface)
}

pub fn accent_button(parent: HWND, text: &str, id: i32, icon: Option<char>, fonts: &crate::ui::Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Accent, icon, fonts.normal, fonts.icon, surface)
}

pub fn checkbox(parent: HWND, text: &str, id: i32, fonts: &crate::ui::Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Check, None, fonts.normal, fonts.icon, surface)
}

pub fn switch(parent: HWND, text: &str, id: i32, fonts: &crate::ui::Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Switch, None, fonts.normal, fonts.icon, surface)
}

/// Updates the fonts after a DPI change.
pub fn set_fonts(h: HWND, fonts: &crate::ui::Fonts) {
    INFO.with(|i| {
        if let Some(info) = i.borrow_mut().get_mut(&h) {
            info.text_font = fonts.normal;
            info.icon_font = fonts.icon;
        }
    });
}

pub fn checked(h: HWND) -> bool {
    INFO.with(|i| i.borrow().get(&h).map(|x| x.checked)).unwrap_or(false)
}

pub fn set_checked(h: HWND, on: bool) {
    INFO.with(|i| {
        if let Some(info) = i.borrow_mut().get_mut(&h) {
            info.checked = on;
        }
    });
    unsafe { RedrawWindow(h, core::ptr::null(), 0, RDW_INVALIDATE) };
}

/// Call on BN_CLICKED: a checkbox or switch flips its state.
pub fn clicked(h: HWND) {
    let is_check = INFO.with(|i| i.borrow().get(&h).map(|x| matches!(x.kind, Kind::Check | Kind::Switch))).unwrap_or(false);
    if is_check {
        set_checked(h, !checked(h));
    }
}

unsafe extern "system" fn proc_(h: HWND, m: u32, w: WPARAM, l: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match m {
        WM_MOUSEMOVE => {
            let was_hot = INFO.with(|i| i.borrow_mut().get_mut(&h).map(|x| std::mem::replace(&mut x.hot, true))).unwrap_or(true);
            if !was_hot {
                let mut t = TRACKMOUSEEVENT { cbSize: core::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: h, dwHoverTime: 0 };
                TrackMouseEvent(&mut t);
                RedrawWindow(h, core::ptr::null(), 0, RDW_INVALIDATE);
            }
        }
        WM_MOUSELEAVE => {
            INFO.with(|i| if let Some(x) = i.borrow_mut().get_mut(&h) { x.hot = false });
            RedrawWindow(h, core::ptr::null(), 0, RDW_INVALIDATE);
        }
        WM_ERASEBKGND => return 1, // drawn entirely in WM_DRAWITEM; avoids flicker
        WM_NCDESTROY => {
            INFO.with(|i| i.borrow_mut().remove(&h));
            theme::forget(h);
            RemoveWindowSubclass(h, proc_, 1);
        }
        _ => {}
    }
    DefSubclassProc(h, m, w, l)
}

/// Draws an owner-drawn button or checkbox. Returns false if it isn't one of ours.
pub fn draw(dis: &DRAWITEMSTRUCT) -> bool {
    if dis.CtlType != ODT_BUTTON {
        return false;
    }
    let h = dis.hwndItem;
    let Some((kind, icon, checked, hot, font, icon_font)) =
        INFO.with(|i| i.borrow().get(&h).map(|x| (x.kind, x.icon, x.checked, x.hot, x.text_font, x.icon_font)))
    else {
        return false;
    };
    let p = theme::palette();
    let hdc = dis.hDC;
    let r = dis.rcItem;
    let disabled = dis.itemState & ODS_DISABLED != 0;
    let pressed = dis.itemState & ODS_SELECTED != 0;
    let focused = dis.itemState & ODS_FOCUS != 0 && dis.itemState & ODS_NOFOCUSRECT == 0;
    let dpi = unsafe { GetDpiForWindow(h) }.max(96);
    let s = |v: i32| crate::ui::scale(v, dpi);
    let text = crate::ui::get_text(h);

    theme::fill(hdc, r, theme::color_of(theme::surface_of(h)));

    if kind == Kind::Switch {
        // Track: a pill, accent-filled when on. Knob: a circle at the left (off) or right (on).
        let (tw, th) = (s(40), s(20));
        let top = r.top + (r.bottom - r.top - th) / 2;
        let track = RECT { left: r.left + s(1), top, right: r.left + s(1) + tw, bottom: top + th };
        let (track_fill, track_border, knob) = if checked {
            (if disabled { p.text_disabled } else if hot { p.accent_hot } else { p.accent }, None, p.on_accent)
        } else {
            (if hot && !disabled { p.control_hot } else { p.control }, Some(if disabled { p.text_disabled } else { p.text2 }), if disabled { p.text_disabled } else { p.text2 })
        };
        theme::round_rect(hdc, track, th / 2, track_fill, track_border);
        let k = s(if hot && !disabled { 14 } else { 12 });
        let kx = if checked { track.right - s(4) - k } else { track.left + s(4) };
        let ky = top + (th - k) / 2;
        theme::round_rect(hdc, RECT { left: kx, top: ky, right: kx + k, bottom: ky + k }, k / 2, knob, None);
        let label = RECT { left: track.right + s(10), top: r.top, right: r.right, bottom: r.bottom };
        theme::text(hdc, &text, label, font, if disabled { p.text_disabled } else { p.text }, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        if focused {
            theme::round_rect(hdc, RECT { left: track.left - s(2), top: track.top - s(2), right: track.right + s(2), bottom: track.bottom + s(2) }, th / 2 + s(2), theme::NO_FILL, Some(p.text));
        }
        return true;
    }

    if kind == Kind::Check {
        let size = s(18);
        let top = r.top + (r.bottom - r.top - size) / 2;
        let bx = RECT { left: r.left + s(1), top, right: r.left + s(1) + size, bottom: top + size };
        if checked {
            let fill = if disabled { p.text_disabled } else if hot { p.accent_hot } else { p.accent };
            theme::round_rect(hdc, bx, s(4), fill, None);
            theme::text(hdc, &ICON_CHECK.to_string(), bx, icon_font, p.on_accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        } else {
            let fill = if hot && !disabled { p.control_hot } else { p.control };
            theme::round_rect(hdc, bx, s(4), fill, Some(if disabled { p.text_disabled } else { p.text2 }));
        }
        let label = RECT { left: bx.right + s(8), top: r.top, right: r.right, bottom: r.bottom };
        theme::text(hdc, &text, label, font, if disabled { p.text_disabled } else { p.text }, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        if focused {
            theme::round_rect(hdc, RECT { left: bx.left - s(1), top: bx.top - s(1), right: bx.right + s(1), bottom: bx.bottom + s(1) }, s(5), theme::NO_FILL, Some(p.text));
        }
        return true;
    }

    let (fill, fg, border) = match kind {
        Kind::Accent if !disabled => (if pressed { p.accent } else if hot { p.accent_hot } else { p.accent }, p.on_accent, None),
        Kind::Accent => (p.control, p.text_disabled, Some(p.control_stroke)),
        _ => (
            if disabled { p.control } else if pressed { p.control_pressed } else if hot { p.control_hot } else { p.control },
            if disabled { p.text_disabled } else { p.text },
            Some(p.control_stroke),
        ),
    };
    theme::round_rect(hdc, r, s(4), fill, border);

    // Icon and text, centred together.
    let gap = s(8);
    let text_w = measure(hdc, &text, font);
    let icon_w = if icon.is_some() { s(16) } else { 0 };
    let total = text_w + if icon.is_some() { icon_w + gap } else { 0 };
    let mut x = r.left + ((r.right - r.left) - total).max(0) / 2;
    if let Some(c) = icon {
        theme::text(hdc, &c.to_string(), RECT { left: x, top: r.top, right: x + icon_w, bottom: r.bottom }, icon_font, fg, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        x += icon_w + gap;
    }
    theme::text(hdc, &text, RECT { left: x, top: r.top, right: r.right - s(4), bottom: r.bottom }, font, fg, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);

    if focused {
        let f = RECT { left: r.left + s(2), top: r.top + s(2), right: r.right - s(2), bottom: r.bottom - s(2) };
        theme::round_rect(hdc, f, s(3), theme::NO_FILL, Some(fg));
    }
    true
}

fn measure(hdc: HDC, s: &str, font: HFONT) -> i32 {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut r = RECT { left: 0, top: 0, right: 10_000, bottom: 1000 };
    unsafe {
        let old = SelectObject(hdc, font);
        DrawTextW(hdc, w.as_ptr(), w.len() as i32, &mut r, 0x0400 /* DT_CALCRECT */ | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(hdc, old);
    }
    r.right - r.left
}

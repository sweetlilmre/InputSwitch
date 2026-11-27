//! Owner-drawn buttons, checkboxes and toggle switches in a WinUI-like style: rounded, with hover
//! and pressed states, an optional Segoe Fluent Icons glyph, and an accent style for the main
//! action. Checkboxes are drawn too, so their labels stay readable in dark mode (themed Win32
//! checkboxes keep black text).
//!
//! Parents must forward WM_DRAWITEM to [`draw`] and call [`clicked`] on BN_CLICKED.

use super::draw;
use super::theme::{self, Surface};
use super::Fonts;
use crate::ffi::*;
use std::cell::RefCell;
use std::collections::HashMap;

pub const ICON_ADD: char = '\u{E710}';
pub const ICON_DELETE: char = '\u{E74D}';
pub const ICON_REFRESH: char = '\u{E72C}';
pub const ICON_EDIT: char = '\u{E70F}';
pub const ICON_PLAY: char = '\u{E768}';
pub const ICON_UP: char = '\u{E70E}';
pub const ICON_DOWN: char = '\u{E70D}';
pub const ICON_MONITOR: char = '\u{E7F4}';
pub const ICON_TICK: char = '\u{E73E}';
pub const ICON_WARNING: char = '\u{E7BA}';
pub const ICON_INFO: char = '\u{E946}';
pub const ICON_KEYBOARD: char = '\u{E765}';

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Standard,
    Accent,
    Check,
    /// A WinUI-style on/off toggle switch.
    Switch,
    /// A drop-down field: the choice on the left, a chevron on the right. The parent shows the
    /// choices when it's clicked (see [`choose`]).
    DropDown,
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

fn create(parent: HWND, text: &str, id: i32, kind: Kind, icon: Option<char>, fonts: &Fonts, surface: Surface) -> HWND {
    let h = super::child(parent, "BUTTON", text, BS_OWNERDRAW | WS_TABSTOP, 0, id, fonts.normal);
    theme::set_surface(h, surface, false);
    INFO.with(|i| i.borrow_mut().insert(h, Info { kind, icon, checked: false, hot: false, text_font: fonts.normal, icon_font: fonts.icon }));
    unsafe { SetWindowSubclass(h, proc_, 1, 0) };
    h
}

pub fn button(parent: HWND, text: &str, id: i32, icon: Option<char>, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Standard, icon, fonts, surface)
}

/// A drop-down field showing `text` (the current choice).
pub fn drop_down(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::DropDown, None, fonts, surface)
}

/// Shows choices in a menu under drop-down h, the current one ticked. Returns the one picked.
pub fn choose(h: HWND, choices: &[String], current: Option<usize>) -> Option<usize> {
    let mut r = RECT::default();
    unsafe {
        GetWindowRect(h, &mut r);
        let menu = CreatePopupMenu();
        for (i, c) in choices.iter().enumerate() {
            let checked = if Some(i) == current { MF_CHECKED } else { 0 };
            AppendMenuW(menu, MF_STRING | checked, i + 1, wide(c).as_ptr());
        }
        let picked = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY, r.left, r.bottom, 0, GetParent(h), core::ptr::null());
        DestroyMenu(menu);
        (picked > 0).then(|| picked as usize - 1)
    }
}

pub fn accent_button(parent: HWND, text: &str, id: i32, icon: Option<char>, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Accent, icon, fonts, surface)
}

pub fn checkbox(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Check, None, fonts, surface)
}

pub fn switch(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Switch, None, fonts, surface)
}

/// Paints the frame behind a borderless edit box, as a Fluent text box: a faint border and a
/// brighter bottom line, which turns accent-coloured and thicker while the box has focus. The edit
/// box itself sits inside, with the field colour (see [`theme::ctl_color`]).
pub fn text_box(hdc: HDC, frame: RECT, dpi: u32, focused: bool) {
    let p = theme::palette();
    let s = |v| super::scale(v, dpi);
    draw::round_rect(hdc, frame, s(4), p.field, Some(p.field_stroke));
    let line = if focused { s(2) } else { 1 };
    let underline = RECT { left: frame.left + s(3), top: frame.bottom - line, right: frame.right - s(3), bottom: frame.bottom };
    draw::fill(hdc, underline, if focused { p.accent } else { p.field_underline });
}

/// Updates the fonts after a DPI change.
pub fn set_fonts(h: HWND, fonts: &Fonts) {
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
    let s = |v: i32| super::scale(v, dpi);
    let text = super::get_text(h);

    draw::fill(hdc, r, theme::color_of(theme::surface_of(h)));

    if kind == Kind::Switch {
        // Track: a pill, accent-filled when on. Knob: a circle at the left (off) or right (on).
        let (tw, th) = (s(40), s(20));
        let top = r.top + (r.bottom - r.top - th) / 2;
        // Inset so the focus ring (2 px outside the track) stays inside the control.
        let inset = s(3);
        let track = RECT { left: r.left + inset, top, right: r.left + inset + tw, bottom: top + th };
        let (track_fill, track_border, knob) = if checked {
            (if disabled { p.text_disabled } else if hot { p.accent_hot } else { p.accent }, None, p.on_accent)
        } else {
            (if hot && !disabled { p.control_hot } else { p.control }, Some(if disabled { p.text_disabled } else { p.text2 }), if disabled { p.text_disabled } else { p.text2 })
        };
        draw::round_rect(hdc, track, th / 2, track_fill, track_border);
        let k = s(if hot && !disabled { 14 } else { 12 });
        let kx = if checked { track.right - s(4) - k } else { track.left + s(4) };
        let ky = top + (th - k) / 2;
        draw::round_rect(hdc, RECT { left: kx, top: ky, right: kx + k, bottom: ky + k }, k / 2, knob, None);
        let label = RECT { left: track.right + s(10), top: r.top, right: r.right, bottom: r.bottom };
        draw::text(hdc, &text, label, font, if disabled { p.text_disabled } else { p.text }, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        if focused {
            // Kept inside the control: anything outside is clipped (DPI rounding can push it out by a pixel).
            let ring = RECT {
                left: (track.left - s(2)).max(r.left),
                top: (track.top - s(2)).max(r.top),
                right: (track.right + s(2)).min(r.right - 1),
                bottom: (track.bottom + s(2)).min(r.bottom - 1),
            };
            draw::round_rect(hdc, ring, (ring.bottom - ring.top) / 2, draw::NO_FILL, Some(p.text));
        }
        return true;
    }

    if kind == Kind::Check {
        let size = s(18);
        let top = r.top + (r.bottom - r.top - size) / 2;
        let bx = RECT { left: r.left + s(1), top, right: r.left + s(1) + size, bottom: top + size };
        if checked {
            let fill = if disabled { p.text_disabled } else if hot { p.accent_hot } else { p.accent };
            draw::round_rect(hdc, bx, s(4), fill, None);
            draw::glyph(hdc, ICON_TICK, bx, icon_font, p.on_accent, DT_CENTER | DT_VCENTER);
        } else {
            let fill = if hot && !disabled { p.control_hot } else { p.control };
            draw::round_rect(hdc, bx, s(4), fill, Some(if disabled { p.text_disabled } else { p.text2 }));
        }
        let label = RECT { left: bx.right + s(8), top: r.top, right: r.right, bottom: r.bottom };
        draw::text(hdc, &text, label, font, if disabled { p.text_disabled } else { p.text }, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        if focused {
            draw::round_rect(hdc, RECT { left: bx.left - s(1), top: bx.top - s(1), right: bx.right + s(1), bottom: bx.bottom + s(1) }, s(5), draw::NO_FILL, Some(p.text));
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
    draw::round_rect(hdc, r, s(4), fill, border);

    if kind == Kind::DropDown {
        let chevron = RECT { left: r.right - s(12 + 16), right: r.right - s(12), ..r };
        draw::glyph(hdc, ICON_DOWN, chevron, icon_font, fg, DT_CENTER | DT_VCENTER);
        let label = RECT { left: r.left + s(12), right: chevron.left - s(8), ..r };
        draw::text(hdc, &text, label, font, fg, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    } else {
        // Icon and text, centred together.
    let gap = s(8);
    let text_w = draw::text_width(&text, font);
    let icon_w = if icon.is_some() { s(16) } else { 0 };
    let gap = if text.is_empty() { 0 } else { gap }; // icon-only buttons centre the icon
    let total = text_w + if icon.is_some() { icon_w + gap } else { 0 };
    let mut x = r.left + ((r.right - r.left) - total).max(0) / 2;
    if let Some(c) = icon {
        draw::glyph(hdc, c, RECT { left: x, top: r.top, right: x + icon_w, bottom: r.bottom }, icon_font, fg, DT_CENTER | DT_VCENTER);
        x += icon_w + gap;
    }
    draw::text(hdc, &text, RECT { left: x, top: r.top, right: r.right - s(4), bottom: r.bottom }, font, fg, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    if focused {
        let f = RECT { left: r.left + s(2), top: r.top + s(2), right: r.right - s(2), bottom: r.bottom - s(2) };
        draw::round_rect(hdc, f, s(3), draw::NO_FILL, Some(fg));
    }
    true
}

